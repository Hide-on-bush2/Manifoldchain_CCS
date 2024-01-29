use crate::{
    types::{
        hash::{H256, Hashable},
    },
    manifoldchain::{
        block::{
            Info,
            consensus_block::ConsensusBlock,
            transaction_block::TransactionBlock,
        },
        transaction::Transaction,
        testimony::Testimony,
    },
};
use serde::{Serialize, Deserialize};
use std::{
    time::SystemTime,
    collections::HashMap,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InclusiveBlock {
    cons_block: ConsensusBlock,
    hash_val: H256,
    inter_parents: Vec<H256>,
    global_parents: Vec<(Vec<H256>, u32)>, //(block, shard_id)
}

impl Hashable for InclusiveBlock {
    fn hash(&self) -> H256 {
        self.hash_val.clone()
    }
}

impl Info for InclusiveBlock {
    fn get_parent(&self) -> H256 {
        self.cons_block.get_parent()
    }
    fn get_difficulty(&self) -> H256 {
        self.cons_block.get_difficulty()
    }
    fn set_difficulty(&mut self, difficulty: &H256) {
        self.cons_block.set_difficulty(&difficulty);
    }
    fn get_nonce(&self) -> usize {
        self.cons_block.get_nonce()
    }
    fn get_timestamp(&self) -> SystemTime {
        self.cons_block.get_timestamp()
    }
    fn get_shard_id(&self) -> usize {
        self.cons_block.get_shard_id()
    }
    fn get_tx_merkle_root(&self) -> H256 {
        self.cons_block.get_tx_merkle_root()
    }
    fn get_info_hash(&self) -> Vec<H256> {
        self.cons_block.get_info_hash()
    }
}

impl Default for InclusiveBlock {
    fn default() -> Self {
        let cons_block = ConsensusBlock::default();
        InclusiveBlock {
            hash_val:cons_block.hash(),
            cons_block,
            inter_parents: vec![],
            global_parents: vec![],
        }
    }
}

impl InclusiveBlock {
    pub fn create(
        cons_block: ConsensusBlock,
        hash_val: H256,
        inter_parents: Vec<H256>,
        global_parents: Vec<(Vec<H256>, usize)>,
    ) -> Self {
        let global_parents: Vec<(Vec<H256>, u32)> = global_parents
            .into_iter()
            .map(|x| (x.0, x.1 as u32))
            .collect();
        InclusiveBlock {
            cons_block,
            hash_val,
            inter_parents,
            global_parents,
        }
    }

    pub fn get_mem_size(&self) -> usize {
        let mut global_parent_mem_size = 0;
        for item in self.global_parents.iter() {
            let item_mem_size = H256::get_mem_size() * item.0.len() 
                + std::mem::size_of::<u32>();
            global_parent_mem_size += item_mem_size;
        }
        global_parent_mem_size
            + H256::get_mem_size() * (1 + self.inter_parents.len())
            + ConsensusBlock::get_mem_size()
    }

    pub fn generate(
        verified_parent: H256,
        shard_id: usize,
        nonce: usize,
        difficulty: H256,
        txs: Vec<Transaction>,
        tmys: Vec<Testimony>,
        inter_parents: Vec<H256>,
        global_parents: Vec<(Vec<H256>, usize)>
    ) -> (InclusiveBlock, TransactionBlock) {
        let (cons_block, tx_block) = ConsensusBlock::generate(
            verified_parent,
            shard_id,
            nonce,
            difficulty,
            txs,
            tmys,
            inter_parents.clone(),
            global_parents.clone()
        );
        let cons_hash = cons_block.hash();
        let in_block = Self::create(
            cons_block,
            cons_hash,
            inter_parents,
            global_parents,
        );
        (in_block, tx_block)
    }

    pub fn get_testimony_merkle_root(&self) -> H256 {
        self.cons_block.get_testimony_merkle_root()
    }


    pub fn get_inter_parent_merkle_root(&self) -> H256 {
        self.cons_block.get_inter_parent_merkle_root()
    }

    pub fn get_global_parent_merkle_root(&self) -> H256 {
        self.cons_block.get_global_parent_merkle_root()
    }

    pub fn verify_format(&self) -> bool {
        if self.cons_block.hash() != self.hash_val {
            return false;
        }

        if self.cons_block.get_inter_parent_merkle_root() != 
            H256::multi_hash(&self.inter_parents) {
            return false;
        }

        let chains: Vec<H256> = self.global_parents
            .clone()
            .into_iter()
            .map(|x| H256::multi_hash(&x.0))
            .collect();
        if self.cons_block.get_global_parent_merkle_root() != 
            H256::multi_hash(&chains) {
            return false;
        }
        
        true
    }
    pub fn verify_hash(&self) -> bool {
        self.cons_block.hash() == self.hash_val
    }    

    pub fn get_cons_block(&self) -> ConsensusBlock {
        self.cons_block.clone()
    }
    
    pub fn get_inter_parents(&self) -> Vec<H256> {
        self.inter_parents.clone()
    }

    pub fn get_global_parents(&self) -> Vec<(Vec<H256>, usize)> {
        self.global_parents
            .iter()
            .map(|x| (x.0.clone(), x.1 as usize))
            .collect()
    }

    pub fn get_global_parents_map(&self) -> HashMap<usize, Vec<H256>> {
        let mut chains: HashMap<usize, Vec<H256>> = HashMap::new();
        for item in self.global_parents.iter() {
            chains.insert(item.1 as usize, item.0.clone());
        }

        chains
    }

    pub fn get_verified_parent(&self) -> H256 {
        self.get_parent()
    }
}

