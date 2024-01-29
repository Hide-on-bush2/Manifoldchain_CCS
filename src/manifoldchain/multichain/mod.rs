use crate::{
    manifoldchain::{
        blockchain::{
            Blockchain,
            State,
            VerStatus,
        },
        configuration::Configuration,
        block::{
            Info,
            versa_block::VersaBlock,
            consensus_block::ConsensusBlock,
        },
        transaction::Transaction,
    },
    types::{
        hash::H256,
    }
};
use std::{
    sync::{Arc, Mutex},
    collections::HashMap,
};


pub struct Multichain {
    pub config: Configuration,
    chains: Vec<Arc<Mutex<Blockchain>>>,
}

impl Clone for Multichain {
    fn clone(&self) -> Self {
        let new_chains: Vec<Arc<Mutex<Blockchain>>> = self.chains
            .clone()
            .into_iter()
            .map(|x| Arc::clone(&x))
            .collect();
        Multichain {
            config: self.config.clone(),
            chains: new_chains,
        }
    }
}

impl Multichain {
    pub fn create(
        chain_refs: Vec<&Arc<Mutex<Blockchain>>>, 
        config: &Configuration) -> Self 
    {
        let chains: Vec<Arc<Mutex<Blockchain>>> = chain_refs
            .into_iter()
            .map(|x| Arc::clone(x))
            .collect();
        Multichain {
            chains,
            config: config.clone()
        }
    }

    //pub fn insert_block(&mut self, block: VersaBlock) 
    //    -> Result<Option<(VersaBlock, usize)>, String> 
    //{
    //    self.chains
    //        .get(self.config.shard_id)
    //        .unwrap()
    //        .lock()
    //        .unwrap()
    //        .insert_block(block)
    //}

    //pub fn insert_block_with_shard(
    //    &mut self, 
    //    block: VersaBlock, 
    //    shard_id: usize
    //) -> Result<Option<(VersaBlock, usize)>, String> {
    //    self.chains
    //        .get(shard_id)
    //        .unwrap()
    //        .lock()
    //        .unwrap()
    //        .insert_block(block)
    //}
    pub fn insert_block_with_parent(
        &mut self,
        block: VersaBlock,
        parent: &H256,
        shard_id: usize
    ) -> Result<Option<(VersaBlock, usize)>, String> {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .insert_block_with_parent(block, parent)
    }
    pub fn get_all_leaves(&self) -> Vec<H256> {
        let mut all_leaves: Vec<H256>  = Vec::new();
        for chain in self.chains.iter() {
            let leaves = chain.lock().unwrap().get_leaves();
            all_leaves.extend(leaves);
        }
        all_leaves
    }

