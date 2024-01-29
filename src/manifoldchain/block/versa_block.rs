use crate::{
    manifoldchain::{
        block::{
            Info,
            Content,
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
            transaction_block::TransactionBlock,
        },
        transaction::Transaction,
        testimony::Testimony,
        network::worker::Sample,
    },
    types::hash::{H256, Hashable},
};
use std::{
    time::SystemTime,
    collections::HashMap,
};
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)] 
pub struct ExclusiveFullBlock {
    pub ex_block: ExclusiveBlock,
    pub tx_block: TransactionBlock
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct InclusiveFullBlock {
    pub in_block: InclusiveBlock,
    pub tx_block: TransactionBlock
}

#[derive(Clone, Serialize, Deserialize)]
pub enum VersaBlock {   
    ExBlock(ExclusiveBlock),
    InBlock(InclusiveBlock),
    ExFullBlock(ExclusiveFullBlock),
    InFullBlock(InclusiveFullBlock),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum VersaHash {
    ExHash(H256),
    InHash(H256),
    ExFullHash(H256),
    InFullHash(H256)
}

impl Hashable for ExclusiveFullBlock {
    fn hash(&self) -> H256 {
        self.ex_block.hash()
    }
}

impl Hashable for InclusiveFullBlock {
    fn hash(&self) -> H256 {
        self.in_block.hash()
    }
}

impl Info for ExclusiveFullBlock {
    fn get_parent(&self) -> H256 {
        self.ex_block.get_parent()
    }
    fn get_difficulty(&self) -> H256 {
        self.ex_block.get_difficulty()
    }
    fn set_difficulty(&mut self, difficulty: &H256) {
        self.ex_block.set_difficulty(difficulty);
    } 
    fn get_nonce(&self) -> usize {
        self.ex_block.get_nonce()
    }
    fn get_timestamp(&self) -> SystemTime {
        self.ex_block.get_timestamp()
    }
    fn get_shard_id(&self) -> usize {
        self.ex_block.get_shard_id()
    }
    fn get_tx_merkle_root(&self) -> H256 {
        self.ex_block.get_tx_merkle_root()
    }
    fn get_info_hash(&self) -> Vec<H256> {
        self.ex_block.get_info_hash()
    }
}

impl Info for InclusiveFullBlock {
    fn get_parent(&self) -> H256 {
        self.in_block.get_parent()
    }
    fn get_difficulty(&self) -> H256 {
        self.in_block.get_difficulty()
    }
    fn set_difficulty(&mut self, difficulty: &H256) {
        self.in_block.set_difficulty(difficulty);
    } 
    fn get_nonce(&self) -> usize {
        self.in_block.get_nonce()
    }
    fn get_timestamp(&self) -> SystemTime {
        self.in_block.get_timestamp()
    }
    fn get_shard_id(&self) -> usize {
        self.in_block.get_shard_id()
    }
    fn get_tx_merkle_root(&self) -> H256 {
        self.in_block.get_tx_merkle_root()
    }
    fn get_info_hash(&self) -> Vec<H256> {
        self.in_block.get_info_hash()
    }
}

impl Content for ExclusiveFullBlock {
    fn get_txs(&self) -> Vec<Transaction> {
        self.tx_block.get_txs()
    }
    fn get_txs_ref(&self) -> &Vec<Transaction> {
        self.tx_block.get_txs_ref()
    }
}

impl Content for InclusiveFullBlock {
    fn get_txs(&self) -> Vec<Transaction> {
        self.tx_block.get_txs()
    }
    fn get_txs_ref(&self) -> &Vec<Transaction> {
        self.tx_block.get_txs_ref()
    }
}

impl Default for VersaBlock {
    fn default() -> Self {
        VersaBlock::ExBlock(ExclusiveBlock::default())
    }
}

impl VersaBlock {
    pub fn get_txs(&self) -> Option<Vec<Transaction>> {
        match self {
            VersaBlock::ExBlock(_) => None,
            VersaBlock::InBlock(_) => None,
            VersaBlock::ExFullBlock(ex_full_block) => Some(ex_full_block.get_txs()),
            VersaBlock::InFullBlock(in_full_block) => Some(in_full_block.get_txs()),
        } 
    }

