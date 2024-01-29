//copy from validator

use log::{info, debug};
use std::{
    time::{self, SystemTime}, 
    thread, 
    sync::{Arc, Mutex},
    collections::HashMap,
};
use crate::{        
    types::{
        merkle::MerkleTree, 
        hash::{H256, Hashable},
    }, 
    manifoldchain::{
        block::{
            Info,
            Content,
            BlockHeader,
            consensus_block::ConsensusBlock,
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
            transaction_block::TransactionBlock,
            versa_block::{
                VersaBlock,
                ExclusiveFullBlock,
                InclusiveFullBlock,
            }
        },
        multichain::Multichain,
        transaction::{Transaction, TxFlag},
        validator::{
            Validator,
        },
        configuration::Configuration,
        mempool::Mempool,
        testimony::{
            Testimony,
            TestimonyUnit,
        },
        network::{
            server::Handle as ServerHandle,
            message::Message,
        }
    },
};
use rand::Rng;

pub type BlockLocate = (H256, usize); //(block hash, shard id)
pub type TxLocate = (H256, H256); //(block hash, tx hash)

#[derive(Clone)]
pub struct Confirmation {
    /// Channel for receiving control signal
    multichain: Multichain,
    progress: Vec<usize>,
    //just a quick access to confirmed block and its shard id
    confirmed_blocks: HashMap<BlockLocate, bool>,
    //(block_hash, tx_hash) -> required blocks' hash for confirmation
    unstable_outputs: HashMap<TxLocate, Vec<BlockLocate>>,
    //mapping block to its related output-tx 
    //(input_block_hash, shard_id) -> [(output_block_hash, output_tx_hash)]
    //An input block corresponds to multiple output-txs
    input_block2output: HashMap<BlockLocate, Vec<TxLocate>>,
    //confirmation for domestic-txs and input-txs
    //pre-confirmation for output-txs and accept/reject-txs
    confirmed_txs: HashMap<H256, bool>, 
    //final-confirmation for output-txs and accept/reject-txs
    final_confirmed_txs: HashMap<H256, bool>,
    config: Configuration,
}

impl Confirmation {
    pub fn new(
        multichain: &Multichain, 
        config: &Configuration,
    ) -> Self {
        Confirmation {
            multichain: multichain.clone(),
            progress: (0..config.shard_num).collect(),
            confirmed_blocks: HashMap::new(),
            confirmed_txs: HashMap::new(),
            unstable_outputs: HashMap::new(),
            input_block2output: HashMap::new(),
            final_confirmed_txs: HashMap::new(),
            config: config.clone(),
        }
    }

    //the shard_id stored in the block may be not the right shard_id
    //for example, the shard_id stored in an inclusive block representes the 
    //originate shard, not the shard where it locates 
    pub fn update(
        &mut self, 
        new_block: Option<VersaBlock>, 
        confirmed_block_height: Option<(VersaBlock, usize)>,
        shard_id: usize,
    ) -> Vec<(Transaction, Testimony, Vec<usize>)> {
        let mut return_txs_tmys: Vec<(Transaction, Testimony, Vec<usize>)> = vec![];
        if let Some(confirmation_info) = confirmed_block_height {
            let sub_txs_tmys = self.handle_confirmed_block(
                confirmation_info.0, 
                confirmation_info.1,
                shard_id,
            );
            return_txs_tmys.extend(sub_txs_tmys);
        }
        if let Some(block) = new_block {
            let sub_txs_tmys = self.handle_new_block(block);
            return_txs_tmys.extend(sub_txs_tmys);
        } 
        return_txs_tmys
    }

    fn confirm_tx(
        &mut self, 
        tx: &Transaction, 
        tmy: Option<&Testimony>, 
    ) {
        match tx.flag.clone() {
            TxFlag::Empty => {
                //confirmed directly
                self.confirmed_txs.insert(tx.hash(), true);
            }
            TxFlag::Initial => {
                //confirmed directly
                self.confirmed_txs.insert(tx.hash(), true);
            }
            TxFlag::Domestic => {
                //confirmed directly
                self.confirmed_txs.insert(tx.hash(), true);
            }
            TxFlag::Input => {
                //confirmed directly 
                self.confirmed_txs.insert(tx.hash(), true);
            }
            TxFlag::Output => {
                //pre-confirmed anyway
                self.confirmed_txs.insert(tx.hash(), true);
                //check if it is final-confirmed
                let tmy = tmy.unwrap();
                let mut is_final_confirmed = true;
                for input in tx.inputs.iter() {
                    let input_shard_id = Validator::get_shard_id(
                        &input.sender_addr, 
                        self.config.shard_num
                    );
                    let tmy_unit = tmy.get_tmy_unit(&input.hash()).unwrap();
                    let originate_block = tmy_unit.get_ori_blk_hash();
                    //check whether the block is confirmed
                    match self.confirmed_blocks.get(&(
                        originate_block,
                        input_shard_id
                    )) {
                        Some(_) => {}
                        None => {
                            is_final_confirmed = false;
                            break;
                        }
                    }
                } 
                if is_final_confirmed {
                    self.final_confirmed_txs.insert(tx.hash(), true);
                }
            }
            _ => {//accept-tx/reject-tx
                //pre-confirmed anyway
                self.confirmed_txs.insert(tx.hash(), true);
                //check if it is final-confirmed
                let tmy = tmy.unwrap();
                let mut is_final_confirmed = true;
                for output in tx.outputs.iter() {
                    let output_shard_id = Validator::get_shard_id(
                        &output.receiver_addr,
                        self.config.shard_num
                    );
                    let tmy_unit = tmy.get_tmy_unit(&output.hash()).unwrap();
                    let originate_block = tmy_unit.get_ori_blk_hash();
                    //check whether the block is confirmed
                    match self.confirmed_blocks.get(&(originate_block, output_shard_id)) {
                        Some(_) => {},
                        None => {
                            is_final_confirmed = false;
                            break;
                        }
                    }
                }
                if is_final_confirmed {
                    self.final_confirmed_txs.insert(tx.hash(), true);
                }
            }
        }
    }