    pub fn get_longest_chain_hash(&self) -> H256 {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .tip()
    }
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .all_blocks_in_longest_chain()

    }
    pub fn all_blocks_in_longest_chain_with_shard(&self, shard_id: usize) -> Vec<H256> {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .all_blocks_in_longest_chain()
    }
    pub fn all_blocks_end_with_block(&self, hash: &H256) -> Option<Vec<H256>> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .all_blocks_end_with_block(hash)
    }
    pub fn all_blocks_end_with_block_with_shard(&self, hash: &H256, shard_id: usize) 
        -> Option<Vec<H256>>
    {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .all_blocks_end_with_block(hash)
    }
    pub fn get_block(&self, hash: &H256) -> Option<VersaBlock> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_block(hash)
    }
    pub fn get_verify_status_with_shard(&self, hash: &H256, shard_id: usize) 
        -> Option<VerStatus> 
    {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_verify_status(hash)
    }
    pub fn get_states(&self) -> HashMap<H256, State> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_states()
    }
    
    pub fn get_leaves(&self) -> Vec<H256> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_leaves()
    }
    pub fn get_tx_in_longest_chain(
        &self, 
        tx_hash: &H256) -> Option<Transaction> 
    {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_tx_in_longest_chain(tx_hash)
    }
    
    pub fn get_tx_in_longest_chain_by_shard(
        &self,
        tx_hash: &H256,
        shard_id: usize
    ) -> Option<Transaction> {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_tx_in_longest_chain(tx_hash)
    }

    pub fn get_consensus_block_by_shard(
        &self, 
        shard_id: usize, 
        hash: &H256) -> Option<ConsensusBlock> 
    {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_consensus_block(hash)
    }
    pub fn is_block_confirmed(
        &self, 
        shard_id: usize, 
        hash: &H256) -> bool 
    {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .is_block_confirmed(hash, self.config.k)
    }

    pub fn is_block_in_longest_chain(
        &self,
        shard_id: usize,
        hash: &H256
    ) -> bool {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .is_block_in_longest_chain(hash)
    }

    pub fn get_all_available_forks(&self) -> Vec<(H256, usize)> {
        let mut res: Vec<(H256, usize)> = Vec::new();
        for shard_id in 0..self.config.shard_num {
            let leaves = self.chains
                .get(shard_id)
                .unwrap()
                .lock()
                .unwrap()
                .get_leaves();
            let leaves_by_shard: Vec<(H256, usize)> = leaves
                .into_iter()
                .map(|x| (x.clone(), shard_id))
                .collect();
            res.extend(leaves_by_shard);
        }
        res
    }
    pub fn get_block_by_shard(&self, hash: &H256, shard_id: usize) -> Option<VersaBlock> {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_block(hash)
    }
    pub fn get_inter_unverified_forks(&self) -> Vec<H256> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_leaves()
    }
    pub fn get_global_unverified_forks(&self) -> Vec<(Vec<H256>, usize)> {
        (0..self.config.shard_num)
            .into_iter()
            .map(|i|{
                (
                    self.chains
                        .get(i)
                        .unwrap()
                        .lock()
                        .unwrap()
                        .get_leaves(),
                    i
                )
            }).collect()
    }
    pub fn get_longest_verified_fork(&self) -> H256 {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_longest_verified_fork()
    }
    pub fn get_longest_verified_fork_with_shard(&self, shard_id: usize) -> H256 {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_longest_verified_fork()
    }

    pub fn prune_fork_with_shard(&self, block: &H256, shard_id: usize) {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .prune_fork(block);
    }

    pub fn get_block_with_tx(&self, tx_hash: &H256) -> Option<(VersaBlock, usize)> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_block_with_tx(tx_hash)
    }

    pub fn verify_block_with_shard(&self, block: &H256, shard_id: usize) 
        -> Result<Option<(VersaBlock, usize)>, String> 
    {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .verify_block(block)
    }

    pub fn get_block_height_with_shard(&self, block_hash: &H256, shard_id: usize) 
        -> Option<usize> 
    {
        self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_block_height(block_hash)
    }

    pub fn get_all_txs_in_longest_chain(&self) -> Vec<Transaction> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_all_txs_in_longest_chain()
    }
    pub fn log_to_file_with_shard(&self, shard_id: usize) {
        let _ = self.chains
            .get(shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .log_to_file();
    }

    pub fn get_unverified_blocks(&self) -> Vec<(H256, usize)> {
        let mut res: Vec<(H256, usize)> = vec![];
        for shard_id in 0..self.config.shard_num {
            let unverified_blocks = self.chains
                .get(shard_id)
                .unwrap()
                .lock()
                .unwrap()
                .get_unverified_blocks();
            res.extend(&unverified_blocks);
        }
        res
    }
    pub fn all_blocks_in_longest_chain_with_time(&self) -> Vec<(H256, String)> {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .all_blocks_in_longest_chain_with_time()
    }

    pub fn get_forking_rate(&self) -> f64 {
        self.chains
            .get(self.config.shard_id)
            .unwrap()
            .lock()
            .unwrap()
            .get_forking_rate()
    }
}
