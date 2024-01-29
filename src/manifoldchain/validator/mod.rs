use crate::{
    manifoldchain::{
        blockchain::{
            State,
            VerStatus,
        },
        multichain::Multichain,
        transaction::{Transaction, UtxoInput, TxFlag},
        block::{
            Info, 
            Content, 
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
            versa_block::{
                VersaBlock,
                ExclusiveFullBlock,
                InclusiveFullBlock,
            },
        },
        configuration::Configuration,
        mempool::Mempool,
        testimony::{
            Testimony,
        },
        fraudproof::{
            FraudProof,
            DoubleSpendingProof,
            UtxoLostProof,
            WrongShardProof,
            UnequalCoinsProof,
            WrongSignatureProof,
            TestimonyLostProof,
            WrongTestimonyProof,
        },
        network::worker::{
            Sample,
            SampleIndex,
        },
    },
    types::{
        hash::{Hashable, H256},
        merkle::MerkleTree,
    },
};
use std::{
    sync::{Arc, Mutex},
    collections::HashMap,
};
use log::{info, debug};

pub struct Validator {
    multichain: Multichain,
    mempool: Arc<Mutex<Mempool>>,
    config: Configuration,
}

impl Clone for Validator {
    fn clone(&self) -> Self {
        Validator {
            multichain: self.multichain.clone(),
            mempool: Arc::clone(&self.mempool),
            config: self.config.clone(),
        }
    }
}

#[derive(Clone)]
pub enum ValidationSource {
    FromBlock,
    FromTransaction,
}

pub enum CrossUtxoStatus {
    Available,
    Confirmed,
}


impl Validator {
    pub fn new(
        multichain: &Multichain,
        mempool: &Arc<Mutex<Mempool>>,
        config: &Configuration
    ) -> Self {
        Validator {
            multichain: multichain.clone(),
            mempool: Arc::clone(mempool),
            config: config.clone(),
        }
    }

    pub fn check_input_from_state(
        input: & UtxoInput, 
        state: & State
    ) -> Result<(Transaction, Option<Testimony>), FraudProof> 
    {
        match state.get(&(input.tx_hash.clone(), input.index)) {
            Some(item) => {
                let tx = item.0.clone();
                let tmy = item.1.clone();
                let sig_vec = input.sig_ref.clone();
                let index: usize = input.index as usize;
                //handle reject-tx
                if let TxFlag::Reject = tx.flag {
                    //complete later
                    return Ok((tx, tmy));
                }
                let output = tx.outputs.get(index).unwrap();
                let pub_key = output.public_key_ref.clone();
                match Transaction::verify(&tx, pub_key.as_slice(), sig_vec.as_slice()) {
                    true => Ok((tx, tmy)),
                    false => {
                        Err(FraudProof::WrongSignature(
                            WrongSignatureProof {
                                shard_id: 0,

                                invalid_block_hash: H256::default(),
                                invalid_tx: Transaction::default(),
                                invalid_tx_merkle_proof: vec![],
                                invalid_index: 0,

                                conflict_block_hash: H256::default(),
                                conflict_tx: tx.clone(),
                                conflict_tx_merkle_proof: vec![],
                                conflict_index: 0,
                            }
                        ))
                    }
                }
            }
            None => {
                Err(FraudProof::UnsolvedFault)
            }
        }
    }

    
    
    pub fn get_shard_id(hash: &H256, shard_num: usize) -> usize {
        //one u8 can represent 256 shards
        let byte_size = shard_num / 256 + 1;
        let mut value: usize = 0;
        for i in 32-byte_size..32 {
            value = value*256 + (hash.0[i] as usize);
        }
        value % shard_num
    }
    
    //if there is one input/output locating at the current shard, then it belongs to the current
    //shard
    pub fn check_tx_ownership(tx: &Transaction, shard_id: usize, shard_num: usize) -> bool {
        for input in tx.inputs.iter() {
            let input_shard_id = Self::get_shard_id(&input.sender_addr, shard_num);
            if input_shard_id == shard_id {
                return true;
            }
        }

        for output in tx.outputs.iter() {
            let output_shard_id = Self::get_shard_id(&output.receiver_addr, shard_num);
            if output_shard_id == shard_id {
                return true;
            }
        }

        false
    }
    
//    //the current mechanism is not secure enough, but it is ok for experimentation
//    //it is not neccessary for out implementation, for convenience we skip the verification of
//    //initial transactions
//    fn validate_initial_tx(
//        &self, 
//        tx: &Transaction, 
//        history: &Vec<H256>
//    ) -> Result<bool, String> {
//        let output = tx.outputs.get(0).unwrap();
//        if output.value != self.config.initial_balance {
//            return Err(String::from("validate initial tx: value is not match"));
//        }
//        let history_blks: Vec<VersaBlock> =
//            (0..history.len()).map(|i| 
//                self.multichain.get_block(&history[i]).unwrap()    
//            ).collect();
//        //check whether there is another initial transaction for the same address exits
//        for i in 0..(history.len() - self.config.k) {
//            if let Some(txs) = history_blks[i].get_txs_ref() {
//                for ttx in txs.iter() {
//                    //check whether is an initial transaction
//                    if let TxFlag::Initial = ttx.flag {
//                        let ottx = ttx.outputs.get(0).unwrap();
//                        if ottx.receiver_addr == output.receiver_addr {
//                            return Err(String::from("validate initial tx: double initial tx"));
//                        }
//                    }
//                }
//            } 
//        }
//        Ok(true)
//    }