    fn return_tx(
        &mut self, 
        block_hash: H256, 
        tx_hash: H256, 
        accpet_or_reject: bool
    ) -> Option<(Transaction, Testimony)> {
        //delete all related information
        let tx_locate = (block_hash.clone(), tx_hash.clone());
        let required_blocks = self.unstable_outputs.get(&tx_locate).unwrap().clone();
        //Delete elements whose key is tx_locate in unstable_outputs directly
        self.unstable_outputs.remove(&tx_locate);
        //As there may be multiple output-txs corresponding to the same block
        //For each require block, delete tx_locate from its corresponding output-txs (update the
        //vector)
        //If the updated vector is empty, remove this element from input_block2output
        //otherwidse, insert the updated vector to input_block2output
        for block_locate in required_blocks.iter() {
            let mut old_elements = self.input_block2output.get(block_locate).unwrap().clone();
            old_elements.retain(|&x| x != tx_locate);
            if old_elements.is_empty() {
                self.input_block2output.remove(block_locate);
            } else {
                self.input_block2output.insert(block_locate.clone(), old_elements);
            }
        }

        //get the exact block
        //get the exact transaction and its corresponding index in the block
        //Both preparation are done for generating its corresponding testimony
        let possible_block = self.multichain.get_block(&block_hash);
        if !possible_block.is_some() {
            return None;
        }
        let block = possible_block.unwrap();
        let mut tx = Transaction::default();
        let mut index = 0;
        let txs = block.get_txs_ref().unwrap();
        for i in 0..self.config.block_size {
            if txs[i].hash() == tx_hash {
                index = i;
                tx = txs[i].clone();
            }
        }
    
        //generate the related testimony
        let mut tmy_units: Vec<TestimonyUnit> = vec![];
        for output in tx.outputs.iter() {
            if Validator::get_shard_id(
                &output.receiver_addr,
                self.config.shard_num
            ) == self.config.shard_id {
                let tmy_unit = TestimonyUnit::create(
                    output.hash(),
                    block_hash.clone(),
                    block.get_tx_merkle_proof(index).unwrap(),
                    index,
                );
                tmy_units.push(tmy_unit);
            }
        }
        let tmy = Testimony::create(
            tx_hash.clone(),
            tmy_units
        );
        //set the tx as accept-tx or reject-tx
        tx.flag = match accpet_or_reject {
            true => TxFlag::Accept,
            false => TxFlag::Reject,
        };
        Some((tx, tmy))
    }

