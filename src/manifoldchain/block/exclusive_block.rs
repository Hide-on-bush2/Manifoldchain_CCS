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
    }
};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExclusiveBlock {
    cons_block: ConsensusBlock,
    hash_val: H256,
    inter_parents: Vec<H256>,
}

impl Hashable for ExclusiveBlock {
    fn hash(&self) -> H256 {
        self.hash_val.clone()
    }
}

impl Info for ExclusiveBlock {
    fn get_parent(&self) -> H256 {
        self.cons_block.get_parent()
    }
    fn get_difficulty(&self) -> H256 {
        self.cons_block.get_difficulty()
    }
    fn set_difficulty(&mut self, difficulty: &H256) {
        self.cons_block.set_difficulty(difficulty);
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

impl Default for ExclusiveBlock {
    fn default() -> Self {
        let cons_block = ConsensusBlock::default();
        ExclusiveBlock {
            hash_val: cons_block.hash(),
            cons_block,
            inter_parents: vec![],
        }
    }
}

impl ExclusiveBlock {
    pub fn create(
        cons_block: ConsensusBlock,
        hash_val: H256,
        inter_parents: Vec<H256>,
    ) -> Self {
        ExclusiveBlock {
            cons_block,
            hash_val,
            inter_parents,
        }
    }

    pub fn get_mem_size(&self) -> usize {
        ConsensusBlock::get_mem_size()
            + H256::get_mem_size() * (self.inter_parents.len() + 1)
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
    ) -> (ExclusiveBlock, TransactionBlock) {
        let (consensus_block, tx_block) = ConsensusBlock::generate(
            verified_parent,
            shard_id,
            nonce,
            difficulty,
            txs,
            tmys,
            inter_parents.clone(),
            global_parents
        );
        let cons_hash = consensus_block.hash();
        let ex_block = Self::create(
            consensus_block,
            cons_hash,
            inter_parents
        ); 
        (ex_block, tx_block)
    }

    pub fn get_testimony_merkle_root(&self) -> H256 {
        self.cons_block.get_testimony_merkle_root()
    }

    pub fn get_inter_parent_merkle_root(&self) -> H256 {
        self.cons_block.get_inter_parent_merkle_root()
    }
        

    pub fn verify_format(&self) -> bool {
        if self.cons_block.hash() != self.hash_val {
            return false;
        }

        if self.cons_block.get_inter_parent_merkle_root() !=
            H256::multi_hash(&self.inter_parents) {
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
   
    pub fn get_verified_parent(&self) -> H256 {
        self.get_parent()
    }
}



