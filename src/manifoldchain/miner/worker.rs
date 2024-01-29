use crossbeam::channel::Receiver;
use log::{debug, info};
use crate::{
    manifoldchain::{
        block::{
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
            versa_block::{
                VersaBlock,
                ExclusiveFullBlock,
                InclusiveFullBlock,
            }
        },
        network::{
            server::Handle as ServerHandle,
            message::Message,
            worker::{SampleIndex, Sample},
        },
        multichain::Multichain,
        miner::MinerMessage,
        confirmation::Confirmation,
        transaction::Transaction,
        testimony::Testimony,
        configuration::Configuration,
    }
};
use std::{
    thread, 
    sync::{Arc, Mutex},
    collections::HashMap,
};
use rand::Rng;

#[derive(Clone)]
pub struct Worker {
    server: ServerHandle,
    finished_block_chan: Receiver<MinerMessage>,
    multichain: Multichain,
    confirmation: Arc<Mutex<Confirmation>>,
    config: Configuration,
}

impl Worker {
    pub fn new(
        server: &ServerHandle,
        finished_block_chan: Receiver<MinerMessage>,
        multichain: &Multichain,
        confirmation: &Arc<Mutex<Confirmation>>,
        config: &Configuration,
    ) -> Self {
        Self {
            server: server.clone(),
            finished_block_chan,
            multichain: multichain.clone(),
            confirmation: Arc::clone(confirmation),
            config: config.clone(),
        }
    }

    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner-worker".to_string())
            .spawn(move || {
                self.worker_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn handle_confirmation(
        &self, 
        versa_block: VersaBlock, 
        confirmation_info: Option<(VersaBlock, usize)>,
        shard_id: usize,
    ) {
        let return_txs_tmys = self.confirmation
                .lock()
                .unwrap()
                .update(
                    Some(versa_block),
                    confirmation_info,
                    shard_id,
                );
        if !return_txs_tmys.is_empty() {
            let mut return_txs: HashMap<usize, Vec<Transaction>> = HashMap::new();
            let mut return_tmys: HashMap<usize, Vec<Testimony>> = HashMap::new();
            for (return_tx, return_tmy, shards) in return_txs_tmys {
                for shard in shards {
                    match return_txs.get(&shard) {
                        Some(old_elements) => {
                            let mut new_elements = old_elements.clone();
                            new_elements.push(return_tx.clone());
                            return_txs.insert(shard, new_elements);
                        }
                        None => {
                            return_txs.insert(shard, vec![return_tx.clone()]);
                        }
                    }
                    match return_tmys.get(&shard) {
                        Some(old_elements) => {
                            let mut new_elements = old_elements.clone();
                            new_elements.push(return_tmy.clone());
                            return_tmys.insert(shard, new_elements);
                        }
                        None => {
                            return_tmys.insert(shard, vec![return_tmy.clone()]);
                        }
                    }
                }
            }
            for (key, value) in return_txs {
                self.server.broadcast_with_shard(
                    Message::Transactions((value, key as u32)),
                    key
                );
            }
            for (key, value) in return_tmys {
                self.server.broadcast_with_shard(
                    Message::Testimonies((value, key as u32)),
                    key
                );
            }
        }
    }

    fn worker_loop(&mut self) {
        loop {
            let message = self.finished_block_chan
                .recv()
                .expect("Receive finished block error");
             
            match message {
                MinerMessage::ExFullBlock(ex_full_block) => {
                    let ex_block = ex_full_block.get_exclusive_block();
                    let inter_parents = ex_full_block.get_inter_parents();
                    let mut successful_insertion = false;
                    info!("inter_parents size: {}", inter_parents.len());
                    for parent in inter_parents {
                        match self.multichain.insert_block_with_parent(
                            VersaBlock::ExFullBlock(ex_full_block.clone()),
                            &parent,
                            self.config.shard_id,
                        ) {
                            Ok(confirmation_info) => {
                                successful_insertion = true;
                                self.handle_confirmation(
                                    VersaBlock::ExFullBlock(ex_full_block.clone()),
                                    confirmation_info,
                                    self.config.shard_id,
                                );
                            }
                            Err(e) => {
                                info!("inserting myself fail: {}", e);
                            }
                        }
                    }
                    if successful_insertion {
                        let new_ex_blocks: Vec<ExclusiveBlock> = vec![ex_block];
                        self.server.broadcast(
                                Message::ExBlocks((
                                    new_ex_blocks, 
                                    self.config.shard_id as u32
                                ))
                            );
                        let new_blocks: Vec<ExclusiveFullBlock> = vec![ex_full_block];
                        self.server.broadcast_with_shard(
                                Message::ExFullBlocks((
                                    new_blocks, 
                                    self.config.shard_id as u32
                                )), 
                                self.config.shard_id
                            );       
                    }
                }
                MinerMessage::InFullBlock(in_full_block) => {
                    let in_block = in_full_block.get_inclusive_block();
                    let global_parents = in_full_block.get_global_parents();
                    let mut successful_insertion = false;
                    for (inter_parents, shard_id) in global_parents {
                        info!("inter_parents size: {}", inter_parents.len());
                        for parent in inter_parents {
                            let inserted_block = match (shard_id == self.config.shard_id) {
                                true => VersaBlock::InFullBlock(in_full_block.clone()),
                                false => VersaBlock::InBlock(in_block.clone()),
                            };
                            match self.multichain.insert_block_with_parent(
                                inserted_block,
                                &parent,
                                shard_id
                            ) {
                                Ok(confirmation_info) => {
                                    successful_insertion = true;
                                    self.handle_confirmation(
                                        VersaBlock::InFullBlock(in_full_block.clone()),
                                        confirmation_info,
                                        shard_id
                                    );
                                }
                                Err(e) => {
                                    info!("inserting myself fail: {}", e);
                                }
                            }
                        }
                    }
                    if successful_insertion {
                        let new_in_blocks: Vec<InclusiveBlock> = vec![in_block];
                        self.server.broadcast(
                            Message::InBlocks((
                                new_in_blocks, 
                                self.config.shard_id as u32
                            ))
                        );
                        let new_blocks: Vec<InclusiveFullBlock> = vec![in_full_block];
                        self.server.broadcast_with_shard(
                            Message::InFullBlocks((
                                new_blocks, 
                                self.config.shard_id as u32
                            )),
                            self.config.shard_id
                        );
                    }
                }
                MinerMessage::Testimonies(tmys) => {
                    for (key, value) in tmys {
                        self.server.broadcast_with_shard(
                            Message::Testimonies((value, key as u32)),
                            key
                        );
                    }
                }
                MinerMessage::OutputTransactions(output_txs) => {
                    for (key, value) in output_txs {
                        self.server.broadcast_with_shard(
                            Message::Transactions((value, key as u32)),
                            key
                        );
                    }
                }
                MinerMessage::GetSamples(blk_infos) => {
                    let mut rq_samples: Vec<SampleIndex> = vec![];
                    info!("Miner worker get {} samples", blk_infos.len());
                    for (blk_hash, shard_id) in blk_infos {
                        let mut rng = rand::thread_rng();
                        let tx_index: usize = rng.gen_range(0..self.config.block_size);
                        rq_samples.push((blk_hash, tx_index as u32, shard_id as u32)); 
                    }    
                    self.server.broadcast(Message::GetSamples(rq_samples));
                }
            }

        }
    }
}
