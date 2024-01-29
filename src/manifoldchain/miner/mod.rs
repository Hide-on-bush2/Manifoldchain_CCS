pub mod worker;

use log::{info, debug};
use crossbeam::channel::{
    unbounded, 
    Receiver, 
    Sender, 
    TryRecvError
};
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
    },
};
use rand::Rng;

enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Update, // update the block in mining, it may due to new blockchain tip or new transaction
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    finished_block_chan: Sender<MinerMessage>,
    multichain: Multichain,
    mempool: Arc<Mutex<Mempool>>,
    validator: Validator,
    config: Configuration,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(multichain: &Multichain, 
    mempool: &Arc<Mutex<Mempool>>, 
    config: &Configuration) -> (Context, Handle, Receiver<MinerMessage>) 
{
    let (signal_chan_sender, signal_chan_receiver) = unbounded();
    let (finished_block_sender, finished_block_receiver) = unbounded();

    let validator = Validator::new(multichain, mempool, config);

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        finished_block_chan: finished_block_sender,
        multichain: multichain.clone(),
        mempool: Arc::clone(mempool),
        validator,
        config: config.clone()
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle, finished_block_receiver)
}

pub enum MinerMessage {
    ExFullBlock(ExclusiveFullBlock),
    InFullBlock(InclusiveFullBlock),
    Testimonies(HashMap<usize, Vec<Testimony>>),
    OutputTransactions(HashMap<usize, Vec<Transaction>>),
    GetSamples(Vec<(H256, usize)>),
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

    pub fn update(&self) {
        self.control_chan.send(ControlSignal::Update).unwrap();
    }
}

#[derive(Clone)]
pub enum DoubleSpentType {
    FromStaticState,
    FromDynamicState,
}

impl Context {
    pub fn start(mut self) {

        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
        
    }
//    //need to polish here
//    pub fn start_sample_monitor(mut self) {
//       thread::Builder::new()
//            .name("SampleMonitor".to_string())
//            .spawn(move || {
//                self.monitor_sample();
//            })
//            .unwrap();
//        info!("Sample monitor started");
//    }

    fn create_consensus_block(&self, 
        txs: Vec<Transaction>, 
        tmys: Vec<Testimony>,
        verified_parent: H256,
        inter_parents: Vec<H256>,
        global_parents: Vec<(Vec<H256>, usize)>) -> (ConsensusBlock, TransactionBlock) 

    {
        let shard_id = self.config.shard_id;
        let difficulty = self.config.difficulty.clone();
        let nonce: usize = rand::thread_rng().gen();

        ConsensusBlock::generate(
            verified_parent,
            shard_id,
            nonce,
            difficulty,
            txs,
            tmys,
            inter_parents,
            global_parents
        )        
    }

    fn PoW(&self, con_block: &mut ConsensusBlock, nonce: usize) -> H256 {
        con_block.set_nonce(nonce);
        con_block.hash()
    }

    fn check_complete_testimony(&self, tx: &Transaction, tmy: &Testimony) -> bool {
        match &tx.flag {
            &TxFlag::Output => {
                for input in tx.inputs.iter() {
                    match tmy.get_tmy_unit(&input.hash()) {
                        Some(_) => {}
                        None => {
                            return false;
                        }
                    }
                }
            }
            &TxFlag::Accept => {
                for output in tx.outputs.iter() {
                    match tmy.get_tmy_unit(&output.hash()) {
                        Some(_) => {}
                        None => {
                            return false;
                        }
                    }
                }
            }
            &TxFlag::Reject => {
                for output in tx.outputs.iter() {
                    match tmy.get_tmy_unit(&output.hash()) {
                        Some(_) => {}
                        None => {
                            return false;
                        }
                    }
                }
            }
            _ => {
                return false;
            }
        }
        return true;
    }

//    fn monitor_sample(&mut self) {
//        loop {
//            //check if there are any unverified blocks, if yes, request the samples
//            let unverified_blocks = self.multichain.get_unverified_blocks();
//            if !unverified_blocks.is_empty() {
//                self.finished_block_chan
//                    .send(MinerMessage::GetSamples(unverified_blocks))
//                    .unwrap();
//            } else {
//                //info!("no unverified blocks");
//            }
//            let interval = time::Duration::from_micros(120000000);
//            thread::sleep(interval);
//        }
//    }