    pub fn validate_tx(
        &self, 
        tx: &Transaction,
        tmy: Option<Testimony>,
        parent: Option<&H256>, 
        flag: ValidationSource) -> Result<bool, FraudProof> 
    {
        if let TxFlag::Initial = tx.flag {
          //      let history = self.multichain.all_blocks_end_with_block(parent_hash).unwrap();
          //      match self.validate_initial_tx(tx, &history) {
          //          Ok(_) => return Ok(true),
          //          Err(_) => return Err(FraudProof::UnsolvedFault),
          //      }
                //we dont verify the initial transactions because it is for experiment setting
            return Ok(true);
        }

        if let TxFlag::Empty = tx.flag {
            return Ok(true);
        }

        //1. check whether the tx belongs to the node accroding to the shard id
        let tx_hash = tx.hash();
        if !Self::check_tx_ownership(tx, self.config.shard_id, self.config.shard_num) {
            //Just return a husk
            return Err(FraudProof::WrongShard(
                WrongShardProof {
                    shard_id: self.config.shard_id as u32,
                    block_hash: H256::default(),
                    invalid_tx: tx.clone(),
                    invalid_tx_merkle_proof: vec![],
                    invalid_index: 0
                }
            ));
        } 

        let mut available_coins: u32 = 0;
        let mut spent_coins: u32 = 0;
        
        for input in tx.inputs.iter() {
            available_coins += input.value;
        }

        for output in tx.outputs.iter() {
            spent_coins += output.value;
        }

        if available_coins != spent_coins {
            return Err(FraudProof::UnequalCoins(
                UnequalCoinsProof {
                    shard_id: self.config.shard_id as u32,
                    block_hash: H256::default(),
                    invalid_tx: tx.clone(),
                    invalid_tx_merkle_proof: vec![],
                    invalid_index: 0
                }
            ));
        }

        if let ValidationSource::FromTransaction = flag {
            //2. If it is a coming transaction, check if it is new 
            if self.mempool.lock().unwrap().check(&tx_hash) {
                return Err(FraudProof::UnsolvedFault);
            }
            if let Some(_) = self.multichain
                        .get_tx_in_longest_chain(&tx_hash) {
                return Err(FraudProof::UnsolvedFault);
            }
            Ok(true)
        } else {
            let parent_hash = parent.unwrap();
            //4. check whether the tx is creating the initial balance
            //3. If it is a tx from block, check the double spending and signatrue
            let states = self.multichain.get_states();

            let state = states
                .get(parent_hash)
                .unwrap();
            match self.check_tx_from_state(
                tx,
                tmy,
                parent.unwrap(),
                &state,
            ) {
                Ok(_) => return Ok(true),
                Err(proof) => {
                    return Err(proof);        
                }
            }
        }
       
    }

