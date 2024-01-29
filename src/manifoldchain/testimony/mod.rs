use crate::{
    types::{
        hash::{H256, Hashable},
    },
    manifoldchain::{
        transaction::{Transaction, TxFlag},
        block::{
            versa_block::{
                VersaBlock,
                ExclusiveFullBlock,
                InclusiveFullBlock,
            },
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
        },
        validator::Validator,
    }
};
use serde::{Serialize, Deserialize};
use rand::{self, Rng};

#[derive(Serialize, Deserialize, Clone, Eq, Hash, PartialEq, Debug)]
pub struct TestimonyUnit {
    input_hash: H256,
    originate_block_hash: H256,
    tx_merkle_proof: Vec<H256>,
    tx_index: u32,
}


impl Hashable for TestimonyUnit {
    fn hash(&self) -> H256 {
        let mut hash_vec = self.tx_merkle_proof.clone();
        hash_vec.push(self.originate_block_hash.clone());
        hash_vec.push(self.input_hash.clone());
        H256::multi_hash(&hash_vec)
    }
}

impl TestimonyUnit {
    pub fn create(
        input_hash: H256,
        originate_block_hash: H256,
        tx_merkle_proof: Vec<H256>,
        tx_index: usize,
    ) -> Self {
        TestimonyUnit {
            input_hash,
            originate_block_hash,
            tx_merkle_proof,
            tx_index: tx_index as u32,
        }
    }
    pub fn get_mem_size(&self) -> usize {
        H256::get_mem_size() * (self.tx_merkle_proof.len() + 2) + 
            std::mem::size_of::<u32>()
    }
    pub fn get_input_index(&self) -> usize {
        self.tx_index as usize
    }
    pub fn get_ori_blk_hash(&self) -> H256 {
        self.originate_block_hash.clone()
    }
    pub fn get_tx_merkle_proof(&self) -> Vec<H256> {
        self.tx_merkle_proof.clone()
    }
    pub fn get_tx_index(&self) -> usize {
        self.tx_index as usize
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct Testimony {
    tx_hash: H256,
    units: Vec<TestimonyUnit>,
}

impl Default for Testimony {
    fn default() -> Self {
        Self {
            tx_hash: H256::default(),
            units: vec![],
        }
    }
}

impl Hashable for Testimony {
    fn hash(&self) -> H256 {
        let mut units_hash: Vec<H256> = self.units
            .clone()
            .into_iter()
            .map(|x| x.hash())
            .collect();
        units_hash.push(self.tx_hash.clone());
        H256::multi_hash(&units_hash)
    }
}

impl Testimony {
    pub fn create(
        tx_hash: H256,
        units: Vec<TestimonyUnit>
    ) -> Self {
        Testimony {
            tx_hash,
            units,
        }
    }
    pub fn get_mem_size(&self) -> usize {
        let mut unit_mem_size = 0;
        for unit in self.units.iter() {
            unit_mem_size += unit.get_mem_size();
        }
        unit_mem_size + H256::get_mem_size()
    }
    pub fn generate(
        tx: &Transaction,
        block: &VersaBlock,
        tx_index: usize,
        shard_id: usize,
        shard_num: usize,
        decision: bool,
    ) -> Option<Testimony> {
        let mut tmy_units: Vec<TestimonyUnit> = vec![];
        match &tx.flag {
            &TxFlag::Input => {
                for input in tx.inputs.iter() {
                    if Validator::get_shard_id(
                        &input.sender_addr,
                        shard_num
                    ) == shard_id {
                        match block.get_tx_merkle_proof(tx_index) {
                            Some(proof) => {
                                let tmy_unit = TestimonyUnit::create(
                                    input.hash(),
                                    block.hash(),
                                    proof,
                                    tx_index,
                                );
                                tmy_units.push(tmy_unit);
                            }
                            None => {
                                return None;
                            }
                        }
                    }
                }
                let mut tx_mod = tx.clone();
                tx_mod.flag = TxFlag::Output;
                let tmy = Testimony::create(
                    tx_mod.hash(),
                    tmy_units
                );
                Some(tmy)
            }
            &TxFlag::Output => {
                for output in tx.outputs.iter() {
                    if Validator::get_shard_id(
                        &output.receiver_addr,
                        shard_num
                    ) == shard_id {
                        match block.get_tx_merkle_proof(tx_index) {
                            Some(proof) => {
                                let tmy_unit = TestimonyUnit::create(
                                    output.hash(),
                                    block.hash(),
                                    proof,
                                    tx_index,
                                );
                                tmy_units.push(tmy_unit);
                            }
                            None => {
                                return None;
                            }
                        }
                    }
                }
                let mut tx_mod = tx.clone();
                tx_mod.flag = match decision {
                    true => TxFlag::Accept,
                    false => TxFlag::Reject,
                };
                let tmy = Testimony::create(
                    tx_mod.hash(),
                    tmy_units
                );
                Some(tmy)
            }
            _ => {
                None
            }
        } 
    }
    pub fn gen_rand_tmy() -> Self {
        let mut rng = rand::thread_rng();
        let bytes: [u8; 32] = rng.gen();
        let rand_hash: H256 = (&bytes).into();
        Testimony {
            tx_hash: rand_hash,
            units: Vec::new()
        }
    }
    pub fn gen_rand_tmy_by_tx(tx_hash: &H256) -> Self {
        Testimony {
            tx_hash: tx_hash.clone(),
            units: Vec::new()
        }
    }
    pub fn get_tx_hash(&self) -> H256 {
        self.tx_hash.clone()
    }
    pub fn get_ori_blk_hash(&self, input_hash: H256) -> Option<H256> {
        for unit in self.units.iter() {
            if unit.input_hash == input_hash {
                return Some(unit.get_ori_blk_hash());
            }
        }
        None
    }
    pub fn get_tx_merkle_proof(&self, input_hash: H256) -> Option<Vec<H256>> {
        for unit in self.units.iter() {
            if unit.input_hash == input_hash {
                return Some(unit.get_tx_merkle_proof());
            }
        }
        None
    }
    pub fn get_tx_index(&self, input_hash: H256) -> Option<usize> {
        for unit in self.units.iter() {
            if unit.input_hash == input_hash {
                return Some(unit.get_tx_index());
            }
        }
        None
    }

    pub fn get_tmy_unit(&self, input_hash: &H256) -> Option<TestimonyUnit> {
        for unit in self.units.iter() {
            if unit.input_hash == *input_hash {
                return Some(unit.clone());
            }
        }
        None
    }

    pub fn get_tmy_units(&self) -> Vec<TestimonyUnit> {
        self.units.clone()
    }

}