    fn miner_loop(&mut self) {
        // check and react to control signals
        // store the hash of parents in the previous round, 
        // if the parents does not change, just need to change the nonce
        // if the parents change, repackage the txs and generate the new consensus block
        let mut pre_verified_parent = H256::default();
        let mut pre_inter_parents = H256::default();
        let mut pre_global_parents = H256::default();
        let mut pre_cons_block = ConsensusBlock::default();
        let mut pre_tx_block = TransactionBlock::default();
        // main mining loop
        loop {
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Miner shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Miner starting in continuous mode with lambda {}", i);
                            self.operating_state = OperatingState::Run(i);
                        }
                        ControlSignal::Update => {
                            // in paused state, don't need to update
                        }
                    };
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        match signal {
                            ControlSignal::Exit => {
                                info!("Miner shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Miner starting in continuous mode with lambda {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Update => {
                                unimplemented!()
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }


            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
 

                let verified_parent = self.multichain.get_longest_verified_fork();
                let inter_parents = self.multichain.get_inter_unverified_forks();
                //assert_eq!(inter_parents.len(), 1);
                let curr_inter_parents = H256::multi_hash(&inter_parents);
                let global_parents = self.multichain.get_global_unverified_forks();
                let tmp_global_parents: Vec<H256> = global_parents
                    .iter()
                    .map(|item| H256::multi_hash(&item.0))
                    .collect();
                let curr_global_parents = H256::multi_hash(&tmp_global_parents);
                let last_blk_hash = self.multichain.get_longest_chain_hash();
                //assert_eq!(verified_parent, last_blk_hash);
                //assert_eq!(inter_parents[0], last_blk_hash);
                //check if parents have been change
                if verified_parent != pre_verified_parent ||
                    curr_inter_parents != pre_inter_parents ||
                    curr_global_parents != pre_global_parents {
                    info!("re-package the block"); 
//                    let mempool_size = self.mempool.lock().unwrap().get_size();
//                    if mempool_size < self.config.block_size {
//                        debug!("Packaging failure: not enough tx, mempool size: {}", mempool_size);
//                        continue;
//                    }
                    ////check if there are any unverified blocks, if yes, request the sample
                    //let unverified_blocks = self.multichain.get_unverified_blocks();
                    //if !unverified_blocks.is_empty() {
                    //    self.finished_block_chan
                    //        .send(MinerMessage::GetSamples(unverified_blocks))
                    //        .unwrap();
                    //}

                    //put all old txs and tmys into mempool
                    let txs = pre_tx_block.get_txs();
                    for tx in txs {
                        if let TxFlag::Empty = tx.flag {
                            continue;
                        }
                        self.mempool.lock().unwrap().insert_tx(tx);
                    }
                    let tmys = pre_tx_block.get_tmys();
                    for (_, tmy) in tmys {
                        self.mempool.lock().unwrap().add_testimony(tmy);
                    }


                    let states = self.multichain.get_states();
                    
                    
                    //parent state

                    let last_state = states
                        .get(&last_blk_hash)
                        .unwrap();
                    //package txs into block
                    let mut counter = 0;
                    let mut txs: Vec<Transaction> = Vec::new();
                    let mut tmys: Vec<Testimony> = Vec::new();

                    let mut invalid_txs: Vec<Transaction> = Vec::new();
                    let mut invalid_tmys: Vec<Testimony> = Vec::new();
                    let mut set: HashMap<H256, bool> = HashMap::new();
                    let mut err_types: Vec<String> = vec![];
                    while counter < self.config.block_size {
                        let (possible_tx, possible_tmy) = self.mempool
                            .lock()
                            .unwrap()
                            .pop_one_tx();
                        match possible_tx {
                            Some(tx) => {
                                if tx.flag == TxFlag::Initial {
                                    counter += 1;
                                    txs.push(tx);
                                    continue;
                                }
                                //check if it is in the chain
                                if let Some(_) = self.multichain.get_tx_in_longest_chain(&tx.hash()) {
                                    invalid_txs.push(tx);
                                    if let Some(tmy) = possible_tmy {
                                        invalid_tmys.push(tmy);
                                    }
                                    err_types.push(String::from("in the longest chain"));
                                    continue;
                                }
                                //check whether all the units in collected
                                if tx.flag == TxFlag::Output ||
                                    tx.flag == TxFlag::Accept ||
                                    tx.flag == TxFlag::Reject {
                                    if let Some(tmy) = possible_tmy.clone() {
                                        if !self.check_complete_testimony(&tx, &tmy) {
                                            invalid_txs.push(tx);
                                            invalid_tmys.push(tmy);
                                            err_types.push(String::from("incomplete tmy"));
                                            continue;
                                        }
                                    } else {
                                        invalid_txs.push(tx);
                                        if let Some(tmy) = possible_tmy {
                                            invalid_tmys.push(tmy);
                                        }
                                        err_types.push(String::from("missing tmy"));
                                        continue;
                                    }
                                }
                                

                                //check the tx's state
                                match self.validator.check_tx_from_state(
                                    &tx, 
                                    possible_tmy.clone(), 
                                    &last_blk_hash,
                                    last_state,
                                )
                                {
                                    Ok(_) => {} 
                                    Err(fp) => {
                                        err_types.push(String::from("invalid state"));
                                        invalid_txs.push(tx);
                                        if let Some(tmy) = possible_tmy {
                                            invalid_tmys.push(tmy);
                                        }
                                        continue;
                                    }
                                }
                                //check whether the double spending occurs inside the block
                                let mut is_inside_double_spent = false;
                                if tx.flag == TxFlag::Input ||
                                    tx.flag == TxFlag::Domestic {
                                    for input in tx.inputs.iter() {
                                        match set.get(&input.hash()) {
                                            Some(_) => {
                                                is_inside_double_spent = true;
                                                break;
                                            }
                                            None => {
                                                set.insert(input.hash(), true);
                                            }
                                        }
                                    }
                                }
                                if is_inside_double_spent {
                                    invalid_txs.push(tx.clone());
                                    debug!("invalid tx hash: {:?}", tx.hash());
                                    debug!("packaged tx hashs:");
                                    for past_tx in txs.iter() {
                                        debug!("{:?}", past_tx.hash());
                                    }
                                    if let Some(tmy) = possible_tmy.clone() {
                                        invalid_tmys.push(tmy);
                                    }
                                    err_types.push(String::from("inside double spending"));
                                    continue;
                                }
                                counter += 1;
                                txs.push(tx);
                                if let Some(tmy) = possible_tmy {
                                    tmys.push(tmy);
                                }
                            } 
                            None => {
                                break;
                            }
                        }
                    }
                    for tx in invalid_txs {
                        self.mempool.lock().unwrap().insert_tx(tx);
                    }
                    for tmy in invalid_tmys {
                        self.mempool.lock().unwrap().add_testimony(tmy);
                    }
                    if counter < self.config.block_size {
//                        //put all txs and tmys back to mempool
//                        for tx in txs {
//                            self.mempool.lock().unwrap().insert_tx(tx);
//                        }
//                        for tmy in tmys {
//                            self.mempool.lock().unwrap().add_testimony(tmy);
//                        }
//                        debug!("{:?}", err_types);
//                        //Update the previous information?
//                        pre_verified_parent = H256::default();
//                        pre_inter_parents = H256::default();
//                        pre_global_parents = H256::default();
//                        continue;
                        
                        //if the txs are not enough, create some empty txs to fill the block
                        for i in 0..self.config.block_size-counter {
                            let empty_tx = Transaction::create_empty_tx(self.config.user_size, self.config.num_tx_recv);
                            txs.push(empty_tx);
                        }
                    }
                    
//                    let (cons_block, tx_block) = self.create_consensus_block(
//                        txs,
//                        tmys,
//                        verified_parent.clone(),
//                        inter_parents.clone(),
//                        global_parents.clone()
//                    );
                    //debug: only one parent in each shard
                    //let mut supposed_global_parents = global_parents.clone();
                    //supposed_global_parents.retain(|x| x.1 != self.config.shard_id );
                    //supposed_global_parents.push((vec![last_blk_hash.clone()], self.config.shard_id));
                    let (cons_block, tx_block) = self.create_consensus_block(
                        txs,
                        tmys,
                        verified_parent.clone(),
                        inter_parents.clone(),
                        global_parents.clone()
                    );
                    //info!("mines a block with parent {:?} of state size: {}", last_blk_hash, last_state.len());
                    //update related information
                    pre_verified_parent = verified_parent.clone();
                    pre_inter_parents = curr_inter_parents.clone();
                    pre_global_parents = curr_global_parents.clone();
                    pre_cons_block = cons_block;
                    pre_tx_block = tx_block;
                }
                
                let nonce: usize = rand::thread_rng().gen();
                let hash_val = self.PoW(&mut pre_cons_block, nonce);
                //info!("block hash: {:?}", hash_val);
                let ex_diff = self.config.difficulty;
                let in_diff = self.config.thredshold;
                //debug: only one parent in each shard
                //let mut supposed_global_parents = global_parents.clone();
                //supposed_global_parents.retain(|x| x.1 != self.config.shard_id );
                //supposed_global_parents.push((vec![last_blk_hash.clone()], self.config.shard_id));
                if hash_val <= ex_diff {
                    let mut final_block: VersaBlock = VersaBlock::default();
                    if hash_val > in_diff {
                        //generate an exclusive block
                        info!("mine an exclusive block {:?} in shard {}", hash_val, self.config.shard_id);
                        let ex_block = ExclusiveBlock::create(
                            pre_cons_block.clone(),
                            hash_val,
                            inter_parents.clone(),
                        );
                        let ex_full_block = ExclusiveFullBlock::create(
                            ex_block,
                            pre_tx_block.clone(),
                        );
                        final_block = VersaBlock::ExFullBlock(ex_full_block.clone());
                        self.finished_block_chan
                            .send(MinerMessage::ExFullBlock(ex_full_block))
                            .unwrap();
                        //leave the job of inserting new blocks to the workers
                    } else {
                        //generate an inclusive block
                        //let parents: Vec<(H256, u32)> = chains_hash_shard_id
                        //    .into_iter()
                        //    .map(|x| (x.0.clone(), x.1 as u32))
                        //    .collect();
                        info!("mine an inclusive block {} in shard {}", hash_val, self.config.shard_id);
                        let in_block = InclusiveBlock::create(
                            pre_cons_block.clone(),
                            hash_val,
                            inter_parents,
                            global_parents,
                        );
                        let in_full_block = InclusiveFullBlock::create(
                            in_block,
                            pre_tx_block.clone(),
                        );
                        final_block = VersaBlock::InFullBlock(in_full_block.clone());
                        self.finished_block_chan
                            .send(MinerMessage::InFullBlock(in_full_block))
                            .unwrap();
                    }

                    //generate testimony
                    let mut new_tmys: HashMap<usize, Vec<Testimony>> = HashMap::new();
                    let mut new_output_txs: HashMap<usize, Vec<Transaction>> = HashMap::new();
                    let txs = pre_tx_block.get_txs_ref();
                    for tx_index in 0..txs.len() {
                        let tx = &txs[tx_index];
                        let mut output_shards: Vec<usize> = vec![];
                       
                        if tx.flag == TxFlag::Input {
                            for output in tx.outputs.iter() {
                                let output_shard_id = Validator::get_shard_id(
                                    &output.receiver_addr, 
                                    self.config.shard_num
                                );
                                output_shards.push(output_shard_id);
                            }
                            let tmy = Testimony::generate(
                                tx,
                                &final_block,
                                tx_index,
                                self.config.shard_id,
                                self.config.shard_num,
                                true
                            ).unwrap();
                            let mut output_tx = tx.clone();
                            output_tx.flag = TxFlag::Output;
                            //debug
                            //let mut output_tx = tx.clone();
                            //output_tx.flag = TxFlag::Output;
                            //debug!("checking consistent tx hash here");
                            //assert_eq!(tmy.get_tx_hash(), output_tx.hash());
                            for shard in output_shards {
                                match new_tmys.get(&shard) {
                                    Some(old_elements) => {
                                        let mut new_elements = old_elements.clone();
                                        new_elements.push(tmy.clone());
                                        new_tmys.insert(shard, new_elements);
                                    }
                                    None => {
                                        new_tmys.insert(shard, vec![tmy.clone()]);
                                    }
                                }
                                match new_output_txs.get(&shard) {
                                    Some(old_txs) => {
                                        let mut new_txs = old_txs.clone();
                                        new_txs.push(output_tx.clone());
                                        new_output_txs.insert(shard, new_txs);
                                    }
                                    None => {
                                        new_output_txs.insert(shard, vec![]);
                                    }
                                }
                                //add the current node's mempool
                                if shard == self.config.shard_id {
                                    self.mempool.lock().unwrap().add_testimony(tmy.clone());
                                    self.mempool.lock().unwrap().insert_tx(output_tx.clone());
                                }
                            }
                        } else if tx.flag == TxFlag::Output {
                             for input in tx.inputs.iter() {
                                let input_shard_id = Validator::get_shard_id(
                                    &input.sender_addr, 
                                    self.config.shard_num
                                );
                                output_shards.push(input_shard_id);
                            }
                            let tmy = Testimony::generate(
                                tx,
                                &final_block,
                                tx_index,
                                self.config.shard_id,
                                self.config.shard_num,
                                true
                            ).unwrap();
                            for shard in output_shards {
                                match new_tmys.get(&shard) {
                                    Some(old_elements) => {
                                        let mut new_elements = old_elements.clone();
                                        new_elements.push(tmy.clone());
                                        new_tmys.insert(shard, new_elements);
                                    }
                                    None => {
                                        new_tmys.insert(shard, vec![tmy.clone()]);
                                    }
                                }
                                if shard == self.config.shard_id {
                                    self.mempool.lock().unwrap().add_testimony(tmy.clone());
                                }
                            }                           
                        }

                    }
                    
                    self.finished_block_chan
                        .send(MinerMessage::Testimonies(new_tmys))
                        .unwrap();
                    self.finished_block_chan
                        .send(MinerMessage::OutputTransactions(new_output_txs))
                        .unwrap();
                    //update the previous information
                    pre_verified_parent = H256::default();
                    pre_inter_parents = H256::default();
                    pre_global_parents = H256::default();
                    pre_cons_block = ConsensusBlock::default();
                    pre_tx_block = TransactionBlock::default();

                }

                
            }
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST


