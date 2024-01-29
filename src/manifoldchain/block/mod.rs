pub mod exclusive_block;
pub mod inclusive_block;
pub mod transaction_block;
pub mod versa_block;
pub mod consensus_block;

use serde::{Serialize, Deserialize};
use crate::{
    types::{
        hash::{H256, Hashable}, 
        merkle::MerkleTree
    },
    manifoldchain::transaction::Transaction,
};
use std::time::{SystemTime, UNIX_EPOCH, Duration};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockHeader {
    parent: H256,
    nonce: u32,
    difficulty: H256,
    shard_id: u32,
    timestamp: SystemTime,
    merkle_root: H256,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockContent {
    txs: MerkleTree<Transaction>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    header: BlockHeader,
    content: BlockContent,
    hash: H256,
}


impl Hashable for BlockHeader {
    fn hash(&self) -> H256 {
        let info_vec = self.get_info_hash(); 
        let info_hash: H256 = H256::multi_hash(&info_vec);
        let inner_hash: H256 = H256::chash(&self.parent, &info_hash);
        let outter_hash: H256 = H256::pow_hash(&inner_hash, self.nonce);
        outter_hash
    }
}

impl Default for BlockHeader {
    fn default() -> Self {
        BlockHeader {
            parent: H256::default(),
            nonce: 0 as u32,
            difficulty: H256::default(),
            timestamp: SystemTime::from(UNIX_EPOCH + Duration::new(0,0)),
            merkle_root: H256::default(),
            shard_id: 0 as u32,
        }
    }
}

impl Default for BlockContent {
    fn default() -> Self {
        let txs: Vec<Transaction> = vec![];
        BlockContent {
            txs: MerkleTree::<Transaction>::new(&txs),
        }
    }
}

impl BlockHeader {
    pub fn create(
        parent: H256, 
        nonce: usize, 
        difficulty: H256, 
        shard_id: usize, 
        timestamp: SystemTime,
        merkle_root: H256
    ) -> Self {
        BlockHeader {
            parent, 
            nonce: nonce as u32,
            difficulty,
            shard_id: shard_id as u32,
            timestamp,
            merkle_root
        }
    }
    pub fn get_mem_size() -> usize {
        H256::get_mem_size() * 3
            + std::mem::size_of::<u32>() * 2
            + std::mem::size_of::<SystemTime>()
    }
    pub fn set_nonce(&mut self, nonce: usize) {
        self.nonce = nonce as u32;
    }
    pub fn set_shard_id(&mut self, shard_id: usize) {
        self.shard_id = shard_id as u32;
    }
}

impl Hashable for Block {
    fn hash(&self) -> H256 {
        self.hash.clone()
    }
}

pub trait Content {
    fn get_txs(&self) -> Vec<Transaction>;
    fn get_txs_ref(&self) -> &Vec<Transaction>;
}

impl Content for BlockContent {
    fn get_txs(&self) -> Vec<Transaction> {
        self.txs.data.clone()
    }
    fn get_txs_ref(&self) -> &Vec<Transaction> {
        &self.txs.data
    }
}

impl BlockContent {
    pub fn get_tx_merkle_root(&self) -> H256 {
        self.txs.root.clone()
    }
    pub fn get_tx_merkle_proof(&self, tx_index: usize) -> Vec<H256> {
        self.txs.proof(tx_index)
    }
}

impl Content for Block {
    fn get_txs(&self) -> Vec<Transaction> {
        self.content.get_txs()
    }
    fn get_txs_ref(&self) -> &Vec<Transaction> {
        self.content.get_txs_ref()
    }
}

pub trait Info {
    fn get_parent(&self) -> H256;
    fn get_difficulty(&self) -> H256;
    fn set_difficulty(&mut self, difficulty: &H256);
    fn get_nonce(&self) -> usize;
    fn get_timestamp(&self) -> SystemTime;
    fn get_shard_id(&self) -> usize;
    fn get_tx_merkle_root(&self) -> H256;
    fn get_info_hash(&self) -> Vec<H256>;
}

impl Info for BlockHeader {
    fn get_parent(&self) -> H256 {
        self.parent.clone()
    }
    fn get_difficulty(&self) -> H256 {
        self.difficulty.clone()
    }
    fn set_difficulty(&mut self, difficulty: &H256) {
        self.difficulty = difficulty.clone();
    }
    fn get_nonce(&self) -> usize {
        self.nonce as usize
    }
    fn get_timestamp(&self) -> SystemTime {
        self.timestamp.clone()
    }
    fn get_tx_merkle_root(&self) -> H256 {
        self.merkle_root.clone()
    }
    fn get_shard_id(&self) -> usize {
        self.shard_id as usize
    }
    fn get_info_hash(&self) -> Vec<H256> {
        let time_str = format!("{:?}", self.timestamp);
        let time_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256,
            time_str.as_bytes()
        ).into();
        let shard_id_hash :H256 = ring::digest::digest(
            &ring::digest::SHA256,
            &self.shard_id.to_be_bytes()
        ).into();
        vec![
            self.difficulty.clone(),
            time_hash,
            shard_id_hash,
            self.merkle_root.clone(),
        ]
    }
}

