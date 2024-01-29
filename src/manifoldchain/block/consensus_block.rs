use crate::{
    types::{
        hash::{H256, Hashable},
    },
    manifoldchain::{
        block::{
            Info,
            BlockHeader,
            transaction_block::TransactionBlock,
        },
        transaction::Transaction,
        testimony::Testimony,
    },
};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH, Duration};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsensusBlock {
    basic: BlockHeader,
    testimony_merkle_root: H256,
    inter_parent_merkle_root: H256,
    global_parent_merkle_root: H256,
}


impl Hashable for ConsensusBlock {
    fn hash(&self) -> H256 {
        let info_vec = self.get_info_hash();
        let info_hash = H256::multi_hash(&info_vec);
        let inner_hash = H256::chash(&self.get_parent(), &info_hash);
        H256::pow_hash(&inner_hash, self.get_nonce() as u32)
    }
}

impl Info for ConsensusBlock {
    fn get_parent(&self) -> H256 {
        self.basic.get_parent()
    }
    fn get_difficulty(&self) -> H256 {
        self.basic.get_difficulty()
    }
    fn set_difficulty(&mut self, difficulty: &H256) {
        self.basic.set_difficulty(&difficulty);
    }
    fn get_nonce(&self) -> usize {
        self.basic.get_nonce()
    }
    fn get_timestamp(&self) -> SystemTime {
        self.basic.get_timestamp()
    }
    fn get_shard_id(&self) -> usize {
        self.basic.get_shard_id()
    }
    fn get_tx_merkle_root(&self) -> H256 {
        self.basic.get_tx_merkle_root()
    }
    fn get_info_hash(&self) -> Vec<H256> {
        let mut basic_info = self.basic.get_info_hash();
        basic_info.push(self.testimony_merkle_root.clone());
        basic_info.push(self.inter_parent_merkle_root.clone());
        basic_info.push(self.global_parent_merkle_root.clone());
        basic_info
    }
}

impl Default for ConsensusBlock {
    fn default() -> Self {
        ConsensusBlock {
            basic: BlockHeader::default(),
            testimony_merkle_root: H256::default(),
            inter_parent_merkle_root: H256::default(),
            global_parent_merkle_root: H256::default(),
        }
    }
}

impl ConsensusBlock {
    pub fn create(
        basic: BlockHeader,
        testimony_merkle_root: H256,
        inter_parent_merkle_root: H256,
        global_parent_merkle_root: H256,
    ) -> Self{
        ConsensusBlock {
            basic,
            testimony_merkle_root,
            inter_parent_merkle_root,
            global_parent_merkle_root
        }
    }

    pub fn get_mem_size() -> usize {
        BlockHeader::get_mem_size() 
            + H256::get_mem_size() * 3
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
    ) -> (ConsensusBlock, TransactionBlock) {
        let tx_block = TransactionBlock::new(
           shard_id,
            txs,
            tmys
        );
        let parent = verified_parent;

        let tx_merkle_root = tx_block.get_tx_merkle_root();
        let tmy_merkle_root = tx_block.get_testimony_merkle_root();

        let inter_parent_merkle_root = H256::multi_hash(&inter_parents);

        let chains: Vec<H256> = global_parents
            .into_iter()
            .map(|x| H256::multi_hash(&x.0))
            .collect();
        let global_parent_merkle_root = H256::multi_hash(&chains);

        let block_header = BlockHeader::create(
            parent,
            nonce,
            difficulty,
            shard_id,
            SystemTime::now(),
            tx_merkle_root.clone()
        );

        let cons_block = Self::create(
            block_header,
            tmy_merkle_root,
            inter_parent_merkle_root,
            global_parent_merkle_root
        );

        (cons_block, tx_block)
    }


    pub fn get_testimony_merkle_root(&self) -> H256 {
        self.testimony_merkle_root.clone()
    }

    pub fn get_inter_parent_merkle_root(&self) -> H256 {
        self.inter_parent_merkle_root.clone()
    } 

    pub fn get_global_parent_merkle_root(&self) -> H256 {
        self.global_parent_merkle_root.clone()
    }

    pub fn get_verified_parent(&self) -> H256 {
        self.get_parent()
    }

    pub fn set_nonce(&mut self, nonce: usize) {
        self.basic.set_nonce(nonce);
    }

    pub fn set_shard_id(&mut self, shard_id: usize) {
        self.basic.set_shard_id(shard_id);
    }
}

