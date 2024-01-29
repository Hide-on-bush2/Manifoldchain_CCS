use crate::{
    types::{
        hash::{H256, Hashable},
        merkle::MerkleTree,
    },
    manifoldchain::{
        block::{BlockContent, Content},
        testimony::Testimony,
        transaction::Transaction,
    }
};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionBlock {
    shard_id: u32,
    txs: BlockContent,
    testimonys: MerkleTree<Testimony>,
}

impl Default for TransactionBlock {
    fn default() -> Self {
        let tmys: Vec<Testimony> = vec![];
        TransactionBlock {
            shard_id: 0 as u32,
            txs: BlockContent::default(),
            testimonys: MerkleTree::<Testimony>::new(&tmys),
        }
    }
}

impl TransactionBlock {
    pub fn new(shard_id: usize, txs: Vec<Transaction>, tmys: Vec<Testimony>) -> Self {
        let tx_merkle_tree: MerkleTree<Transaction> = MerkleTree::new(txs.as_slice());
        let tmy_merkle_tree: MerkleTree<Testimony> = MerkleTree::new(tmys.as_slice());
        let block_content = BlockContent {
            txs: tx_merkle_tree,
        };
        TransactionBlock {
            shard_id: shard_id as u32,
            txs: block_content,
            testimonys: tmy_merkle_tree,
        }
    }
    pub fn get_mem_size(&self) -> usize {
        let txs = self.get_txs();
        let tmys = self.get_tmys();
        let mut txs_mem_size = 0;
        let mut tmys_mem_size = 0;
        for tx in txs {
            txs_mem_size += tx.get_mem_size();
        }
        for (_, val) in tmys {
            tmys_mem_size += val.get_mem_size();
        }
        std::mem::size_of::<u32>() + txs_mem_size + tmys_mem_size
    }
    pub fn get_tx_merkle_root(&self) -> H256 {
        self.txs.get_tx_merkle_root()
    }
    pub fn get_testimony_merkle_root(&self) -> H256 {
        self.testimonys.root()
    } 
    pub fn get_shard_id(&self) -> usize {
        self.shard_id as usize
    }
    pub fn get_tmys(&self) -> HashMap<H256, Testimony> {
        let mut res: HashMap<H256, Testimony> = HashMap::new();
        for tmy in self.testimonys.data.iter() {
            res.insert(tmy.get_tx_hash(), tmy.clone());
        }
        res
    }
    pub fn get_tx_merkle_proof(&self, tx_index: usize) -> Vec<H256> {
        self.txs.get_tx_merkle_proof(tx_index)
    }
    pub fn get_tx_merkle_proof2(&self, tx_hash: &H256) -> Option<(Vec<H256>, usize)> {
        let txs = self.get_txs_ref();
        for i in 0..txs.len() {
            let tx_ref = &txs[i];
            if tx_ref.hash() == *tx_hash {
                return Some((self.get_tx_merkle_proof(i), i));
            }
        } 
        None
    } 
    pub fn get_tmy_merkle_proof(&self, tmy_hash: &H256) -> Option<(Vec<H256>, usize)> {
        for i in 0..self.testimonys.data.len() {
            let tmy = &self.testimonys.data[i];
            if tmy.hash() == *tmy_hash {
                return Some((self.testimonys.proof(i), i));
            }
        }
        None
    }
}


impl Content for TransactionBlock {
    fn get_txs(&self) -> Vec<Transaction> {
        self.txs.get_txs()
    }
    fn get_txs_ref(&self) -> &Vec<Transaction> {
        self.txs.get_txs_ref()
    }
}