impl Info for Block {
    fn get_parent(&self) -> H256 {
        self.header.get_parent()
    }
    fn get_difficulty(&self) -> H256 {
        self.header.get_difficulty()
    }
    fn set_difficulty(&mut self, difficulty: &H256) {
        self.header.set_difficulty(difficulty);
    }
    fn get_nonce(&self) -> usize {
        self.header.get_nonce()
    }
    fn get_timestamp(&self) -> SystemTime {
        self.header.get_timestamp()
    }
    fn get_shard_id(&self) -> usize {
        self.header.get_shard_id()
    }
    fn get_tx_merkle_root(&self) -> H256 {
        self.header.get_tx_merkle_root()
    }
    fn get_info_hash(&self) -> Vec<H256> {
        self.header.get_info_hash()
    }
}

impl Default for Block {
    fn default() -> Self {
        let parent: H256 = H256::from(&[0; 32]);
        let nonce: u32 = 0;
        let difficulty: H256 = H256::from(&[0; 32]);
        let zero_time = UNIX_EPOCH + Duration::new(0, 0);
        let timestamp: SystemTime = SystemTime::from(zero_time);
        let txs: Vec<Transaction> = Vec::new();
        let merkle_tree: MerkleTree<Transaction> = MerkleTree::new(txs.as_slice());

        let header: BlockHeader = BlockHeader {
            parent,
            nonce,
            difficulty,
            timestamp,
            merkle_root: merkle_tree.root(),
            shard_id: 0,
        };
        let content: BlockContent = BlockContent {
            txs: merkle_tree,
        };

        let hash: H256 = header.hash();

        Block {
            header,
            content,
            hash,
        }
    }
}

impl Block {
    pub fn verify_hash(blk: &Block) -> bool {
        blk.hash() == blk.header.hash()
    }

    pub fn construct(parent: H256, timestamp: SystemTime, difficulty: H256,
        txs: MerkleTree<Transaction>, nonce: u32, shard_id: u32) -> Block {
        let header: BlockHeader = BlockHeader {
            parent, 
            nonce,
            difficulty,
            timestamp,
            merkle_root: txs.root(),
            shard_id,
        };

        let content: BlockContent = BlockContent {
            txs
        };

        let hash: H256 = header.hash();

        Block {
            header, 
            content,
            hash,
        }
    }

    
}



//#[cfg(test)]
//mod tests {
//    use super::*;
//    use crate::types::hash::H256;
//    use rand::Rng;
//
//    #[cfg(any(test, test_utilities))]
//    pub fn generate_random_block(parent: &H256) -> Block {
//        let nonce: u32 = rand::thread_rng().gen();
//        let difficulty: H256 = H256::from(&[0; 32]);
//        let timestamp: SystemTime = SystemTime::now();
//        let txs: Vec<Transaction> = Vec::new();
//        let merkle_tree: MerkleTree<Transaction> = MerkleTree::new(txs.as_slice());
//
//        let header: BlockHeader = BlockHeader {
//            parent: parent.clone(),
//            nonce,
//            difficulty,
//            timestamp,
//            merkle_root: merkle_tree.root(),
//        };
//        let content: BlockContent = BlockContent {
//            txs: merkle_tree,
//        };
//
//        let hash: H256 = header.hash();
//
//        Block {
//            header,
//            content,
//            hash,
//        }
//    }
//    #[cfg(any(test, test_utilities))]
//    pub fn generate_random_block(parent: &H256) -> Block {
//        let nonce: u32 = rand::thread_rng().gen();
//        let difficulty: H256 = H256::from(&[0; 32]);
//        let timestamp: SystemTime = SystemTime::now();
//        let txs: Vec<Transaction> = Vec::new();
//        let merkle_tree: MerkleTree<Transaction> = MerkleTree::new(txs.as_slice());
//
//        let header: BlockHeader = BlockHeader {
//            parent: parent.clone(),
//            nonce,
//            difficulty,
//            timestamp,
//            merkle_root: merkle_tree.root(),
//        };
//        let content: BlockContent = BlockContent {
//            txs: merkle_tree,
//        };
//
//        let hash: H256 = header.hash();
//
//        Block {
//            header,
//            content,
//            hash,
//        }
//    }
//
//    #[test]
//    fn generate_block() {
//        let parent: H256 = H256::from(&[0; 32]);
//        let blk: Block = generate_random_block(&parent);
//        assert_eq!(blk.header.parent, parent);
//        assert_eq!(blk.header.merkle_root, H256::from(&[0; 32]));
//    }
//}