    fn handle_confirmed_block(
        &mut self, 
        confirmed_block: VersaBlock, 
        confirmed_height: usize,
        shard_id: usize,
    ) -> Vec<(Transaction, Testimony, Vec<usize>)> {
        let confirmed_block_hash = confirmed_block.hash();
        let confirmed_locate = (confirmed_block_hash.clone(), shard_id);
        let mut return_txs_tmys: Vec<(Transaction, Testimony, Vec<usize>)> = vec![];
        if let None = self.confirmed_blocks.get(&confirmed_locate) {
            self.confirmed_blocks.insert(
                confirmed_locate.clone(), 
                true
            );
            //update the progress in related shard
            if confirmed_height > self.progress[shard_id] {
                self.progress[shard_id] = confirmed_height;
            }
            //confirm the txs in confirmed block
            match confirmed_block.get_txs() {
                Some(txs) => {
                    let tmys = confirmed_block.get_tmys().unwrap();
                    for tx in txs.iter() {
                        let tmy = tmys.get(&tx.hash());
                        self.confirm_tx(tx, tmy);
                    }
                }
                None => {}
            }
            //check if any accept/reject-txs should be sent
            if let None = self.input_block2output.get(&confirmed_locate) {

            } else {
                let items = self.input_block2output.get(&confirmed_locate).unwrap().clone();
                //As there may be multiple output-txs'confirmation corresponding to the same
                //input-block, each output-tx should be handled individually
                for item in items.iter() {
                    //get all related blocks required for becoming stable
                    if let None = self.unstable_outputs.get(item) {
                        continue;
                    }
                    let required_blocks = self.unstable_outputs.get(item).unwrap();
                    let mut all_confirmed = true;
                    let mut confirmed_to_accept = true;
                    let mut shards: HashMap<usize, bool> = HashMap::new();
                    //For each required block, check if it is confirmed or deconfirmed
                    for block in required_blocks.iter() {
                        let block_hash = block.0.clone();
                        let shard_id = block.1;
                        let block_height = self.multichain.get_block_height_with_shard(
                            &block_hash,
                            shard_id
                        );
                        shards.insert(shard_id, true);
                        match self.confirmed_blocks.get(block) {
                            Some(_) => {},
                            None => {
                                match block_height {
                                    Some(height) => {
                                        if self.progress[shard_id] >= height {
                                            confirmed_to_accept = false;
                                        } else {
                                            all_confirmed = false;
                                            break;
                                        }
                                    }
                                    None => {
                                        all_confirmed = false;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if all_confirmed {
                        let shards = shards
                            .into_iter()
                            .map(|(key, _)| key)
                            .collect();
                        if let Some((return_tx, return_tmy)) = self.return_tx(
                            item.0.clone(), 
                            item.1.clone(), 
                            confirmed_to_accept
                        ) {
                            return_txs_tmys.push((return_tx, return_tmy, shards)); 
                        }
                    } 
                }
            }
        }
        return_txs_tmys
    }

    fn handle_new_block(
        &mut self,
        new_block: VersaBlock,
    ) -> Vec<(Transaction, Testimony, Vec<usize>)> {
        let mut return_txs_tmys: Vec<(Transaction, Testimony, Vec<usize>)> = vec![];
        //update information about output-txs
        match new_block.get_txs() {
            Some(txs) => {
                let tmys = new_block.get_tmys().unwrap();
                let new_block_hash = new_block.hash();
                for tx in txs {
                    if let TxFlag::Output = tx.flag {
                        let tx_hash = tx.hash();
                        //get all required blocks and throw them to a set
                        let mut input_ori_blocks: HashMap<BlockLocate, bool> 
                            = HashMap::new();
                        let tmy = tmys.get(&tx_hash).unwrap();
                        for input in tx.inputs.iter() {
                            let tmy_unit = tmy.get_tmy_unit(&input.hash()).unwrap();
                            let ori_shard_id = Validator::get_shard_id(
                                &input.sender_addr, 
                                self.config.shard_num
                            );
                            let ori_block_hash = tmy_unit.get_ori_blk_hash();
                            input_ori_blocks.insert(
                                (ori_block_hash, ori_shard_id), 
                                true
                            );
                        }
                        //transafer the set into a vector(maybe it is not necessary)
                        let required_confirmd_blocks: Vec<BlockLocate> = input_ori_blocks
                            .into_iter()
                            .map(|(key, _)| key)
                            .collect();
                        //check whether all the blocks are confirmed already
                        let mut all_confirmed = true;
                        let mut confirmed_to_accept = true;
                        for item in required_confirmd_blocks.iter() {
                            let block_hash = item.0.clone();
                            let shard_id = item.1;
                            let block_height = self.multichain.get_block_height_with_shard(
                                &block_hash,
                                shard_id
                            );
                            match self.confirmed_blocks.get(&(block_hash, shard_id)) {
                                Some(_) => {}
                                None => {
                                    match block_height {
                                        Some(height) => {
                                            if self.progress[shard_id] >= height {
                                                confirmed_to_accept = false;        
                                            } else {
                                                all_confirmed = false;
                                                break;
                                            }
                                        }
                                        None => {
                                            all_confirmed = false;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        //update related information
                        self.unstable_outputs.insert(
                            (new_block_hash.clone(), tx_hash.clone()), 
                            required_confirmd_blocks.clone()
                        );
                        for item in required_confirmd_blocks.iter() {
                            //check if the related vec in input_block2output exits, 
                            match self.input_block2output.get(item) {
                                None => {
                                    //if not, create a new one
                                    self.input_block2output.insert(
                                        item.clone(),
                                        vec![(new_block_hash.clone(), tx_hash.clone())]
                                    );
                                }
                                Some(old_elements) => {
                                    //if yes, insert the new element
                                    let mut new_elements = old_elements.clone();
                                    new_elements.push((new_block_hash.clone(), tx_hash.clone()));
                                    self.input_block2output.insert(
                                        item.clone(),
                                        new_elements
                                    );
                                }
                            }
                            
                        }
                        if all_confirmed {
                            //if the inputs of the output-tx are all confirmed/deconfirmed, return
                            //the corresponding accept-reject-tx to all related shards 
                            let mut shards: HashMap<usize, bool> = HashMap::new();
                            for item in required_confirmd_blocks.iter() {
                                shards.insert(item.1, true);
                            }
                            let shards: Vec<usize> = shards
                                .into_iter()
                                .map(|(key, _)| key)
                                .collect();
                            if let Some((return_tx, return_tmy)) = self.return_tx(
                                new_block_hash.clone(), 
                                tx_hash.clone(), 
                                confirmed_to_accept
                            ) {
                                return_txs_tmys.push((return_tx, return_tmy, shards));
                            }
                        } 
                    }
                }
            }
            None => {}
        }
        return_txs_tmys
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST





