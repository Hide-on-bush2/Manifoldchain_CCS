use crate::{
    types::{
        hash::{H256, Hashable},
    },
    manifoldchain::{
        network::{
            message::Message,
            peer,
            server::Handle as ServerHandle,
        },
        transaction::{Transaction},
        block::{
            Info, 
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
            versa_block::{
                VersaBlock,
                VersaHash,
                ExclusiveFullBlock,
                InclusiveFullBlock,
            }
        },
        configuration::Configuration,
        validator::{Validator, ValidationSource},
        mempool::Mempool,
        multichain::Multichain,
        testimony::Testimony,
        fraudproof::FraudProof,
        confirmation::Confirmation,
    }
};
use log::{debug, warn, error, info};
use std::{
    time::{self, SystemTime},
    thread,
    sync::{Arc,Mutex},
    collections::HashMap,
};
use rand::Rng;

//#[cfg(any(test,test_utilities))]
//use super::peer::TestReceiver as PeerTestReceiver;
//#[cfg(any(test,test_utilities))]
//use super::server::TestReceiver as ServerTestReceiver;
#[derive(Clone)]
pub struct Worker {
    msg_chan: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    multichain: Multichain,
    blk_buff: HashMap<H256, VersaBlock>,
    fp_map: HashMap<H256, FraudProof>,
    //block_hash -> fp, upon receiving a new block, after inserting it, check wherther
    //there is an associated fp, it there is, prune it immediately
    blk2fp: HashMap<H256, FraudProof>, 
    sample_map: HashMap<SampleIndex, Vec<Sample>>,
    blk2sample: HashMap<H256, Vec<SampleIndex>>,
    mempool: Arc<Mutex<Mempool>>,
    config: Configuration,
    validator: Validator,
    confirmation: Arc<Mutex<Confirmation>>,
}

pub type SampleIndex = (H256, u32, u32); //block_hash, tx_index, shard_id
pub type Sample = (u32, H256);

impl Worker {
    pub fn new(
        num_worker: usize,
        msg_src: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
        server: &ServerHandle,
        multichain: &Multichain,
        mempool: &Arc<Mutex<Mempool>>,
        config: &Configuration,
        confirmation: &Arc<Mutex<Confirmation>>,
    ) -> Self {
        let validator = Validator::new(multichain, mempool, config);
        Self {
            msg_chan: msg_src,
            num_worker,
            server: server.clone(),
            multichain: multichain.clone(),
            blk_buff: HashMap::new(),
            mempool: Arc::clone(mempool),
            config: config.clone(),
            validator,
            fp_map: HashMap::new(),
            sample_map: HashMap::new(),
            blk2sample: HashMap::new(),
            confirmation: Arc::clone(confirmation),
            blk2fp: HashMap::new(),
        }
    }

    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let mut cloned = self.clone();
            thread::Builder::new()
                .name("network-worker".to_string())
                .spawn(move || {
                    cloned.worker_loop();
                    warn!("Worker thread {} exited", i);
            }).unwrap();
        }
    }


    fn worker_loop(&mut self) {
        loop {
            let result = smol::block_on(self.msg_chan.recv());
            if let Err(e) = result {
                error!("network worker terminated {}", e);
                break;
            }
            let msg = result.unwrap();
            let (msg, mut peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                Message::NewTransactionHash((tx_hashs, shard_id)) => {
                    //info!("New transaction hashs");
                    if let Some(response) = self
                        .handle_new_transaction_hash(tx_hashs, shard_id as usize) {
                        peer.write(response);
                    } 
                }
                Message::GetTransactions((tx_hashs, shard_id)) => {
                    //info!("Get transactions");
                    if let Some(response) = self
                        .handle_get_transactions(tx_hashs, shard_id as usize) {
                        peer.write(response);
                    } 
                }
                Message::Transactions((txs, shard_id)) => {
                    //info!("Comming transactions");
                    if let Some(response) = self.handle_transactions(txs, shard_id as usize) {
                        if let Message::NewTransactionHash((tx_hashs, shard_id)) = response {
                            self.server.broadcast_with_shard(
                                Message::NewTransactionHash((tx_hashs, shard_id)),
                                shard_id as usize,
                            )
                        } 
                    }
                }
//                Message::NewBlockHash(hash_vec) => {
//                    debug!("New block hash");
//                    if let Some(response) = self
//                        .handle_new_block_hash(hash_vec) {
//                        peer.write(response);
//                    }
//                }
//                Message::GetBlocks(hash_vec) => {
//                    debug!("Get blocks");
//                    if let Some(response) = self
//                        .handle_get_blocks(hash_vec) {
//                        peer.write(response);
//                    }
//                }
//                Message::Blocks(blocks) => {
//                    debug!("Coming Blocks");
//                    if let Some(response) = self.
//                        handle_blocks(blocks) {
//                        self.server.broadcast(response);
//                    }
//                }
                //Exclusive
                Message::NewExBlockHash((ex_hash_vec, shard_id)) => {
                    info!("New exclusive block hash");
                    let versa_hash_vec = ex_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::ExHash(x) )
                        .collect();
                    if let Some(response) = self
                        .handle_new_block_hash(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::GetExBlocks((ex_hash_vec, shard_id)) => {
                    info!("Get exclusive blocks");
                    let versa_hash_vec = ex_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::ExHash(x))
                        .collect();
                    if let Some(response) = self
                        .handle_get_blocks(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::ExBlocks((ex_blocks, shard_id)) => {
                    info!("Coming exclusive blocks");
                    let versa_blocks = ex_blocks
                        .into_iter()
                        .map(|x| VersaBlock::ExBlock(x))
                        .collect();
                    let (response_1, response_2, response_3, response_4, response_5) = self
                        .handle_blocks(versa_blocks, shard_id as usize); 
                    if let Some(res_1) = response_1 {
                        self.server.broadcast(res_1.clone());
                        //Request the samples from peer 
                        if let Message::NewExBlockHash(new_ex_blocks) = res_1 {
                            let mut rq_samples: Vec<SampleIndex> = vec![];
                            for block_hash in new_ex_blocks.0 {
                                match self.blk2sample.get(&block_hash) {
                                    Some(sample_index_vec) => {
                                        for sample_index in sample_index_vec.iter() {
                                            let samples = self.sample_map
                                                .get(&sample_index)
                                                .unwrap()
                                                .clone();
                                            if self.validator.verify_samples(
                                                &sample_index, 
                                                samples   
                                            ) {
                                                let _ = self.multichain.verify_block_with_shard(
                                                    &block_hash,
                                                    shard_id as usize
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                    None => {},
                                }                          
                                let mut rng = rand::thread_rng();

                                let tx_index: usize = rng
                                    .gen_range(0..self.config.block_size);
                                rq_samples.push((block_hash, tx_index as u32, shard_id as u32));
                            }
                            peer.write(Message::GetSamples(rq_samples));
                        } 
                    }
                    if let Some(res_2) = response_2 {
                        self.server.broadcast(res_2);
                    }
                    //handling return transactions
                    if let Some(res_3) = response_3 {
                        for message in res_3 {
                            if let Message::Transactions((txs, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Transactions((txs, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }

                    //handling return testimonies
                    if let Some(res_4) = response_4 {
                        for message in res_4 {
                            if let Message::Testimonies((tmys, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Testimonies((tmys, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }

                    //handle missing blocks
                    if let Some(res_5) = response_5 {
                        for message in res_5 {
                            peer.write(message);
                        }
                    }
                }
                //Inclusive
                Message::NewInBlockHash((in_hash_vec, shard_id)) => {
                    info!("New inclusive block hash");
                    let versa_hash_vec = in_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::InHash(x))
                        .collect();
                    if let Some(response) = self
                        .handle_new_block_hash(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::GetInBlocks((in_hash_vec, shard_id)) => {
                    info!("Get inclusive blocks");
                    let versa_hash_vec = in_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::InHash(x))
                        .collect();
                    if let Some(response) = self
                        .handle_get_blocks(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::InBlocks((in_blocks, shard_id)) => {
                    info!("Coming inclusive blocks");
                    let versa_blocks = in_blocks
                        .into_iter()
                        .map(|x| VersaBlock::InBlock(x))
                        .collect();
                    let (response_1, response_2, response_3, response_4, response_5) = self
                        .handle_blocks(versa_blocks, shard_id as usize); 
                    if let Some(res_1) = response_1 {
                        self.server.broadcast(res_1.clone());
                        //Request the samples from peer 
                        if let Message::NewInBlockHash(new_in_blocks) = res_1 {
                            let mut rq_samples: Vec<SampleIndex> = vec![];
                            for block_hash in new_in_blocks.0 {
                                match self.blk2sample.get(&block_hash) {
                                    Some(sample_index_vec) => {
                                        for sample_index in sample_index_vec.iter() {
                                            let samples = self.sample_map
                                                .get(&sample_index)
                                                .unwrap()
                                                .clone();
                                            if self.validator.verify_samples(
                                                &sample_index, 
                                                samples   
                                            ) {
                                                let _ = self.multichain.verify_block_with_shard(
                                                    &block_hash,
                                                    shard_id as usize
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                    None => {},
                                } 
                                
                                let mut rng = rand::thread_rng();

                                let tx_index: usize = rng
                                    .gen_range(0..self.config.block_size);
                                rq_samples.push((block_hash, tx_index as u32, shard_id as u32));
                            }
                            peer.write(Message::GetSamples(rq_samples));
                        } 

                    }
                    if let Some(res_2) = response_2 {
                        self.server.broadcast(res_2);
                    }
                    if let Some(res_3) = response_3 {
                        for message in res_3 {
                            if let Message::Transactions((txs, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Transactions((txs, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                    if let Some(res_4) = response_4 {
                        for message in res_4 {
                            if let Message::Testimonies((tmy, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Testimonies((tmy, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                    //handle missing blocks
                    if let Some(res_5) = response_5 {
                        for message in res_5 {
                            peer.write(message);
                        }
                    }
                 }
                //ExclusiveFull
                Message::NewExFullBlockHash((ex_full_hash_vec, shard_id)) => {
                    info!("New exclusive full block hash");
                    let versa_hash_vec = ex_full_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::ExFullHash(x))
                        .collect();
                    if let Some(response) = self
                        .handle_new_block_hash(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::GetExFullBlocks((ex_full_hash_vec, shard_id)) => {
                    info!("Get exclusive full blocks");
                    let versa_hash_vec = ex_full_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::ExFullHash(x))
                        .collect();
                    if let Some(response) = self
                        .handle_get_blocks(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::ExFullBlocks((ex_full_blocks, shard_id)) => {
                    info!("Coming exclusive full blocks");
                    if self.config.network_delay != 0 {
                        let interval = time::Duration::from_micros(self.config.network_delay as u64);
                        thread::sleep(interval);
                    }
                    //debug!("Coming exclusive full blocks");
                    let versa_blocks = ex_full_blocks
                        .into_iter()
                        .map(|x| VersaBlock::ExFullBlock(x))
                        .collect();
                    let (response_1, response_2, response_3, response_4, response_5) = self
                        .handle_blocks(versa_blocks, shard_id as usize); 
                    if let Some(res_1) = response_1 {
                        self.server.broadcast_with_shard(res_1, shard_id as usize);
                    }
                    if let Some(res_2) = response_2 {
                        self.server.broadcast(res_2);
                    }
                    if let Some(res_3) = response_3 {
                        for message in res_3 {
                            if let Message::Transactions((txs, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Transactions((txs, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                    if let Some(res_4) = response_4 {
                        for message in res_4 {
                            if let Message::Testimonies((tmys, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Testimonies((tmys, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                    //handle missing blocks
                    if let Some(res_5) = response_5 {
                        for message in res_5 {
                            peer.write(message);
                        }
                    }
                }
                //InclusiveFull
                Message::NewInFullBlockHash((in_full_hash_vec, shard_id)) => {
                    info!("New inclusive full block hash");
                    let versa_hash_vec = in_full_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::InFullHash(x))
                        .collect();
                    if let Some(response) = self
                        .handle_new_block_hash(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::GetInFullBlocks((in_full_hash_vec, shard_id)) => {
                    info!("Get inclusive full blocks");
                    let versa_hash_vec = in_full_hash_vec
                        .into_iter()
                        .map(|x| VersaHash::InFullHash(x))
                        .collect();
                    if let Some(response) = self
                        .handle_get_blocks(versa_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::InFullBlocks((in_full_blocks, shard_id)) => {
                    info!("Coming inclusive full blocks");
                    if self.config.network_delay != 0 {
                        let interval = time::Duration::from_micros(self.config.network_delay as u64);
                        thread::sleep(interval);
                    }
                    //debug!("Coming inclusive full blocks");
                    let versa_blocks = in_full_blocks
                        .into_iter()
                        .map(|x| VersaBlock::InFullBlock(x))
                        .collect();
                    let (response_1, response_2, response_3, response_4, response_5) = self
                        .handle_blocks(versa_blocks, shard_id as usize); 
                    if let Some(res_1) = response_1 {
                        self.server.broadcast_with_shard(res_1, shard_id as usize);
                    }
                    if let Some(res_2) = response_2 {
                        self.server.broadcast(res_2);
                    }
                    if let Some(res_3) = response_3 {
                        for message in res_3 {
                            if let Message::Transactions((txs, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Transactions((txs, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                    if let Some(res_4) = response_4 {
                        for message in res_4 {
                            if let Message::Testimonies((tmys, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Testimonies((tmys, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                    //handle missing blocks
                    if let Some(res_5) = response_5 {
                        for message in res_5 {
                            peer.write(message);
                        }
                    }
                }
                //Testimony
                Message::NewTestimonyHash((tmy_hash_vec, shard_id)) => {
                    //info!("New testimony hash");
                    if let Some(response) = self
                        .handle_new_testimony_hash(tmy_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::GetTestimonies((tmy_hash_vec, shard_id)) => {
                    //info!("Get testimonies");
                    if let Some(response) = self
                        .handle_get_testimonies(tmy_hash_vec, shard_id as usize) {
                        peer.write(response);
                    }
                }
                Message::Testimonies((tmys, shard_id)) => {
                    //info!("Testimonies");
                    if let Some(response) = self
                        .handle_testimonies(tmys, shard_id as usize) {
                        self.server.broadcast(response);
                    }
                }
                //FraudProof
                Message::NewFraudProofHash(fp_hash_vec) => {
                    //info!("New Fraud Proofs");
                    if let Some(response) = self
                        .handle_new_fraud_proof_hash(fp_hash_vec) {
                        peer.write(response);
                    }
                }
                Message::GetFraudProofs(fp_hash_vec) => {
                    //info!("Get Fraud Proofs");
                    if let Some(response) = self
                        .handle_get_fraud_proofs(fp_hash_vec) {
                        peer.write(response);
                    }
                }
                Message::FraudProofs(fps) => {
                    //info!("Coming Fraud Proofs");
                    if let Some(response) =
                        self.handle_fraud_proofs(fps) {
                        self.server.broadcast(response);
                    }
                }
                Message::NewSamples(sample_info) => {
                    //info!("New Samples");
                    if let Some(response) = self
                        .handle_new_samples(sample_info) {
                        peer.write(response);
                    }
                }
                Message::GetSamples(sample_info) => {
                    //info!("Get Samples");
                    if let Some(response) = self
                        .handle_get_samples(sample_info) {
                        peer.write(response);
                    }
                }
                Message::Samples(samples) => {
                    //info!("Coming Samples");
                    let (response_1, response_2, response_3) = self
                        .handle_samples(samples);
                    if let Some(res_1) = response_1 {
                        self.server.broadcast(res_1);
                    }
                    if let Some(res_2) = response_2 {
                        for message in res_2 {
                            if let Message::Transactions((txs, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Transactions((txs, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                    if let Some(res_3) = response_3 {
                        for message in res_3 {
                            if let Message::Testimonies((tmys, shard_id)) = message {
                                self.server.broadcast_with_shard(
                                    Message::Testimonies((tmys, shard_id)),
                                    shard_id as usize
                                );
                            }
                        }
                    }
                }
                Message::NewMissBlockHash((miss_blk_vec, shard_id)) => {
                    info!("Coming new missing block hash");
                    for blk in miss_blk_vec {
                        match self.multichain.get_block_by_shard(
                            &blk,
                            shard_id as usize
                        ) {
                            Some(versa_block) => {
                                match versa_block {
                                    VersaBlock::ExBlock(ex_block) => {
                                        peer.write(Message::ExBlocks((vec![ex_block], shard_id)));
                                    }
                                    VersaBlock::InBlock(in_block) => {
                                        peer.write(Message::InBlocks((vec![in_block], shard_id)));
                                    }
                                    VersaBlock::ExFullBlock(ex_full_block) => {
                                        let ex_block = ex_full_block.get_exclusive_block();
                                        peer.write(Message::ExBlocks((vec![ex_block], shard_id)));
                                        peer.write(Message::ExFullBlocks((vec![ex_full_block], shard_id)));
                                    }
                                    VersaBlock::InFullBlock(in_full_block) => {
                                        let in_block = in_full_block.get_inclusive_block();
                                        peer.write(Message::InBlocks((vec![in_block], shard_id)));
                                        peer.write(Message::InFullBlocks((vec![in_full_block], shard_id)));        
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                }
                _ => unimplemented!()
            }
        }
    }
   
    //handle transaction message
    fn handle_new_transaction_hash(
        &self, 
        tx_hashs: Vec<H256>, 
        shard_id: usize) -> Option<Message> 
    {
        if shard_id != self.config.shard_id {
            return None;
        }
        let mut unreceived_txs: Vec<H256> = Vec::new();
        for tx_hash in tx_hashs.iter() {
            if self.mempool.lock().unwrap().check(tx_hash) {
                continue;
            }
            if let Some(_) = self.multichain
                .get_tx_in_longest_chain(tx_hash) {
                continue;
            }
            unreceived_txs.push(tx_hash.clone());
        }
        if !unreceived_txs.is_empty() {
            Some(Message::GetTransactions((unreceived_txs, shard_id as u32)))
        } else {
            None
        }
    }
    fn handle_get_transactions(
        &self, 
        tx_hashs: Vec<H256>, 
        shard_id: usize) -> Option<Message> 
    {
        if shard_id != self.config.shard_id {
            return None;
        }
        let mut res_txs: Vec<Transaction> = Vec::new();
        for tx_hash in tx_hashs.iter() {
            //find tx in mempool
            if let Some(tx) = self.mempool.lock().unwrap().get_tx(tx_hash) {
                res_txs.push(tx);
                continue;
            }
            //find tx in blockchain
            if let Some(tx) = self.multichain.get_tx_in_longest_chain(tx_hash) {
                res_txs.push(tx);
            }
        }
        if !res_txs.is_empty() {
            Some(Message::Transactions((res_txs, shard_id as u32)))
        } else {
            None
        }
    }
    fn handle_transactions(
        &self, 
        txs: Vec<Transaction>, 
        shard_id: usize) -> Option<Message> 
    {
        if shard_id != self.config.shard_id {
            return None;
        }
        let mut new_tx_hashs: Vec<H256> = Vec::new();
        for tx in txs.iter() {
            //find tx in mempool
            let tx_hash = tx.hash();
            if let Some(tx) = self.mempool.lock().unwrap().get_tx(&tx_hash) {
                continue;
            }
            //2.validate the transaction
            match self.validator.validate_tx(tx, None, None, ValidationSource::FromTransaction) {
                Ok(_) => {}
                Err(_) => {
                    continue;
                }
            }
            new_tx_hashs.push(tx_hash);
            self.mempool.lock().unwrap().insert_tx(tx.clone());
        }
        if !new_tx_hashs.is_empty() {
            Some(Message::NewTransactionHash((new_tx_hashs, shard_id as u32)))
        } else {
            None
        }
    }
    fn handle_new_block_hash(
        &self, 
        block_hash_vec: Vec<VersaHash>, 
        shard_id: usize) -> Option<Message> 
    {
        if block_hash_vec.is_empty() {
            return None;
        }
        let first_hash = block_hash_vec[0].clone();

        let mut unreceived_blks: Vec<H256> = vec![];

        match first_hash {
            VersaHash::ExHash(_) => {
                //miner never accept exclusive blocks within his shard
                if shard_id != self.config.shard_id {
                    for versa_block_hash in block_hash_vec {
                        if let VersaHash::ExHash(ex_block_hash) = versa_block_hash {
                            match self.multichain.get_block_by_shard(
                                &ex_block_hash,
                                shard_id
                            ) {
                                Some(_) => {}
                                None => unreceived_blks.push(
                                    ex_block_hash
                                ),
                            }
                        }
                    }
                }
            }
            VersaHash::InHash(_) => {
                for versa_block_hash in block_hash_vec {
                    if let VersaHash::InHash(in_block_hash) = versa_block_hash {
                        let mut is_found = false;
                        for id in 0..self.config.shard_num {
                            match self.multichain.get_block_by_shard(
                                &in_block_hash,
                                id
                            ){
                                Some(_) => {
                                    is_found = true;
                                    break;
                                }
                                None => {}
                            }
                        }
                        if !is_found {
                            unreceived_blks.push(
                                in_block_hash
                            );
                        }
                    }
                }
            }
            VersaHash::ExFullHash(_) => {
                //miner only accept full block within this shard
                if shard_id == self.config.shard_id {
                    for versa_block_hash in block_hash_vec {
                        if let VersaHash::ExFullHash(ex_full_block_hash) = versa_block_hash {
                            match self.multichain.get_block_by_shard(
                                &ex_full_block_hash,
                                shard_id
                            ){
                                Some(_) => {}
                                None => {
                                    unreceived_blks.push(ex_full_block_hash);
                                }
                            }
                        }
                    }
                }
            }
            VersaHash::InFullHash(_) => {
                //miner only accept full blocks within his shard
                if shard_id == self.config.shard_id {
                    for versa_block_hash in block_hash_vec {
                        if let VersaHash::InFullHash(in_full_block_hash) = versa_block_hash {
                            match self.multichain.get_block_by_shard(
                                &in_full_block_hash,
                                shard_id
                            ){
                                Some(_) => {}
                                None => {
                                    unreceived_blks.push(in_full_block_hash);
                                }
                            }
                        }
                    }
                }
            }
        }

        if !unreceived_blks.is_empty() {
            match first_hash {
                VersaHash::ExHash(_) => Some(Message::GetExBlocks((unreceived_blks, shard_id as u32))),
                VersaHash::InHash(_) => Some(Message::GetInBlocks((unreceived_blks, shard_id as u32))),
                VersaHash::ExFullHash(_) => Some(Message::GetExFullBlocks((unreceived_blks, shard_id as u32))),
                VersaHash::InFullHash(_) => Some(Message::GetInFullBlocks((unreceived_blks, shard_id as u32))),
            }
        } else {
            None
        }
    }

    fn handle_get_blocks(&self, hash_vec: Vec<VersaHash>, shard_id: usize) 
        -> Option<Message>
    {
        if hash_vec.is_empty() {
            return None;
        }
        let first_hash = hash_vec[0].clone();

        let mut res_blks: Vec<VersaBlock> = vec![];
        
        match first_hash {
            VersaHash::ExHash(_) => {
                //miner never accept exclusive blocks within his shard
                if shard_id != self.config.shard_id {
                    for versa_hash in hash_vec {
                        if let VersaHash::ExHash(ex_hash) = versa_hash {
                            match self.multichain.get_block_by_shard(
                                &ex_hash,
                                shard_id
                            ){
                                Some(block) => res_blks.push(block),
                                None => {}
                            }
                        }
                    }
                }
            }
            VersaHash::InHash(_) => {
                for versa_hash in hash_vec {
                    if let VersaHash::InHash(in_hash) = versa_hash {
                        for id in 0..self.config.shard_num {
                            match self.multichain.get_block_by_shard(
                                &in_hash, 
                                id
                            ){
                                Some(block) => {
                                    res_blks.push(block);
                                    break;
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
            VersaHash::ExFullHash(_) => {
                //miner does not have full blocks outside his shard
                if shard_id == self.config.shard_id {
                    for versa_hash in hash_vec {
                        if let VersaHash::ExFullHash(ex_full_hash) = versa_hash {
                            match self.multichain.get_block_by_shard(
                                &ex_full_hash,
                                shard_id
                            ) {
                                Some(block) => {
                                    res_blks.push(block);
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
            VersaHash::InFullHash(_) => {
                //miner only accept full blocks within his shard
                if shard_id == self.config.shard_id {
                    for versa_hash in hash_vec {
                        if let VersaHash::InFullHash(in_full_hash) = versa_hash {
                            match self.multichain.get_block_by_shard(
                                &in_full_hash,
                                shard_id
                            ){
                                Some(block) => {
                                    res_blks.push(block);
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
        }

        if !res_blks.is_empty() {
            match first_hash {
                VersaHash::ExHash(_) => {
                    let mut ex_blocks: Vec<ExclusiveBlock> = vec![];
                    for versa_blk in res_blks {
                        match versa_blk {
                            VersaBlock::ExBlock(ex_block) 
                                => ex_blocks.push(ex_block),
                            _ => {}
                        }
                    }
                    Some(Message::ExBlocks((ex_blocks, shard_id as u32)))
                }
                VersaHash::InHash(_) => {
                    let mut in_blocks: Vec<InclusiveBlock> = vec![];
                    for versa_blk in res_blks {
                        match versa_blk {
                            VersaBlock::InBlock(in_block) 
                                => in_blocks.push(in_block),
                            _ => {}
                        }
                    }
                    Some(Message::InBlocks((in_blocks, shard_id as u32)))
                }
                VersaHash::ExFullHash(_) => {
                    let mut ex_full_blocks: Vec<ExclusiveFullBlock> = vec![];
                    for versa_blk in res_blks {
                        match versa_blk {
                            VersaBlock::ExFullBlock(ex_full_block) 
                                => ex_full_blocks.push(ex_full_block),
                            _ => {}
                        }
                    }
                    Some(Message::ExFullBlocks((ex_full_blocks, shard_id as u32)))
                }
                VersaHash::InFullHash(_) => {
                    let mut in_full_blocks: Vec<InclusiveFullBlock> = vec![];
                    for versa_blk in res_blks {
                        match versa_blk {
                            VersaBlock::InFullBlock(in_full_block) 
                                => in_full_blocks.push(in_full_block),
                            _ => {}
                        }
                    }
                    Some(Message::InFullBlocks((in_full_blocks, shard_id as u32)))
                }
            }
        } else {
            None
        }
    }

    fn handle_blocks(&mut self, blocks: Vec<VersaBlock>, shard_id: usize) 
        -> (Option<Message>, Option<Message>, Option<Vec<Message>>, Option<Vec<Message>>, Option<Vec<Message>>) 
    //new_block_hash, fraud_proof_hash, return_tx, return_tmy
    {
        if blocks.is_empty() {
            return (None, None, None, None, None);
        }
        for block in blocks.iter() {
            info!("Comming block {:?} in shard {}", block.hash(), self.config.shard_id);
        }
        let first_block = blocks[0].clone();
         match first_block.clone() {
            VersaBlock::ExBlock(ex_block) => {
                if ex_block.get_shard_id() == self.config.shard_id {
                    return (None, None, None, None, None);
                }
            }
            VersaBlock::InBlock(_) => {
                
            }
            VersaBlock::ExFullBlock(_) => {
                if shard_id != self.config.shard_id {
                    return (None, None, None, None, None);
                }
            }
            VersaBlock::InFullBlock(_) => {
                if shard_id != self.config.shard_id {
                    return (None, None, None, None, None);
                }
            }
        } 
        for block in blocks.iter() {
            info!("Handling block {:?} in shard {}", block.hash(), self.config.shard_id);
        }       
        //key: hash of fraud_proof, value: shard_id
        let mut fraud_proofs: HashMap<FraudProof, usize> = HashMap::new();
        //key: hash of the block, value: shard_id
        let mut new_hashs: HashMap<H256, usize> = HashMap::new();
        // return tx
        let mut return_txs_tmys: Vec<(Transaction, Testimony, Vec<usize>)> = vec![];
        let mut missing_parents: HashMap<usize, Vec<H256>> = HashMap::new();
        for block in blocks {
            //verification

            
            //check whether the parent exits
            let parents: Vec<(H256, usize)> = match block.clone() {
                VersaBlock::ExBlock(ex_block) => {
                    ex_block
                        .get_inter_parents()
                        .into_iter()
                        .map(|x| (x.clone(), ex_block.get_shard_id()))
                        .collect()
                }
                VersaBlock::ExFullBlock(ex_full_block) => {
                    ex_full_block
                        .get_inter_parents()
                        .into_iter()
                        .map(|x| (x.clone(), ex_full_block.get_shard_id()))
                        .collect()
                }
                VersaBlock::InBlock(in_block) => {

                    let global_parents = in_block.get_global_parents();
                    let mut unzip_global_parents: Vec<(H256, usize)> = vec![];
                    for item in global_parents {
                        let inter_parents = item.0;
                        let shard_id = item.1;
                        for inter_parent in inter_parents {
                            unzip_global_parents.push((inter_parent, shard_id));
                        }
                    }
                    unzip_global_parents
                 }
                 VersaBlock::InFullBlock(in_full_block) => {
                    //let global_parents = in_full_block.get_global_parents();
                    //let mut unzip_global_parents: Vec<(H256, usize)> = vec![];
                    //for item in global_parents {
                    //    let inter_parents = item.0;
                    //    let shard_id = item.1;
                    //    for inter_parent in inter_parents {
                    //        unzip_global_parents.push((inter_parent, shard_id));
                    //    }
                    //}
                    //unzip_global_parents
                    in_full_block
                        .get_inter_parents()
                        .into_iter()
                        .map(|x| (x.clone(), in_full_block.get_shard_id()))
                        .collect()
                 }              
            };
            
            for item in parents {
                let parent_hash = item.0;
                let inserted_shard_id = item.1;
                //this is important
                //the inclusive block can not be inserted in his own shard
                if let VersaBlock::InBlock(_) = block {
                    if inserted_shard_id == self.config.shard_id &&
                        block.get_shard_id() == self.config.shard_id {
                        continue;
                    }
                }
                
                //check whether the parent exits
                match self.multichain.get_block_by_shard(&parent_hash, inserted_shard_id) {
                    Some(_) => {}
                    None => {
                        self.blk_buff.insert(
                            parent_hash.clone(),
                            block.clone()
                        );
                        info!("block insertion failure in shard {}: parent {:?} not fould", inserted_shard_id, parent_hash);
                        match missing_parents.get(&inserted_shard_id) {
                            Some(old_elements) => {
                                let mut new_elements = old_elements.clone();
                                new_elements.push(parent_hash.clone());
                                missing_parents.insert(inserted_shard_id, new_elements);
                            }
                            None => {
                                missing_parents.insert(
                                    inserted_shard_id, 
                                    vec![parent_hash.clone()]
                                );
                            }
                        }
                        continue;
                        //debug!("Put block into buff: parent not found");
                    }
                }
                match self.validator.validate_block(&block, &parent_hash) {
                    Ok(_) => {}
                    Err(proof) => {
                        //match &block {
                        //    &VersaBlock::ExBlock(_) => {
                        //        info!("Ex");
                        //    }
                        //    &VersaBlock::InBlock(_) => {
                        //        info!("In");
                        //    }
                        //    &VersaBlock::ExFullBlock(_) => {
                        //        info!("ExFull");
                        //    }
                        //    &VersaBlock::InFullBlock(_) => {
                        //        info!("InFull");
                        //    }
                        //}
                        info!("block insertion failure: the verification fails");
                        if let FraudProof::UnsolvedFault = proof {
                            continue;
                        }
                        match &block {
                            &VersaBlock::ExBlock(_) => {
                                continue;
                            }
                            &VersaBlock::InBlock(_) => {
                                continue;
                            }
                            _ => {}
                        }
                        fraud_proofs.insert(proof.clone(), shard_id);
                        self.fp_map.insert(block.hash(), proof);
                        continue;
                    }
                }

                let mut inserted_blk = block.clone();
                let mut removed_buff: Vec<H256> = vec![];
                loop {
                    match self.multichain.insert_block_with_parent(
                        inserted_blk.clone(),
                        &parent_hash,
                        inserted_shard_id
                    ) {
                        Ok(confirmed_info) => {
                            let new_hash = inserted_blk.hash();
                            //After successful insertion, check whether there is an associated fp,
                            //If there is, check whether it is valid, if it does, prune it
                            //immediately, skipping the following operation and continue next
                            //iteration
                            match self.blk2fp.get(&new_hash) {
                                Some(fp) => {
                                    if self.validator.verify_fraud_proof(fp) {
                                        info!("skip block {:?}", new_hash);
                                        let shard_id = fp.get_shard_id();
                                        let block_hash = fp.get_invalid_block();
                                        //self.multichain.prune_fork_with_shard(&block_hash, shard_id);
                                        break;
                                    }
                                }
                                None => {}
                            }
                            info!("successfully inserting block: {:?}", new_hash);
                            match inserted_blk.clone() {
                                VersaBlock::ExBlock(_) 
                                    => new_hashs.insert(
                                            new_hash,
                                            shard_id
                                        ),
                                VersaBlock::InBlock(_)
                                    => new_hashs.insert(
                                            new_hash,
                                            shard_id,
                                        ),
                                VersaBlock::ExFullBlock(_)
                                    => new_hashs.insert(
                                            new_hash,
                                            shard_id,
                                        ),
                                VersaBlock::InFullBlock(_)
                                    => new_hashs.insert(
                                            new_hash,
                                            shard_id,
                                        ),
                            };
                            //handle the confirmation issue
                            let sub_txs_tmys = self.confirmation
                                    .lock()
                                    .unwrap()
                                    .update(
                                        Some(inserted_blk.clone()),
                                        confirmed_info,
                                        inserted_shard_id,
                                    );
                            return_txs_tmys.extend(sub_txs_tmys);

                            //if there are some blocks in the buff whose parent is the new block,
                            //continue to insert it
                            match self.blk_buff.get(&new_hash) {
                                Some(child_blk) => {
                                    inserted_blk = child_blk.clone();
                                    removed_buff.push(new_hash);
                                }
                                None => {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            info!("Reject block {:?} in shard {}: insertion fails: {}", inserted_blk.hash(), self.config.shard_id, e);
                            break;
                        }
                    }
                }
                for item2 in removed_buff {
                    self.blk_buff.remove(&item2);
                }
            }
        }
        let res_hash: Vec<H256> = new_hashs
            .into_iter()
            .map(|(key, _)| key)
            .collect();

        let res_fp: Vec<FraudProof> = fraud_proofs
            .into_iter()
            .map(|(key, val)| key)
            .collect();

        let res_blk_hash = match res_hash.is_empty() {
            false => {
                 match first_block {
                    VersaBlock::ExBlock(_) => Some(Message::NewExBlockHash((res_hash, shard_id as u32))),
                    VersaBlock::InBlock(_) => Some(Message::NewInBlockHash((res_hash, shard_id as u32))),
                    VersaBlock::ExFullBlock(_) => Some(Message::NewExFullBlockHash((res_hash, shard_id as u32))),
                    VersaBlock::InFullBlock(_) => Some(Message::NewInFullBlockHash((res_hash, shard_id as u32))),
                } 
            }
            true => None,
        };
        
        let res_fp_hash = match res_fp.is_empty() {
            false => Some(Message::FraudProofs(res_fp)),
            true => None,
        };

        let missing_blks: Vec<Message> = missing_parents
            .into_iter()
            .map(|(key, value)| Message::NewMissBlockHash((value, key as u32)))
            .collect();

        let res_missing_blks = match missing_blks.is_empty() {
            true => None,
            false => Some(missing_blks),
        };


        if return_txs_tmys.is_empty() {
            return (res_blk_hash, res_fp_hash, None, None, res_missing_blks);
        }

        let mut res_return_txs: HashMap<usize, Vec<Transaction>> = HashMap::new();
        let mut res_return_tmys: HashMap<usize, Vec<Testimony>> = HashMap::new();
        for (return_tx, return_tmy, shards) in return_txs_tmys {
            for shard in shards {
                match res_return_txs.get(&shard) {
                    Some(old_elements) => {
                        let mut new_elements = old_elements.clone();
                        new_elements.push(return_tx.clone());
                        res_return_txs.insert(shard, new_elements);
                    }
                    None => {
                        res_return_txs.insert(shard, vec![return_tx.clone()]);
                    }
                }
                match res_return_tmys.get(&shard) {
                    Some(old_elements) => {
                        let mut new_elements = old_elements.clone();
                        new_elements.push(return_tmy.clone());
                        res_return_tmys.insert(shard, new_elements);
                    }
                    None => {
                        res_return_tmys.insert(shard, vec![return_tmy.clone()]);
                    }
                }
            }
        }
        
        let res_return_txs: Vec<Message> = res_return_txs
            .into_iter()
            .map(|(key, value)| Message::Transactions((value, key as u32)))
            .collect();
        let res_return_tmys: Vec<Message> = res_return_tmys
            .into_iter()
            .map(|(key, value)| Message::Testimonies((value, key as u32)))
            .collect();

        (res_blk_hash, res_fp_hash, Some(res_return_txs), Some(res_return_tmys), res_missing_blks)
    }



    fn handle_new_testimony_hash(&self, tmy_hash_vec: Vec<H256>, shard_id: usize) 
        -> Option<Message> 
    {
        if shard_id != self.config.shard_id {
            return None;
        }
        let mut unreceived_tmys: Vec<H256> = Vec::new();
        for tmy in tmy_hash_vec.iter() {
            if let Some(_) = self.mempool
                .lock()
                .unwrap()
                .get_testimony(tmy) {
                continue;
            }
            unreceived_tmys.push(tmy.clone());
        }
        if !unreceived_tmys.is_empty() {
            Some(Message::GetTestimonies((unreceived_tmys, shard_id as u32)))
        } else {
            None
        }
    }
    fn handle_get_testimonies(&self, tmy_hash_vec: Vec<H256>, shard_id: usize)
        -> Option<Message>
    {
        if shard_id != self.config.shard_id {
            return None;
        }
        let mut res_tmys: Vec<Testimony> = Vec::new();
        for tmy_hash in tmy_hash_vec.iter() {
            if let Some(tmy) = self.mempool
                .lock()
                .unwrap()
                .get_testimony(tmy_hash) {
                res_tmys.push(tmy);
            }
        }
        if !res_tmys.is_empty() {
            Some(Message::Testimonies((res_tmys, shard_id as u32)))
        } else {
            None
        }
    }
    fn handle_testimonies(&mut self, tmys: Vec<Testimony>, shard_id: usize)
        -> Option<Message>
    {
        if shard_id != self.config.shard_id {
            return None;
        }
        let mut new_tmy_hash: Vec<H256> = Vec::new();
        for tmy in tmys {
            let tmy_hash = tmy.hash();
            self.mempool
                .lock()
                .unwrap()
                .add_testimony(tmy);
            new_tmy_hash.push(tmy_hash);
        }
        if !new_tmy_hash.is_empty() {
            Some(Message::NewTestimonyHash((new_tmy_hash, shard_id as u32)))
        } else {
            None
        }
    }

    fn handle_new_fraud_proof_hash(&self, fp_hash_vec: Vec<H256>) 
        -> Option<Message> 
    {
        let mut unreceived_fps: Vec<H256> = vec![];
        for fp_hash in fp_hash_vec {
            match self.fp_map.get(&fp_hash) {
                Some(_) => {}
                None => unreceived_fps.push(fp_hash),
            }
        }
        if !unreceived_fps.is_empty() {
            Some(Message::GetFraudProofs(unreceived_fps))
        } else {
            None
        }
    }

    fn handle_get_fraud_proofs(&self, fp_hash_vec: Vec<H256>)
        -> Option<Message>
    {
        let mut res_fps: Vec<FraudProof> = vec![];
        for fp_hash in fp_hash_vec {
            match self.fp_map.get(&fp_hash) {
                Some(fp) => res_fps.push(fp.clone()),
                None => {}
            }
        }
        if !res_fps.is_empty() {
            Some(Message::FraudProofs(res_fps))
        } else {
            None
        }
    }
    fn handle_fraud_proofs(&mut self, fps: Vec<FraudProof>)
        -> Option<Message>
    {
        let mut new_fp_hash: Vec<H256> = vec![];
        for fp in fps {
            let fp_hash = fp.hash();
            match self.fp_map.get(&fp_hash) {
                Some(_) => continue,
                None => {
                    self.fp_map.insert(fp_hash.clone(), fp.clone());
                    new_fp_hash.push(fp_hash);
                    //connect this fraudproof to its corresponding block
                    let block_hash = fp.get_invalid_block();
                    self.blk2fp.insert(block_hash, fp.clone());
                }
            }
            if self.validator.verify_fraud_proof(&fp) {
                let shard_id = fp.get_shard_id();
                let block_hash = fp.get_invalid_block();
                if shard_id == self.config.shard_id {
                    continue;
                }
                //self.multichain.prune_fork_with_shard(&block_hash, shard_id);
            }
        } 

        if !new_fp_hash.is_empty() {
            Some(Message::NewFraudProofHash(new_fp_hash))
        } else {
            None
        }
    }

    fn handle_new_samples(&self, sample_index_vec: Vec<SampleIndex>) -> Option<Message> {
        let mut unreceived_samples: Vec<SampleIndex> = vec![];

        for sample in sample_index_vec {
            let block_hash = sample.0.clone();
            let tx_index = sample.1 as usize;
            let shard_id = sample.2 as usize;

            if shard_id == self.config.shard_id {
                continue;
            }
            //match self.sample_map.get(&sample) {
            //    Some(_) => {}
            //    None => unreceived_samples.push(sample),
            //}
            unreceived_samples.push(sample);
        }
        if !unreceived_samples.is_empty() {
            Some(Message::GetSamples(unreceived_samples))
        } else {
            None
        }
    }

    fn handle_get_samples(&self, sample_index_vec: Vec<SampleIndex>) -> Option<Message> {
        let mut res_samples: Vec<(SampleIndex, Vec<Sample>)> = vec![];
        for sample_eq in sample_index_vec {
            let block_hash = sample_eq.0.clone();
            let tx_index = sample_eq.1 as usize;
            let shard_id = sample_eq.2 as usize;
            match self.sample_map.get(&sample_eq) {
                Some(samples) => {
                    res_samples.push((sample_eq.clone(), samples.clone()));
                    continue;
                }
                None => {}
            }
            
            //need more consideration here
            if shard_id != self.config.shard_id {
                continue;
            }

            match self.multichain.get_block(&block_hash) {
                Some(versa_block) => {
                    match versa_block {
                        VersaBlock::ExFullBlock(ex_full_block) => {
                            let samples = ex_full_block.into_samples(tx_index);
                            res_samples.push((sample_eq.clone(), samples));
                        }
                        VersaBlock::InFullBlock(in_full_block) => {
                            let samples = in_full_block.into_samples(tx_index);
                            res_samples.push((sample_eq.clone(), samples));
                        }
                        _ => {}
                    }
                }
                None => {}
            }
        }

        if !res_samples.is_empty() {
            Some(Message::Samples(res_samples))
        } else {
            None
        }
    }

    fn handle_samples(&mut self, samples: Vec<(SampleIndex, Vec<Sample>)>) 
        -> (Option<Message>, Option<Vec<Message>>, Option<Vec<Message>>) //new_sample_hash, return_txs,
    //return_tmys
    {
        let mut new_samples: Vec<SampleIndex> = vec![];
        let mut return_txs_tmys: Vec<(Transaction, Testimony, Vec<usize>)> = vec![];
        for sample in samples {
            let sample_key = sample.0;
            let sample_value = sample.1;
            let block_hash = sample_key.0.clone();
            let tx_index = sample_key.1 as usize;
            let shard_id = sample_key.2 as usize;

            //consider more here
            //if shard_id == self.config.shard_id {
            //    continue;
            //}
            
            

            //match self.multichain.get_block(&block_hash) {
            //    Some(versa_block) => {
            //        match versa_block {
            //            VersaBlock::ExFullBlock(ex_full_block) => continue,
            //            VersaBlock::InFullBlock(in_full_block) => continue,
            //            _ => {}
            //        }
            //    } 
            //    None => {}
            //}

            match self.sample_map.get(&sample_key) {
                Some(old_sample) => {
                    let mut new_sample = old_sample.clone();
                    let mut is_updated = false;
                    for sample_unit in sample_value.iter() {
                        if !new_sample.contains(sample_unit) {
                            new_sample.push(sample_unit.clone());
                            is_updated = true;
                        }
                    }
                    if is_updated {
                        self.sample_map.insert(sample_key.clone(), new_sample);
                        new_samples.push(sample_key.clone());
                    }
                }
                None => {
                    self.sample_map.insert(sample_key.clone(), sample_value.clone());
                    new_samples.push(sample_key.clone());
                }
            }
            
            match self.blk2sample.get(&block_hash) {
                Some(old_sample_keys) => {
                    if !old_sample_keys.contains(&sample_key) {
                        let mut new_sample_keys = old_sample_keys.clone();
                        new_sample_keys.push(sample_key.clone());
                        self.blk2sample.insert(block_hash.clone(), new_sample_keys);
                    }
                }
                None => {
                    self.blk2sample.insert(block_hash.clone(), vec![sample_key.clone()]);
                }
            }
            //It should verify blocks across all shards 
            if self.validator.verify_samples(
                &sample_key,
                self.sample_map.get(&sample_key).unwrap().clone()
            ) {
                for shard_id in 0..self.config.shard_num {
                    match self.multichain.verify_block_with_shard(
                        &sample_key.0, 
                        shard_id
                    ) {
                        Ok(confirmed_info) => { 
                            let sub_txs_tmys = self.confirmation
                                    .lock()
                                    .unwrap()
                                    .update(
                                        None,
                                        confirmed_info,
                                        shard_id,
                                    );
                            return_txs_tmys.extend(sub_txs_tmys);
                        }
                        Err(_) => {
                            //info!("{}", e);
                        }
                    }
                }
            }
            
        }
        
        let res_samples = match new_samples.is_empty() {
            false => Some(Message::NewSamples(new_samples)),
            true => None,
        };
        if return_txs_tmys.is_empty() {
            return (res_samples, None, None);
        }

        let mut res_return_txs: HashMap<usize, Vec<Transaction>> = HashMap::new();
        let mut res_return_tmys: HashMap<usize, Vec<Testimony>> = HashMap::new();
        for (return_tx, return_tmy, shards) in return_txs_tmys {
            for shard in shards {
                match res_return_txs.get(&shard) {
                    Some(old_elements) => {
                        let mut new_elements = old_elements.clone();
                        new_elements.push(return_tx.clone());
                        res_return_txs.insert(shard, new_elements);
                    }
                    None => {
                        res_return_txs.insert(shard, vec![return_tx.clone()]);
                    }
                }
                match res_return_tmys.get(&shard) {
                    Some(old_elements) => {
                        let mut new_elements = old_elements.clone();
                        new_elements.push(return_tmy.clone());
                        res_return_tmys.insert(shard, new_elements);
                    }
                    None => {
                        res_return_tmys.insert(shard, vec![return_tmy.clone()]);
                    }
                }
            }
        }
        
        let res_return_txs: Vec<Message> = res_return_txs
            .into_iter()
            .map(|(key, value)| Message::Transactions((value, key as u32)))
            .collect();
        let res_return_tmys: Vec<Message> = res_return_tmys
            .into_iter()
            .map(|(key, value)| Message::Testimonies((value, key as u32)))
            .collect();
        (res_samples, Some(res_return_txs), Some(res_return_tmys))
    }
}

//#[cfg(any(test,test_utilities))]
//struct TestMsgSender {
//    s: smol::channel::Sender<(Vec<u8>, peer::Handle)>
//}
//#[cfg(any(test,test_utilities))]
//impl TestMsgSender {
//    fn new() -> (TestMsgSender, smol::channel::Receiver<(Vec<u8>, peer::Handle)>) {
//        let (s,r) = smol::channel::unbounded();
//        (TestMsgSender {s}, r)
//    }
//
//    fn send(&self, msg: Message) -> PeerTestReceiver {
//        let bytes = bincode::serialize(&msg).unwrap();
//        let (handle, r) = peer::Handle::test_handle();
//        smol::block_on(self.s.send((bytes, handle))).unwrap();
//        r
//    }
//}
//#[cfg(any(test,test_utilities))]
///// returns two structs used by tests, and an ordered vector of hashes of all blocks in the blockchain
//fn generate_test_worker_and_start() -> (TestMsgSender, ServerTestReceiver, Vec<H256>) {
//    let (server, server_receiver) = ServerHandle::new_for_test();
//    let (test_msg_sender, msg_chan) = TestMsgSender::new();
//    let difficulty: H256 = (&[255u8; 32]).into();
//    let blockchain: Arc<Mutex<Blockchain>> = Arc::new(Mutex::new(Blockchain::new(&difficulty)));
//    let worker = Worker::new(1, msg_chan, &server, &blockchain);
//    worker.start(); 
//    let res = (test_msg_sender, server_receiver, blockchain.lock().unwrap().all_blocks_in_longest_chain());
//    res
//}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

//#[cfg(test)]
//mod test {
//    use ntest::timeout;
//    use crate::types::block::generate_random_block;
//    use crate::types::hash::Hashable;
//
//    use super::super::message::Message;
//    use super::generate_test_worker_and_start;
//
//    #[test]
//    #[timeout(60000)]
//    fn reply_new_block_hashes() {
//        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
//        let random_block = generate_random_block(v.last().unwrap());
//        let mut peer_receiver = test_msg_sender.send(Message::NewBlockHashes(vec![random_block.hash()]));
//        let reply = peer_receiver.recv();
//        if let Message::GetBlocks(v) = reply {
//            assert_eq!(v, vec![random_block.hash()]);
//        } else {
//            panic!();
//        }
//    }
//    #[test]
//    #[timeout(60000)]
//    fn reply_get_blocks() {
//        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
//        let h = v.last().unwrap().clone();
//        let mut peer_receiver = test_msg_sender.send(Message::GetBlocks(vec![h.clone()]));
//        let reply = peer_receiver.recv();
//        if let Message::Blocks(v) = reply {
//            assert_eq!(1, v.len());
//            assert_eq!(h, v[0].hash())
//        } else {
//            panic!();
//        }
//    }
//    #[test]
//    #[timeout(60000)]
//    fn reply_blocks() {
//        let (test_msg_sender, server_receiver, v) = generate_test_worker_and_start();
//        let random_block = generate_random_block(v.last().unwrap());
//        let mut _peer_receiver = test_msg_sender.send(Message::Blocks(vec![random_block.clone()]));
//        let reply = server_receiver.recv().unwrap();
//        if let Message::NewBlockHashes(v) = reply {
//            assert_eq!(v, vec![random_block.hash()]);
//        } else {
//            panic!();
//        }
//    }
//}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