    pub fn validate_block(&self, block: &VersaBlock, parent: &H256) -> Result<bool, FraudProof> {
        //check whether the PoW is valid
        let blk_hash = block.hash();
        
        //check the hash value is corrent
        if !block.verify_hash() {
            return Err(FraudProof::UnsolvedFault);
        }
        
        //For exclusive blocks and inclusive blocks, skip the verification of transactions
        match block {
            VersaBlock::ExBlock(_) => return Ok(true),
            VersaBlock::InBlock(_) => return Ok(true),
            _ => {}
        }


        ////For full blocks, verify the transaction
        ////
        //let mut longest_parent = H256::default();
        //let mut longest_height = 0;
        //let mut is_parent_found = false;
        //for parent in block.get_inter_parents() {
        //    match self.multichain.get_block_height_with_shard(
        //        &parent,
        //        self.config.shard_id,
        //    ) {
        //        Some(height) => {
        //            if height >= longest_height {
        //                longest_height = height;
        //                longest_parent = parent;
        //            }
        //            is_parent_found = true;
        //        }
        //        None => {}
        //    }
        //}
        //if !is_parent_found {
        //    return Err(FraudProof::UnsolvedFault);
        //}
        match self.multichain.get_block(parent) {
            Some(_) => {}
            None => {
                info!("validation: parent not found");
                return Err(FraudProof::UnsolvedFault);              
            }
        }
        let states = self.multichain.get_states();

        let state = states
            .get(parent)
            .unwrap();


        //check whether the transactions inside are invalid
        let mut set: HashMap<H256, (Transaction, usize)> = HashMap::new();
        let txs = block.get_txs_ref().unwrap();
        let tmys = block.get_tmys().unwrap();
        for i in 0..txs.len() {
            let tx_ref = &txs[i];
            let tx_hash = tx_ref.hash();
            let tmy = tmys.get(&tx_hash);
            match self.validate_tx(
                tx_ref,
                tmy.cloned(),
                Some(parent),
                ValidationSource::FromBlock
            ) {
                Ok(_) => {}
                Err(proof) => {
                    info!("invalid tx {:?} in validating block: {:?}", tx_ref, blk_hash);
                    info!("invalid type: {:?}", proof);
                    match proof {
                        FraudProof::DoubleSpending(mut fp) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            fp.invalid_block_hash = blk_hash.clone();
                            fp.invalid_tx_merkle_proof = invalid_tx_merkle_proof;
                            fp.invalid_index = i as u32;
                            return Err(FraudProof::DoubleSpending(fp));
                        }
                        FraudProof::UtxoLost(mut fp) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            fp.block_hash = blk_hash.clone();
                            fp.invalid_tx_merkle_proof = invalid_tx_merkle_proof;
                            fp.invalid_index = i as u32;
                            return Err(FraudProof::UtxoLost(fp));
                        }
                        FraudProof::WrongShard(mut fp) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            fp.block_hash = blk_hash.clone();
                            fp.invalid_tx_merkle_proof = invalid_tx_merkle_proof;
                            fp.invalid_index = i as u32;
                            return Err(FraudProof::WrongShard(fp));
                        }
                        FraudProof::UnequalCoins(mut fp) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            fp.block_hash = blk_hash.clone();
                            fp.invalid_tx_merkle_proof = invalid_tx_merkle_proof;
                            fp.invalid_index = i as u32;
                            return Err(FraudProof::UnequalCoins(fp));
                        }
                        FraudProof::WrongSignature(mut fp) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            fp.invalid_block_hash = blk_hash.clone();
                            fp.invalid_tx_merkle_proof = invalid_tx_merkle_proof;
                            fp.invalid_index = i as u32;
                            return Err(FraudProof::WrongSignature(fp));
                        }
                        FraudProof::TestimonyLost(mut fp) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            fp.block_hash = blk_hash.clone();
                            fp.invalid_tx_merkle_proof = invalid_tx_merkle_proof;
                            fp.invalid_index = i as u32;
                            return Err(FraudProof::TestimonyLost(fp));
                        }
                        FraudProof::WrongTestimony(mut fp) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            fp.block_hash = blk_hash.clone();
                            fp.invalid_tx_merkle_proof = invalid_tx_merkle_proof;
                            fp.invalid_index = i as u32;

                            let invalid_tmy = tmy.unwrap();
                            let (invalid_tmy_merkle_proof, invalid_tmy_index) = block
                                .get_tmy_merkle_proof(&invalid_tmy.hash()).unwrap();
                            fp.invalid_tmy = invalid_tmy.clone();
                            fp.invalid_tmy_merkle_proof = invalid_tmy_merkle_proof;
                            fp.invalid_tmy_index = invalid_tmy_index as u32;
                            return Err(FraudProof::WrongTestimony(fp));
                        }
                        FraudProof::UnsolvedFault => {
                            return Err(FraudProof::UnsolvedFault);
                        }
                    }
                }
            }
            if tx_ref.flag == TxFlag::Input ||
                tx_ref.flag == TxFlag::Domestic {
                for input in tx_ref.inputs.iter() {
                    match set.get(&input.hash()) {
                        Some((conflict_tx, conflict_index)) => {
                            let invalid_tx_merkle_proof = block
                                .get_tx_merkle_proof(i).unwrap();
                            let conflict_tx_merkle_proof = block
                                .get_tx_merkle_proof(*conflict_index).unwrap();
                            return Err(FraudProof::DoubleSpending(
                                DoubleSpendingProof {
                                    shard_id: self.config.shard_id as u32,

                                    invalid_tx: tx_ref.clone(),
                                    invalid_block_hash: blk_hash.clone(),
                                    invalid_tx_merkle_proof,
                                    invalid_index: i as u32,

                                    conflict_tx: conflict_tx.clone(),
                                    conflict_block_hash: blk_hash.clone(),
                                    conflict_tx_merkle_proof,
                                    conflict_index: *conflict_index as u32,
                                }
                            ))
                        }
                        None => {
                            set.insert(input.hash(), (tx_ref.clone(), i));
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    pub fn validate_cross_utxo(
        &self, 
        tx: &Transaction, 
        unit_hash: &H256, 
        tmy: &Testimony,
        ori_shard_id: usize,
        status: CrossUtxoStatus
    ) -> Result<bool, String> {
        let tx_hash = tx.hash();
        let ori_block_hash = match tmy
            .get_ori_blk_hash(unit_hash.clone()) {
                Some(hash) => hash,
                None => {
                    return Err(String::from("Originate block not exit"));
                }
            };
        let tx_index = match tmy
            .get_tx_index(unit_hash.clone()) {
                Some(index) => index,
                None => {
                    return Err(String::from("originate tx not found"));
                }
            };
        let tx_merkle_proof = match tmy 
            .get_tx_merkle_proof(unit_hash.clone()) {
                Some(proof) => proof,
                None => {
                    return Err(String::from("originate proof not found"));
                }
            };
        match self.multichain.get_consensus_block_by_shard(
            ori_shard_id,
            &ori_block_hash
        ) {
            Some(blk) => {
                //verify whether the testimony is valid
                if !MerkleTree::<Transaction>::verify(
                    &blk.get_tx_merkle_root(),
                    &tx_hash,
                    &tx_merkle_proof,
                    tx_index,
                    self.config.block_size,
                ) {
                    return Err(
                        String::from(
                            "validating cross utxo: some testimonies are not valid"
                        )
                    );
                }
                //verify whether the utxo is growing on the longest chain
                match status {
                    CrossUtxoStatus::Available => {
                        if !self.multichain.is_block_in_longest_chain(
                            ori_shard_id,
                            &ori_block_hash
                        ) {
                            return Err(
                                String::from(
                                    "validating cross utxo exit: not extending the longest chain"
                                )
                            );
                        }
                        Ok(true)
                    }
                    CrossUtxoStatus::Confirmed => {
                        if !self.multichain.is_block_confirmed(
                            ori_shard_id,
                            &ori_block_hash
                        ) {
                            return Err(
                                String::from(
                                    "validating cross utxo confimed: originate block not confirmed"
                                )
                            );
                        }
                        Ok(true)
                    }
                }
            }
            None => {
                return Err(
                    String::from(
                        "validating cross utxo: originate block not found"
                    )
                );
            }
        }
    }
    pub fn check_tx_from_state(&self, 
        tx: &Transaction,
        tmy: Option<Testimony>,
        verified_parent: &H256,
        state: &State
    ) -> Result<bool, FraudProof> {
        let flag = tx.flag.clone();
        match flag {
            TxFlag::Initial => Ok(true),
            TxFlag::Empty => Ok(true),
            TxFlag::Domestic => self.check_domestic_input_tx_from_state(
                tx,
                verified_parent,
                state
            ),
            TxFlag::Input => self.check_domestic_input_tx_from_state(
                tx,
                verified_parent,
                state
            ),
            TxFlag::Output => {
                if let Some(tmy) = tmy {
                    self.check_output_tx(tx, &tmy)
                } else {
                    Err(FraudProof::UnsolvedFault)
                }
            }
            TxFlag::Accept => {
                if let Some(tmy) = tmy {
                    self.check_accept_reject_tx(tx, &tmy)
                } else {
                    Err(FraudProof::UnsolvedFault)
                }
            }
            TxFlag::Reject => {
                if let Some(tmy) = tmy {
                    self.check_accept_reject_tx(tx, &tmy)
                } else {
                    Err(FraudProof::UnsolvedFault)
                }
            }
        }
    }

    pub fn check_domestic_input_tx_from_state(
        &self,
        tx: &Transaction,
        verified_parent: &H256,
        state: &State
    ) -> Result<bool, FraudProof> {
        let mut set: HashMap<H256, bool> = HashMap::new();
        for input in tx.inputs.iter() {
            let shard_id = Self::get_shard_id(
                &input.sender_addr,
                self.config.shard_num
            );
            if shard_id != self.config.shard_id {
                continue;
            }
            let input_hash = input.hash();
            //check whether the double spending happens inside the tx
            match set.get(&input_hash) {
                Some(_) => {
                    return Err(FraudProof::DoubleSpending(
                        DoubleSpendingProof {
                            shard_id: self.config.shard_id as u32,

                            invalid_block_hash: H256::default(),
                            invalid_tx: tx.clone(),
                            invalid_tx_merkle_proof: vec![],
                            invalid_index: 0 as u32,

                            conflict_tx: tx.clone(),
                            conflict_block_hash: H256::default(),
                            conflict_tx_merkle_proof: vec![],
                            conflict_index: 0 as u32,
                        }
                    ));                   
                }
                None => {
                    set.insert(input_hash, true);
                }
            }
            //check whether the coins exit in the state
            match Self::check_input_from_state(input, state) {
                Ok((input_tx, possible_tmy)) => {
                    match possible_tmy {
                        Some(tmy) => {
                            //check whether the input_tx is an output-tx or reject-tx
                            //if it is an output-tx, we check the testimony for each input
                            //if it is an reject-tx, we check the testimony for each output
                            let flag = input_tx.flag.clone();
                            match flag {
                                TxFlag::Output => {
                                    for input_tx_input in input_tx.inputs.iter() {
                                        let ori_shard_id = Self::get_shard_id(
                                            &input_tx_input.sender_addr,
                                            self.config.shard_num
                                        );
                                        match self.validate_cross_utxo(
                                            &input_tx,
                                            &input_tx_input.hash(),
                                            &tmy,
                                            ori_shard_id,
                                            CrossUtxoStatus::Confirmed
                                        ) {
                                            Ok(_) => {}
                                            Err(_) => {
                                                 return Err(FraudProof::WrongTestimony(
                                                    WrongTestimonyProof {
                                                        shard_id: self.config.shard_id as u32,
                                                        block_hash: H256::default(),
                                                        invalid_tx: tx.clone(),
                                                        invalid_tx_merkle_proof: vec![],
                                                        invalid_index: 0 as u32,

                                                        invalid_tmy: tmy.clone(),
                                                        invalid_tmy_merkle_proof: vec![],
                                                        invalid_tmy_index: 0 as u32,
                                                    }
                                                ));
                                            }
                                        }
                                    } 
                                }
                                TxFlag::Reject => {
                                    for input_tx_output in input_tx.outputs.iter() {
                                        let ori_shard_id = Self::get_shard_id(
                                            &input_tx_output.receiver_addr,
                                            self.config.shard_num
                                        );
                                        match self.validate_cross_utxo(
                                            &input_tx,
                                            &input_tx_output.hash(),
                                            &tmy,
                                            ori_shard_id,
                                            CrossUtxoStatus::Confirmed
                                        ) {
                                            Ok(_) => {}
                                            Err(_) => {
                                                 return Err(FraudProof::WrongTestimony(
                                                    WrongTestimonyProof {
                                                        shard_id: self.config.shard_id as u32,
                                                        block_hash: H256::default(),
                                                        invalid_tx: tx.clone(),
                                                        invalid_tx_merkle_proof: vec![],
                                                        invalid_index: 0 as u32,

                                                        invalid_tmy: tmy.clone(),
                                                        invalid_tmy_merkle_proof: vec![],
                                                        invalid_tmy_index: 0 as u32,
                                                    }
                                                ));
                                            }                                           
                                        }
                                    }
                                }
                                _ => {
                                    panic!("Only output-tx and reject-tx contains testimony");
                                }
                            }
                        }
                        None => {
                            //Nothing to do, continue to verify next input
                        }
                    }
                }
                Err(proof) => {
                    let new_proof = self.handle_check_tx_from_state_err(
                        tx, 
                        proof, 
                        verified_parent,
                        &input_hash,
                    );
                    return Err(new_proof);
                }
            }
        }
        Ok(true)
    }
    
    pub fn check_output_tx(
        &self,
        tx: &Transaction,
        tmy: &Testimony,
    ) -> Result<bool, FraudProof> { //(commit or not, fraud proof)
        let mut set: HashMap<H256, bool> = HashMap::new();
        let mut all_valid_inputs = true;
        let mut one_valid_input = false;
        for input in tx.inputs.iter() {
            let input_hash = input.hash();
            //check whether the double spending occurs inside the tx
            match set.get(&input_hash) {
                Some(_) => {
                     return Err(FraudProof::DoubleSpending(
                        DoubleSpendingProof {
                            shard_id: self.config.shard_id as u32,

                            invalid_block_hash: H256::default(),
                            invalid_tx: tx.clone(),
                            invalid_tx_merkle_proof: vec![],
                            invalid_index: 0 as u32,

                            conflict_tx: tx.clone(),
                            conflict_block_hash: H256::default(),
                            conflict_tx_merkle_proof: vec![],
                            conflict_index: 0 as u32,
                        }
                    ));                   
                }
                None => {
                    set.insert(input_hash, true);
                }
            }

            //check whether there is at least one valid input
            let ori_shard_id = Self::get_shard_id(
                &input.sender_addr,
                self.config.shard_num
            );
            let mut ori_tx = tx.clone();
            ori_tx.flag = TxFlag::Input;
            match self.validate_cross_utxo(
                &ori_tx, 
                &input_hash,
                tmy,
                ori_shard_id,
                //CrossUtxoStatus::Available
                CrossUtxoStatus::Confirmed,
            ) {
                Ok(_) => {
                    one_valid_input = true;
                }
                Err(_) => {
                    all_valid_inputs = false;
                }
            }
        }
        if all_valid_inputs {
            return Ok(true);
        } else {
            if one_valid_input {
                return Ok(false);
            } else{
                return Ok(false);
                // remain the invalid output tx?
                //return Err(FraudProof::UnsolvedFault);
            }
        }

    }

    pub fn check_accept_reject_tx(
        &self,
        tx: &Transaction,
        tmy: &Testimony,
    ) -> Result<bool, FraudProof> {
        //simply
        return Ok(true);
        let tx_hash = tx.hash();
        let mut input_tx = tx.clone();
        input_tx.flag = TxFlag::Input;
        if let None = self.multichain
            .get_tx_in_longest_chain(&input_tx.hash()) {
            return Err(FraudProof::UnsolvedFault);
        }
        for output in tx.outputs.iter() {
            let output_hash = output.hash();
            let ori_shard_id = Self::get_shard_id(
                &output.receiver_addr,
                self.config.shard_num
            );
            let mut ori_tx = tx.clone();
            ori_tx.flag = TxFlag::Output;
            match self.validate_cross_utxo(
                &ori_tx,
                &output_hash,
                tmy,
                ori_shard_id,
                CrossUtxoStatus::Confirmed,
            ){
                Ok(_) => {}
                Err(e) => {
                     return Err(FraudProof::WrongTestimony(
                        WrongTestimonyProof {
                            shard_id: self.config.shard_id as u32,
                            block_hash: H256::default(),
                            invalid_tx: tx.clone(),
                            invalid_tx_merkle_proof: vec![],
                            invalid_index: 0 as u32,

                            invalid_tmy: tmy.clone(),
                            invalid_tmy_merkle_proof: vec![],
                            invalid_tmy_index: 0 as u32,
                        }
                    ));
                }
            }
        }
        Ok(true)
    }

    fn handle_check_tx_from_state_err(
        &self, 
        tx: &Transaction, 
        proof: FraudProof, 
        verified_parent: &H256,
        input_hash: &H256,
    ) -> FraudProof {
        match proof {
            FraudProof::WrongSignature(mut fp) => {
                fp.shard_id = self.config.shard_id as u32;
                fp.invalid_tx = tx.clone();                            
                //complete the information of conflict tx
                let conflict_tx_hash = tx.hash();
                let (conflict_block, conflict_index) = self.multichain
                    .get_block_with_tx(&conflict_tx_hash).unwrap();
                fp.conflict_block_hash = conflict_block.hash();
                let conflict_tx_merkle_proof = conflict_block
                    .get_tx_merkle_proof(conflict_index).unwrap();
                fp.conflict_tx_merkle_proof = conflict_tx_merkle_proof;
                fp.conflict_index = conflict_index as u32;
                return FraudProof::WrongSignature(fp);
            }
            FraudProof::UnsolvedFault => {
                let history_hash_vec = self.multichain
                    .all_blocks_end_with_block(verified_parent).unwrap();
                let history_blocks: Vec<VersaBlock> = history_hash_vec
                    .into_iter()
                    .map(|x| self.multichain.get_block(&x).unwrap())
                    .collect();
                let mut conflict_block: Option<VersaBlock> = None;
                let mut conflict_tx: Option<Transaction> = None;
                let mut conflict_tx_index: Option<usize> = None;
                for blk in history_blocks {
                    match blk {
                        VersaBlock::ExBlock(_) => continue,
                        VersaBlock::InBlock(_) => continue,
                        _ => {}
                    }
                    let txs = blk.get_txs_ref().unwrap();
                    for i in 0..txs.len() {
                        let tx_ref = &txs[i];
                        for input2 in tx_ref.inputs.iter() {
                            if input2.hash() == *input_hash {
                                conflict_block = Some(blk.clone());
                                conflict_tx = Some(tx_ref.clone());
                                conflict_tx_index = Some(i);
                            }
                        }
                    }
                }
                if conflict_block.is_some() {
                    let conflict_block = conflict_block.unwrap();
                    let conflict_tx = conflict_tx.unwrap();
                    let conflict_tx_index = conflict_tx_index.unwrap();
                    let conflict_tx_merkle_proof = conflict_block
                        .get_tx_merkle_proof(conflict_tx_index).unwrap();
                    return FraudProof::DoubleSpending(
                        DoubleSpendingProof {
                            shard_id: self.config.shard_id as u32,
                            invalid_block_hash: H256::default(),
                            invalid_tx: tx.clone(),
                            invalid_tx_merkle_proof: vec![],
                            invalid_index: 0 as u32,

                            conflict_tx: conflict_tx,
                            conflict_block_hash: conflict_block.hash(),
                            conflict_tx_merkle_proof,
                            conflict_index: conflict_tx_index as u32,
                        }
                    ); 
                } else {
                    return FraudProof::UtxoLost(
                        UtxoLostProof {
                            shard_id: self.config.shard_id as u32,
                            block_hash: H256::default(),
                            invalid_tx: tx.clone(),
                            invalid_tx_merkle_proof: vec![],
                            invalid_index: 0 as u32,
                        }
                    );
                }
            }
            _ => {
                return FraudProof::UnsolvedFault;
            }
        }
    }

    pub fn verify_fraud_proof(&self, fraud_proof: &FraudProof) -> bool {
        //need to be completed in the future
        return true;
        if let FraudProof::UnsolvedFault = fraud_proof {
            return true;
        }
        let invalid_block_hash = fraud_proof.get_invalid_block();
        let shard_id = fraud_proof.get_shard_id();
    
        //check whether the block exits
        let invalid_block = match self.multichain
            .get_block_by_shard(&invalid_block_hash, shard_id) {
            Some(block) => block,
            None => return false,
        };

        //check whether the block is still unverified
        match self.multichain.get_verify_status_with_shard(
            &invalid_block_hash,
            shard_id
        ).unwrap() {
            VerStatus::Unverified => {},
            _ => return false,
        };

        //check the inclusion of invalid transaction
        let invalid_tx_merkle_root = invalid_block.get_tx_merkle_root();
        let invalid_tx = fraud_proof.get_invalid_tx();
        let invalid_tx_merkle_proof = fraud_proof.get_invalid_tx_merkle_proof();
        let invalid_index = fraud_proof.get_invalid_index();
        let invalid_tx_hash = invalid_tx.hash();
        if !MerkleTree::<Transaction>::verify(
            &invalid_tx_merkle_root,
            &invalid_tx_hash,
            &invalid_tx_merkle_proof,
            invalid_index,
            self.config.block_size
        ) {
            return false;
        }

        match fraud_proof {
            FraudProof::DoubleSpending(ds_fp) => self.verify_doublespending_fp(ds_fp),
            FraudProof::UtxoLost(ul_fp) => self.verify_utxolost_fp(ul_fp),
            FraudProof::WrongShard(ws_fp) => self.verify_wrongshard_fp(ws_fp),
            FraudProof::UnequalCoins(uc_fp) => self.verify_unequalcoins_fp(uc_fp),
            FraudProof::WrongSignature(wsig_fp) => self.verify_wrongsig_fp(wsig_fp),
            FraudProof::TestimonyLost(tl_fp) => self.verify_tmylost_fp(tl_fp),
            FraudProof::WrongTestimony(wt_fp) => self.verify_wrongtmy_fp(wt_fp),
            FraudProof::UnsolvedFault => true,
        }
    }

    fn verify_doublespending_fp(&self, fp: &DoubleSpendingProof) -> bool {
        let shard_id = fp.shard_id as usize;
        let invalid_block = match self.multichain
            .get_block_by_shard(&fp.invalid_block_hash, shard_id) {
            Some(block) => block,
            None => return false,
        };


        let conflict_block = match self.multichain
            .get_block_by_shard(&fp.conflict_block_hash, shard_id) {
            Some(block) => block,
            None => return false,
        };
        //check whether the conflict block is the ancestor of the invalid block
        let history_blocks = match self.multichain
            .all_blocks_end_with_block(&fp.invalid_block_hash) {
            Some(res) => res,
            None => return false,
        };
    
        if !history_blocks.contains(&fp.conflict_block_hash) {
            return false;
        }

        //check the inclusion of conflict tx
        let conflict_tx_merkle_root = conflict_block.get_tx_merkle_root();
        let conflict_tx_hash = fp.conflict_tx.hash();
        if !MerkleTree::<Transaction>::verify(
            &conflict_tx_merkle_root,
            &conflict_tx_hash,
            &fp.conflict_tx_merkle_proof,
            fp.conflict_index as usize,
            self.config.block_size
        ) {
            return false;
        }

        //check whether there are multiple same inputs in one tx
        let mut set: HashMap<H256, bool> = HashMap::new();

        for in_input in fp.invalid_tx.inputs.iter() {
            let in_input_hash = in_input.hash();
            if let Some(_) = set.get(&in_input_hash) {
                return true;
            } else {
                set.insert(in_input_hash.clone(), true);
            }

            for con_input in fp.conflict_tx.inputs.iter() {
                let con_input_hash = con_input.hash();

                if con_input_hash == in_input_hash {
                    return true;
                }
            }
        
        }

        return false;
    }

    fn verify_utxolost_fp(&self, fp: &UtxoLostProof) -> bool {
        //...
        true
    }

    fn verify_wrongshard_fp(&self, fp: &WrongShardProof) -> bool {
        for input in fp.invalid_tx.inputs.iter() {
            let shard_id = Self::get_shard_id(&input.sender_addr, self.config.shard_num as usize) as u32;
            if shard_id == fp.shard_id {
                return false;
            }
        } 

        for output in fp.invalid_tx.outputs.iter() {
            let shard_id = Self::get_shard_id(&output.receiver_addr, self.config.shard_num as usize) as u32;
            if shard_id == fp.shard_id {
                return false;
            }
        }

        true
    } 
    
    fn verify_unequalcoins_fp(&self, fp: &UnequalCoinsProof) -> bool {
        let mut input_coins = 0 as usize;
        for input in fp.invalid_tx.inputs.iter() {
            input_coins += input.value as usize;
        }
        
        let mut output_coins = 0 as usize;
        for output in fp.invalid_tx.outputs.iter() {
            output_coins += output.value as usize;
        }

        input_coins != output_coins
    }

    fn verify_wrongsig_fp(&self, fp: &WrongSignatureProof) -> bool {
        let shard_id = fp.shard_id as usize;
        let invalid_block = match self.multichain
            .get_block_by_shard(&fp.invalid_block_hash, shard_id) {
            Some(block) => block,
            None => return false,
        };
            


        let conflict_block = match self.multichain
            .get_block_by_shard(&fp.conflict_block_hash, shard_id) {
            Some(block) => block,
            None => return false,
        };
        //check whether the conflict block is the ancestor of the invalid block
        let history_blocks = match self.multichain
            .all_blocks_end_with_block(&fp.invalid_block_hash) {
            Some(res) => res,
            None => return false,
        };
        if !history_blocks.contains(&fp.conflict_block_hash) {
            return false;
        }

        //check the inclusion of conflict tx
        let conflict_tx_merkle_root = conflict_block.get_tx_merkle_root();
        let conflict_tx_hash = fp.conflict_tx.hash();
        if !MerkleTree::<Transaction>::verify(
            &conflict_tx_merkle_root,
            &conflict_tx_hash,
            &fp.conflict_tx_merkle_proof,
            fp.conflict_index as usize,
            self.config.block_size
        ) {
            return false;
        }


        for input in fp.invalid_tx.inputs.iter() {
            if input.tx_hash == conflict_tx_hash {
                let sig_vec = input.sig_ref.clone();
                let utxo_index = input.index as usize;
                match fp.conflict_tx.outputs.get(utxo_index) {
                    Some(output) => {
                        let pub_key = output.public_key_ref.clone();
                        return !Transaction::verify(
                            &fp.conflict_tx, 
                            pub_key.as_slice(), 
                            sig_vec.as_slice()
                        ); 
                    }
                    None => return true,
                }
                break;
            }    
        }

        return false;       
    }

    fn verify_tmylost_fp(&self, fp: &TestimonyLostProof) -> bool {
        //...
        true
    }

    fn verify_wrongtmy_fp(&self, fp: &WrongTestimonyProof) -> bool {
        let shard_id = fp.shard_id as usize;
        //check the inclusion 
        let invalid_block = match self.multichain.get_block_by_shard(
            &fp.block_hash,
            shard_id
        ) {
            Some(block) => block,
            None => return false,
        };

        let invalid_tmy_merkle_root = invalid_block.get_testimony_merkle_root();
        let invalid_tmy = fp.invalid_tmy.clone();
        let invalid_tmy_hash = invalid_tmy.hash();
        if !MerkleTree::<Testimony>::verify(
            &invalid_tmy_merkle_root,
            &invalid_tmy_hash,
            &fp.invalid_tmy_merkle_proof,
            fp.invalid_tmy_index as usize,
            self.config.block_size
        ) {
            return false;
        }

        //check the validity of tmy
        if invalid_tmy.get_tx_hash() != fp.invalid_tx.hash() {
            return false;
        }

        for input in fp.invalid_tx.inputs.iter() {
            let input_shard_id = Self::get_shard_id(
                &input.sender_addr, self.config.shard_num
            );
            if input_shard_id != shard_id {
                match self.validate_cross_utxo(
                    &fp.invalid_tx,
                    &input.hash(),
                    &invalid_tmy,
                    input_shard_id,
                    CrossUtxoStatus::Available
                ) {
                    Ok(_) => {},
                    Err(e) => return true,
                }
            }
        }

        return false;

    }

    fn recursive_samples(
        basic_vec: Vec<H256>, 
        samples: &HashMap<usize, Vec<H256>>, 
        index: usize, max_index: usize) -> Vec<Vec<H256>> 
    {
        let possible_samples: Vec<H256> = samples.get(&index).unwrap().clone();
        let mut res: Vec<Vec<H256>> = vec![];
        for sample in possible_samples {
            let mut tmp_vec = basic_vec.clone();
            tmp_vec.push(sample);
            if index == max_index {
                res.push(tmp_vec);
            } else {
                let sub_res_vec = Self::recursive_samples(tmp_vec, samples, index+1, max_index);
                for sub_res in sub_res_vec {
                    res.push(sub_res);
                }
            }
        }
        res 
    }
    pub fn verify_samples(&self, sample_index: &SampleIndex, samples: Vec<Sample>) -> bool {
        return true;
        if samples.len() < self.config.tx_merkle_proof_len {
            return false;
        }
        
        let block_hash = &sample_index.0;
        let tx_index = sample_index.1 as usize;
        let shard_id = sample_index.2 as usize;
        let block: VersaBlock = match self.multichain.get_block_by_shard(&block_hash, shard_id) {
            Some(versa_block) => versa_block,
            None => return false,
        };
        

        let mut sample_hash: HashMap<usize, Vec<H256>> = HashMap::new();
        for sample in samples {
            let unit_index = sample.0 as usize;
            let unit_value = sample.1;

            match sample_hash.get(&unit_index) {
                Some(old_units) => {
                    if !old_units.contains(&unit_value) {
                        let mut new_units = old_units.clone();
                        new_units.push(unit_value);
                        sample_hash.insert(unit_index, new_units);
                    }
                }
                None => {
                    sample_hash.insert(unit_index, vec![unit_value]);
                }
            }
        }

        for i in 0..self.config.tx_merkle_proof_len {
            if let None = sample_hash.get(&i) {
                return false;
            }
        }

        let possible_proofs = Self::recursive_samples(
            vec![], 
            &sample_hash, 
            0, 
            self.config.tx_merkle_proof_len
        );
        let tx_merkle_root = block.get_tx_merkle_root();
        for proof in possible_proofs {
            if MerkleTree::<Transaction>::verify(
                &tx_merkle_root,
                &proof[tx_index],
                &proof,
                tx_index,
                self.config.block_size
            ) {
                return true;
            }
        }       
        return false;
    }

}