    pub fn get_txs_ref(&self) -> Option<&Vec<Transaction>> {
        match self {
            VersaBlock::ExBlock(_) => None,
            VersaBlock::InBlock(_) => None,
            VersaBlock::ExFullBlock(ex_full_block) => Some(ex_full_block.get_txs_ref()),
            VersaBlock::InFullBlock(in_full_block) => Some(in_full_block.get_txs_ref()),
        }
    }

    pub fn get_tmys(&self) -> Option<HashMap<H256, Testimony>> {
        match self {
            VersaBlock::ExBlock(_) => None,
            VersaBlock::InBlock(_) => None,
            VersaBlock::ExFullBlock(ex_full_block) => Some(ex_full_block.get_tmys()),
            VersaBlock::InFullBlock(in_full_block) => Some(in_full_block.get_tmys()),
        }
    }

    pub fn get_inter_parents(&self) -> Vec<H256> {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_inter_parents(),
            VersaBlock::InBlock(in_block) => in_block.get_inter_parents(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_inter_parents(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_inter_parents(),
        }
    }

    pub fn get_global_parents(&self) -> Option<Vec<(Vec<H256>, usize)>> {
        match self {
            VersaBlock::ExBlock(ex_block) => None,
            VersaBlock::InBlock(in_block) => Some(in_block.get_global_parents()),
            VersaBlock::ExFullBlock(ex_full_block) => None,
            VersaBlock::InFullBlock(in_full_block) => Some(in_full_block.get_global_parents()),
        }
    }

    pub fn get_verified_parent(&self) -> H256 {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_verified_parent(),
            VersaBlock::InBlock(in_block) => in_block.get_verified_parent(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_verified_parent(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_verified_parent(),
        }
    }

    pub fn verify_hash(&self) -> bool {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.verify_hash(),
            VersaBlock::InBlock(in_block) => in_block.verify_hash(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.verify_hash(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.verify_hash(),
        }
    }

    pub fn get_tx_merkle_proof(&self, index: usize) -> Option<Vec<H256>> {
        match self {
            VersaBlock::ExBlock(_) => None,
            VersaBlock::InBlock(_) => None,
            VersaBlock::ExFullBlock(ex_full_block) => Some(ex_full_block.get_tx_merkle_proof(index)),
            VersaBlock::InFullBlock(in_full_block) => Some(in_full_block.get_tx_merkle_proof(index)),
        }
    }

    pub fn into_samples(&self, index: usize) -> Option<Vec<Sample>> {
        match self {
            VersaBlock::ExBlock(_) => None,
            VersaBlock::InBlock(_) => None,
            VersaBlock::ExFullBlock(ex_full_block) => Some(ex_full_block.into_samples(index)),
            VersaBlock::InFullBlock(in_full_block) => Some(in_full_block.into_samples(index)),
        } 
    }

    pub fn get_tx_merkle_proof2(&self, tx_hash: &H256) -> Option<(Vec<H256>, usize)> {
        match self {
            VersaBlock::ExBlock(_) => None,
            VersaBlock::InBlock(_) => None,
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_tx_merkle_proof2(tx_hash),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_tx_merkle_proof2(tx_hash),
        }
    }

    pub fn get_tmy_merkle_proof(&self, tmy_hash: &H256) -> Option<(Vec<H256>, usize)> {
        match self {
            VersaBlock::ExBlock(_) => None,
            VersaBlock::InBlock(_) => None,
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_tmy_merkle_proof(tmy_hash),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_tmy_merkle_proof(tmy_hash),
        }
    }

    pub fn get_testimony_merkle_root(&self) -> H256 {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_testimony_merkle_root(),
            VersaBlock::InBlock(in_block) => in_block.get_testimony_merkle_root(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_testimony_merkle_root(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_testimony_merkle_root(),
        }
    }

}

impl ExclusiveFullBlock {
    pub fn create(
        ex_block: ExclusiveBlock,
        tx_block: TransactionBlock
    ) -> Self {
        ExclusiveFullBlock {
            ex_block,
            tx_block
        }
    }

    pub fn get_mem_size(&self) -> usize {
        self.ex_block.get_mem_size() + self.tx_block.get_mem_size()
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
    ) -> ExclusiveFullBlock {
        let (ex_block, tx_block) = ExclusiveBlock::generate(
            verified_parent,
            shard_id,
            nonce,
            difficulty,
            txs,
            tmys,
            inter_parents,
            global_parents
        );
        ExclusiveFullBlock {
            ex_block,
            tx_block
        }
    }
    pub fn get_testimony_merkle_root(&self) -> H256 {
        self.ex_block.get_testimony_merkle_root()
    }
    pub fn get_tmys(&self) -> HashMap<H256, Testimony> {
        self.tx_block.get_tmys()
    }
    pub fn get_inter_parent_merkle_root(&self) -> H256 {
        self.ex_block.get_inter_parent_merkle_root()
    }
    pub fn verify_hash(&self) -> bool {
        self.ex_block.verify_hash()
    }
    pub fn get_exclusive_block(&self) -> ExclusiveBlock {
        self.ex_block.clone()
    }
    pub fn get_inter_parents(&self) -> Vec<H256> {
        self.ex_block.get_inter_parents()
    }
    pub fn verify_format(&self) -> bool {
        self.ex_block.verify_hash()
    }
    pub fn get_verified_parent(&self) -> H256 {
        self.ex_block.get_verified_parent()
    }
    pub fn get_tx_merkle_proof(&self, index: usize) -> Vec<H256> {
        self.tx_block.get_tx_merkle_proof(index)
    }
    pub fn get_tx_merkle_proof2(&self, tx_hash: &H256) -> Option<(Vec<H256>, usize)> {
        self.tx_block.get_tx_merkle_proof2(tx_hash)
    }
    pub fn get_tmy_merkle_proof(&self, tmy_hash: &H256) -> Option<(Vec<H256>, usize)> {
        self.tx_block.get_tmy_merkle_proof(tmy_hash)
    }
    pub fn into_samples(&self, index: usize) -> Vec<Sample> {
        let proof = self.tx_block.get_tx_merkle_proof(index);
        let mut res: Vec<Sample> = vec![];
        for i in 0..proof.len() {
            let sample: Sample = (i as u32, proof[i]);
            res.push(sample);
        }
        res
    }
}


impl InclusiveFullBlock {
    pub fn create(
        in_block: InclusiveBlock,
        tx_block: TransactionBlock
    ) -> Self {
        InclusiveFullBlock {
            in_block,
            tx_block
        }
    }
    pub fn get_mem_size(&self) -> usize {
        self.in_block.get_mem_size() + self.tx_block.get_mem_size()
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
    ) -> Self {
        let (in_block, tx_block) = InclusiveBlock::generate(
            verified_parent,
            shard_id,
            nonce,
            difficulty,
            txs,
            tmys,
            inter_parents,
            global_parents
        );
        InclusiveFullBlock {
            in_block,
            tx_block
        }
    }
    pub fn get_testimony_merkle_root(&self) -> H256 {
        self.in_block.get_testimony_merkle_root()
    } 
    pub fn get_inter_parent_merkle_root(&self) -> H256 {
        self.in_block.get_inter_parent_merkle_root()
    }
    pub fn get_global_parent_merkle_root(&self) -> H256 {
        self.in_block.get_global_parent_merkle_root()
    }
    pub fn get_tmys(&self) -> HashMap<H256, Testimony> {
        self.tx_block.get_tmys()
    }
    pub fn verify_hash(&self) -> bool {
        self.in_block.verify_hash()
    }
    pub fn get_inclusive_block(&self) -> InclusiveBlock {
        self.in_block.clone()
    }
    pub fn get_inter_parents(&self) -> Vec<H256> {
        self.in_block.get_inter_parents()
    }
    pub fn get_global_parents(&self) -> Vec<(Vec<H256>, usize)> {
        self.in_block.get_global_parents()
    }
    pub fn get_global_parents_map(&self) -> HashMap<usize, Vec<H256>> {
        self.in_block.get_global_parents_map()
    }
    pub fn get_verified_parent(&self) -> H256 {
        self.in_block.get_verified_parent()
    }
    pub fn get_tx_merkle_proof(&self, index: usize) -> Vec<H256> {
        self.tx_block.get_tx_merkle_proof(index)
    }
    pub fn get_tx_merkle_proof2(&self, tx_hash: &H256) -> Option<(Vec<H256>, usize)> {
        self.tx_block.get_tx_merkle_proof2(tx_hash)
    }
    pub fn get_tmy_merkle_proof(&self, tmy_hash: &H256) -> Option<(Vec<H256>, usize)> {
        self.tx_block.get_tmy_merkle_proof(tmy_hash)
    }
    pub fn into_samples(&self, index: usize) -> Vec<Sample> {
        let proof = self.tx_block.get_tx_merkle_proof(index);
        let mut res: Vec<Sample> = vec![];
        for i in 0..proof.len() {
            let sample: Sample = (i as u32, proof[i]);
            res.push(sample);
        }
        res
    }
}

impl Hashable for VersaBlock {
    fn hash(&self) -> H256 {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.hash(),
            VersaBlock::InBlock(in_block) => in_block.hash(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.hash(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.hash(),
        }
    }
}

impl Info for VersaBlock {
    fn get_parent(&self) -> H256 {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_parent(),
            VersaBlock::InBlock(in_block) => in_block.get_parent(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_parent(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_parent(),
        }
    }

    fn get_difficulty(&self) -> H256 {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_difficulty(),
            VersaBlock::InBlock(in_block) => in_block.get_difficulty(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_difficulty(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_difficulty(),
        }
    }

    fn set_difficulty(&mut self, difficulty: &H256) {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.set_difficulty(difficulty),
            VersaBlock::InBlock(in_block) => in_block.set_difficulty(difficulty),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.set_difficulty(difficulty),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.set_difficulty(difficulty),
        }
    } 

    fn get_nonce(&self) -> usize {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_nonce(),
            VersaBlock::InBlock(in_block) => in_block.get_nonce(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_nonce(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_nonce(),
        }
    }

    fn get_timestamp(&self) -> SystemTime {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_timestamp(),
            VersaBlock::InBlock(in_block) => in_block.get_timestamp(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_timestamp(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_timestamp(),
        }
    }

    fn get_shard_id(&self) -> usize {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_shard_id(),
            VersaBlock::InBlock(in_block) => in_block.get_shard_id(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_shard_id(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_shard_id(),
        }
    }

    fn get_tx_merkle_root(&self) -> H256 {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_tx_merkle_root(),
            VersaBlock::InBlock(in_block) => in_block.get_tx_merkle_root(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_tx_merkle_root(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_tx_merkle_root(),
        }
    }

    fn get_info_hash(&self) -> Vec<H256> {
        match self {
            VersaBlock::ExBlock(ex_block) => ex_block.get_info_hash(),
            VersaBlock::InBlock(in_block) => in_block.get_info_hash(),
            VersaBlock::ExFullBlock(ex_full_block) => ex_full_block.get_info_hash(),
            VersaBlock::InFullBlock(in_full_block) => in_full_block.get_info_hash(),
        }
    }
}
