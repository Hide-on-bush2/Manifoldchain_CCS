use crate::{
    types::{
        hash::{H256, Hashable}
    },
    manifoldchain::{
        transaction::Transaction,
        testimony::Testimony,
    }
};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub enum FraudProof {
    DoubleSpending(DoubleSpendingProof),
    UtxoLost(UtxoLostProof),
    WrongShard(WrongShardProof),
    UnequalCoins(UnequalCoinsProof),
    WrongSignature(WrongSignatureProof),
    TestimonyLost(TestimonyLostProof),
    WrongTestimony(WrongTestimonyProof),
    UnsolvedFault,
}

impl Hashable for FraudProof {
    fn hash(&self) -> H256 {
        match self {
            FraudProof::DoubleSpending(fp) => fp.hash(),
            FraudProof::UtxoLost(fp) => fp.hash(),
            FraudProof::WrongShard(fp) => fp.hash(),
            FraudProof::UnequalCoins(fp) => fp.hash(),
            FraudProof::WrongSignature(fp) => fp.hash(),
            FraudProof::TestimonyLost(fp) => fp.hash(),
            FraudProof::WrongTestimony(fp) => fp.hash(),
            FraudProof::UnsolvedFault => H256::default(),
        }
    }
}

impl FraudProof {
    pub fn get_invalid_block(&self) -> H256 {
        match self {
            FraudProof::DoubleSpending(fp) => fp.invalid_block_hash.clone(),
            FraudProof::UtxoLost(fp) => fp.block_hash.clone(),
            FraudProof::WrongShard(fp) => fp.block_hash.clone(),
            FraudProof::UnequalCoins(fp) => fp.block_hash.clone(),
            FraudProof::WrongSignature(fp) => fp.invalid_block_hash.clone(),
            FraudProof::TestimonyLost(fp) => fp.block_hash.clone(),
            FraudProof::WrongTestimony(fp) => fp.block_hash.clone(),
            FraudProof::UnsolvedFault => H256::default(),
        }
    }

    pub fn get_shard_id(&self) -> usize {
        match self {
            FraudProof::DoubleSpending(fp) => fp.shard_id.clone() as usize,
            FraudProof::UtxoLost(fp) => fp.shard_id.clone() as usize,
            FraudProof::WrongShard(fp) => fp.shard_id.clone() as usize,
            FraudProof::UnequalCoins(fp) => fp.shard_id.clone() as usize,
            FraudProof::WrongSignature(fp) => fp.shard_id.clone() as usize,
            FraudProof::TestimonyLost(fp) => fp.shard_id.clone() as usize,
            FraudProof::WrongTestimony(fp) => fp.shard_id.clone() as usize,
            FraudProof::UnsolvedFault => 0,
        }
    }

    pub fn get_invalid_tx(&self) -> Transaction {
         match self {
            FraudProof::DoubleSpending(fp) => fp.invalid_tx.clone(),
            FraudProof::UtxoLost(fp) => fp.invalid_tx.clone(),
            FraudProof::WrongShard(fp) => fp.invalid_tx.clone(),
            FraudProof::UnequalCoins(fp) => fp.invalid_tx.clone(),
            FraudProof::WrongSignature(fp) => fp.invalid_tx.clone(),
            FraudProof::TestimonyLost(fp) => fp.invalid_tx.clone(),
            FraudProof::WrongTestimony(fp) => fp.invalid_tx.clone(),
            FraudProof::UnsolvedFault => Transaction::default(),
        }       
    }

    pub fn get_invalid_tx_merkle_proof(&self) -> Vec<H256> {
         match self {
            FraudProof::DoubleSpending(fp) => fp.invalid_tx_merkle_proof.clone(),
            FraudProof::UtxoLost(fp) => fp.invalid_tx_merkle_proof.clone(),
            FraudProof::WrongShard(fp) => fp.invalid_tx_merkle_proof.clone(),
            FraudProof::UnequalCoins(fp) => fp.invalid_tx_merkle_proof.clone(),
            FraudProof::WrongSignature(fp) => fp.invalid_tx_merkle_proof.clone(),
            FraudProof::TestimonyLost(fp) => fp.invalid_tx_merkle_proof.clone(),
            FraudProof::WrongTestimony(fp) => fp.invalid_tx_merkle_proof.clone(),
            FraudProof::UnsolvedFault => vec![],
        }
    }

    pub fn get_invalid_index(&self) -> usize {
        match self {
            FraudProof::DoubleSpending(fp) => fp.invalid_index.clone() as usize,
            FraudProof::UtxoLost(fp) => fp.invalid_index.clone() as usize,
            FraudProof::WrongShard(fp) => fp.invalid_index.clone() as usize,
            FraudProof::UnequalCoins(fp) => fp.invalid_index.clone() as usize,
            FraudProof::WrongSignature(fp) => fp.invalid_index.clone() as usize,
            FraudProof::TestimonyLost(fp) => fp.invalid_index.clone() as usize,
            FraudProof::WrongTestimony(fp) => fp.invalid_index.clone() as usize,
            FraudProof::UnsolvedFault => 0,
        }
    }
}


#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct DoubleSpendingProof {
    pub shard_id: u32,

    pub invalid_block_hash: H256,
    pub invalid_tx: Transaction,
    pub invalid_tx_merkle_proof: Vec<H256>,
    pub invalid_index: u32,

    pub conflict_tx: Transaction,
    pub conflict_block_hash: H256,
    pub conflict_tx_merkle_proof: Vec<H256>,
    pub conflict_index: u32,
}

