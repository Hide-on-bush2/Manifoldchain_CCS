use crate::{
    types::hash::{
        H256, Hashable,
    },
    manifoldchain::{
        transaction::{Transaction, TxFlag},
        testimony::{
            Testimony,
            TestimonyUnit,
        },
        database::Database,
        configuration::Configuration,
    },
};
use std::collections::{VecDeque, HashMap};
use log::{info, debug};
use std::time::{SystemTime};

pub struct Mempool {
    //txs_map: HashMap<H256, Transaction>, //the key is the hash of the tx, while value is the
    txs_map: Database<Transaction>,
    //testimony_map: HashMap<H256, Testimony>, //the key is the hash of the testimony,
    //while the value is the testimony of the transaction
    testimony_map: Database<Testimony>,
    //exact value of tx
    txs_queue: VecDeque<H256>,
    tx2tmy: HashMap<H256, H256>,
}


impl Mempool {
    pub fn new() -> Self {
        let now = SystemTime::now();
        let txs_map: Database<Transaction> = 
            Database::<Transaction>::new(format!("{:?}/mempool/txs_map", now));
        let testimony_map: Database<Testimony> =
            Database::<Testimony>::new(format!("{:?}/mempool/testimony_map", now));
        Mempool {
            txs_map,
            testimony_map,
            txs_queue: VecDeque::new(),
            tx2tmy: HashMap::new(),
        }
    }

    pub fn get_size(&self) -> usize {
        self.txs_queue.len()
    }

//    pub fn get_tmy_size(&self) -> usize {
//        self.testimony_map.len()
//    }

    pub fn get_queue_size(&self) -> usize {
        self.txs_queue.len()
    }
    
    pub fn insert_tx(&mut self, tx: Transaction) -> bool {
        let hash: H256 = tx.hash();
        if self.txs_map.contains_key(&hash) {
            false
        } else {
            let _ = self.txs_map.insert(hash.clone(), tx.clone());
            if tx.flag == TxFlag::Initial {
                self.txs_queue.push_back(hash); 
            } else {
                let mut new_queue: VecDeque<H256> = VecDeque::new();
                while !self.txs_queue.is_empty() {
                    let tx_hash = self.txs_queue.front().unwrap();
                    let tx = self.txs_map.get(tx_hash).unwrap();
                    match &tx.flag {
                        TxFlag::Initial => {
                            break;
                        }
                        _ => {
                            new_queue.push_back(tx_hash.clone());
                            self.txs_queue.pop_front();
                        }
                    }
                }
                new_queue.push_back(hash);
                while !self.txs_queue.is_empty() {
                    let tx_hash = self.txs_queue.pop_front().unwrap();
                    new_queue.push_back(tx_hash);
                }
                self.txs_queue = new_queue;
            }
            true
        }
    }

    pub fn check(&self, hash: &H256) -> bool {
        self.txs_map.contains_key(hash)
    }
        
    pub fn get_tx(&self, hash: &H256) -> Option<Transaction> {
        if self.check(hash) {
            Some(self.txs_map.get(hash).unwrap().clone())
        } else {
            None
        }
    }

    pub fn get_all_txs(&self) -> Vec<Transaction> {
        let mut res: Vec<Transaction> = Vec::new();
        for (_, val) in self.txs_map.iter() {
            res.push(val.clone());
        }
        res
    }

    pub fn delete_txs(&mut self, tx_hashs: Vec<H256>) -> bool {
        for tx_hash in tx_hashs.iter() {
            self.txs_map.remove(&tx_hash);
            self.txs_queue.retain(|x| x != tx_hash);
        }
        true
    }

    

    pub fn pop_one_tx(&mut self) -> (Option<Transaction>, Option<Testimony>) {
        if self.txs_queue.is_empty() {
            (None, None)
        } else {
            let tx_hash = self.txs_queue.pop_front().unwrap();
            let tx = self.txs_map.get(&tx_hash).unwrap().clone();
            self.txs_map.remove(&tx_hash);
            
            if let Some(tmy_hash) = self.tx2tmy.get(&tx_hash) {
                let tmy = self.testimony_map.get(&tmy_hash).unwrap().clone();
                self.testimony_map.remove(&tmy_hash);
                self.tx2tmy.remove(&tx_hash);
                (Some(tx), Some(tmy.clone()))
            } else {
                (Some(tx), None)
            }
        }
    }

    pub fn get_all_tx_hash(&self) -> Vec<H256> {
        let mut res: Vec<H256> = Vec::new();
        for (_, val) in self.txs_map.iter() {
            res.push(val.hash());
        }
        res
    }

    pub fn add_testimony(&mut self, tmy: Testimony) -> bool {
        let tx_hash = tmy.get_tx_hash();
        let tmy_hash = tmy.hash();
        let inserted_tmy = match self.tx2tmy.get(&tx_hash) {
            Some(old_tmy_hash) => {
                let old_tmy = self.testimony_map.get(old_tmy_hash).unwrap();
                let old_tmy_units = old_tmy.get_tmy_units();
                let new_tmy_units = tmy.get_tmy_units();
                let mut tmy_units_set: HashMap<TestimonyUnit, bool> = HashMap::new();
                for unit in old_tmy_units {
                    tmy_units_set.insert(unit, true);
                }
                for unit in new_tmy_units {
                    tmy_units_set.insert(unit, true);
                }
                let union_units: Vec<TestimonyUnit> = tmy_units_set
                    .into_iter()
                    .map(|(key, val)| key.clone())
                    .collect();
                Testimony::create(
                    tx_hash.clone(),
                    union_units
                )
            }
            None => tmy,
        };
        let inserted_tmy_hash = inserted_tmy.hash();
        let mut is_update = false;
        //if the old tmy is update
        if inserted_tmy_hash != tmy_hash {
            self.testimony_map.remove(&tmy_hash);
            is_update = true;
        }
        
        let _ = self.testimony_map.insert(inserted_tmy_hash.clone(), inserted_tmy);
        self.tx2tmy.insert(tx_hash.clone(), inserted_tmy_hash);
        is_update
    }
    pub fn remove_testimony(&mut self, tmy_hash: &H256) -> bool {
        self.testimony_map.remove(tmy_hash);
        self.tx2tmy.retain(|_, val| !(*val).eq(tmy_hash));
        true
    }
    pub fn get_testimony_by_tx(&self, tx_hash: &H256) -> Option<Testimony> {
        if let Some(tmy_hash) = self.tx2tmy.get(tx_hash) {
            match self.testimony_map.get(tmy_hash) {
                Some(tmy) => Some(tmy.clone()),
                None => None,
            }
        } else {
            None
        }
    }
    pub fn get_testimony(&self, tmy_hash: &H256) -> Option<Testimony> {
        match self.testimony_map.get(tmy_hash) {
            Some(tmy) => Some(tmy.clone()),
            None => None,
        }
    }
    
}