impl Hashable for DoubleSpendingProof {
    fn hash(&self) -> H256 {
        let str = format!("{}{}{}", 
            self.shard_id,
            self.invalid_index,
            self.conflict_index,
        );
        let str_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, str.as_bytes()
        ).into();
        let mut tmp_vec: Vec<H256> = vec![
            str_hash, 
            self.invalid_block_hash.clone(),
            self.invalid_tx.hash(),
            self.conflict_tx.hash(),
            self.conflict_block_hash.clone(),
        ];
        tmp_vec.extend(self.invalid_tx_merkle_proof.clone());
        tmp_vec.extend(self.conflict_tx_merkle_proof.clone());
        H256::multi_hash(&tmp_vec)
    }
}

//...
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct UtxoLostProof {
    pub shard_id: u32,
    pub block_hash: H256,

    pub invalid_tx: Transaction,
    pub invalid_tx_merkle_proof: Vec<H256>,
    pub invalid_index: u32
}

impl Hashable for UtxoLostProof {
    fn hash(&self) -> H256 {
        let str = format!("{}{}", self.shard_id, self.invalid_index);
        let str_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, str.as_bytes()
        ).into();

        let mut tmp_vec: Vec<H256> = vec![
            str_hash,
            self.block_hash.clone(),
            self.invalid_tx.hash(),
        ];

        tmp_vec.extend(self.invalid_tx_merkle_proof.clone());
        H256::multi_hash(&tmp_vec)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct WrongShardProof {
    pub shard_id: u32,
    pub block_hash: H256,

    pub invalid_tx: Transaction,
    pub invalid_tx_merkle_proof: Vec<H256>,
    pub invalid_index: u32,
}

impl Hashable for WrongShardProof {
    fn hash(&self) -> H256 {
        let str = format!("{}{}", self.shard_id, self.invalid_index);
        let str_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, str.as_bytes()
        ).into();

        let mut tmp_vec: Vec<H256> = vec![
            str_hash,
            self.block_hash.clone(),
            self.invalid_tx.hash(),
        ];

        tmp_vec.extend(self.invalid_tx_merkle_proof.clone());
        H256::multi_hash(&tmp_vec)
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct UnequalCoinsProof {
    pub shard_id: u32,
    pub block_hash: H256,

    pub invalid_tx: Transaction,
    pub invalid_tx_merkle_proof: Vec<H256>,
    pub invalid_index: u32,
}

impl Hashable for UnequalCoinsProof {
    fn hash(&self) -> H256 {
        let str = format!("{}{}", self.shard_id, self.invalid_index);
        let str_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, str.as_bytes()
        ).into();

        let mut tmp_vec: Vec<H256> = vec![
            str_hash,
            self.block_hash.clone(),
            self.invalid_tx.hash(),
        ];

        tmp_vec.extend(self.invalid_tx_merkle_proof.clone());
        H256::multi_hash(&tmp_vec)
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct WrongSignatureProof {
    pub shard_id: u32,

    pub invalid_block_hash: H256,
    pub invalid_tx: Transaction,
    pub invalid_tx_merkle_proof: Vec<H256>,
    pub invalid_index: u32,

    pub conflict_block_hash: H256,
    pub conflict_tx: Transaction,   
    pub conflict_tx_merkle_proof: Vec<H256>,
    pub conflict_index: u32,
}

impl Hashable for WrongSignatureProof {
    fn hash(&self) -> H256 {
        let str = format!("{}{}{}", 
            self.shard_id,
            self.invalid_index,
            self.conflict_index,
        );
        let str_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, str.as_bytes()
        ).into();
        let mut tmp_vec: Vec<H256> = vec![
            str_hash, 
            self.invalid_block_hash.clone(),
            self.invalid_tx.hash(),
            self.conflict_tx.hash(),
            self.conflict_block_hash.clone(),
        ];
        tmp_vec.extend(self.invalid_tx_merkle_proof.clone());
        tmp_vec.extend(self.conflict_tx_merkle_proof.clone());
        H256::multi_hash(&tmp_vec)
    }
}
//...
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct TestimonyLostProof {
    pub shard_id: u32,
    pub block_hash: H256,
    
    pub invalid_tx: Transaction,
    pub invalid_tx_merkle_proof: Vec<H256>,
    pub invalid_index: u32,
}

impl Hashable for TestimonyLostProof {
    fn hash(&self) -> H256 {
        let str = format!("{}{}", self.shard_id, self.invalid_index);
        let str_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, str.as_bytes()
        ).into();

        let mut tmp_vec: Vec<H256> = vec![
            str_hash,
            self.block_hash.clone(),
            self.invalid_tx.hash(),
        ];

        tmp_vec.extend(self.invalid_tx_merkle_proof.clone());
        H256::multi_hash(&tmp_vec)
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct WrongTestimonyProof {
    pub shard_id: u32,
    pub block_hash: H256,

    pub invalid_tx: Transaction,
    pub invalid_tx_merkle_proof: Vec<H256>,
    pub invalid_index: u32,

    pub invalid_tmy: Testimony,
    pub invalid_tmy_merkle_proof: Vec<H256>,
    pub invalid_tmy_index: u32,
}

impl Hashable for WrongTestimonyProof {
    fn hash(&self) -> H256 {
        let str = format!("{}{}{}", 
            self.shard_id, 
            self.invalid_index, 
            self.invalid_tmy_index
        );
        let str_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, str.as_bytes()
        ).into();

        let mut tmp_vec: Vec<H256> = vec![
            str_hash,
            self.block_hash.clone(),
            self.invalid_tx.hash(),
            self.invalid_tmy.hash(),
        ];

        tmp_vec.extend(self.invalid_tx_merkle_proof.clone());
        tmp_vec.extend(self.invalid_tmy_merkle_proof.clone());
        H256::multi_hash(&tmp_vec)
    }
}

