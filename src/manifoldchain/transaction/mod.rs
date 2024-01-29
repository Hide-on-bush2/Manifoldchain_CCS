pub mod generator;

use serde::{Serialize, Deserialize};
use ring::signature::{self, Ed25519KeyPair, Signature, KeyPair};
use crate::types::hash::{H256, Hashable};
use rand::{self, Rng};
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub enum TxFlag{
    Initial,
    Domestic,
    Input,
    Output,
    Accept,
    Reject,
    Empty, //if there are not enough txs in the mempool, miners will create enough 
    //empty txs to generate a block
}


impl Default for TxFlag {
    fn default() -> Self {
        TxFlag::Domestic
    }
}

impl ToString for TxFlag {
    fn to_string(&self) -> String {
        match self {
            TxFlag::Initial => String::from("initial"),
            TxFlag::Domestic => String::from("domestic"),
            TxFlag::Input => String::from("input"),
            TxFlag::Output => String::from("output"),
            TxFlag::Accept => String::from("accept"),
            TxFlag::Reject => String::from("reject"),
            TxFlag::Empty => String::from("empty"),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
pub struct Transaction {
    pub inputs: Vec<UtxoInput>,
    pub outputs: Vec<UtxoOutput>,
    pub flag: TxFlag,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, Hash, PartialEq)]
pub struct UtxoInput {
    pub sender_addr: H256,
    pub tx_hash: H256,
    pub value: u32,
    pub index: u32,
    pub sig_ref: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, Hash, PartialEq)]
pub struct UtxoOutput {
    pub receiver_addr: H256,
    pub value: u32,
    pub public_key_ref: Vec<u8>,
}

impl Hashable for UtxoInput {
    fn hash(&self) -> H256 {
        let sig_ref_str: String = serde_json::to_string(&self.sig_ref).unwrap();
        let input_str = format!("{}{}{}", 
            self.value,
            self.index,
            sig_ref_str,
        );
        let tmp_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, input_str.as_bytes()
        ).into();
        let hash_vec: Vec<H256> = vec![
            self.sender_addr.clone(), 
            tmp_hash, 
            self.tx_hash.clone()
        ];
        H256::multi_hash(&hash_vec)
    }
}
impl UtxoInput {
    pub fn get_mem_size(&self) -> usize {
        H256::get_mem_size() * 2 
            + std::mem::size_of::<u32>() * 2 
            + std::mem::size_of::<u8>() * self.sig_ref.len()
    }
}

impl Hashable for UtxoOutput {
    fn hash(&self) -> H256 {
        let public_key_ref_str: String = serde_json::to_string(&self.public_key_ref).unwrap();
        let output_str = format!("{}{}",
            self.value,
            public_key_ref_str
        );
        let tmp_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256, output_str.as_bytes()
        ).into();
        let hash_vec: Vec<H256> = vec![self.receiver_addr.clone(), tmp_hash];
        H256::multi_hash(&hash_vec)
    }
}

impl UtxoOutput {
    pub fn get_mem_size(&self) -> usize {
        H256::get_mem_size()
            + std::mem::size_of::<u32>() 
            + std::mem::size_of::<u8>() * self.public_key_ref.len()
    }
}

impl Hashable for Transaction {
    fn hash(&self) -> H256 {
        let mut input_hash_vec: Vec<H256> = self.inputs
            .clone()
            .into_iter()
            .map(|x| x.hash())
            .collect();
        let out_hash_vec: Vec<H256> = self.outputs
            .clone()
            .into_iter()
            .map(|x| x.hash())
            .collect();
        input_hash_vec.extend(out_hash_vec);
        let flag_hash: H256 = ring::digest::digest(
            &ring::digest::SHA256,
            self.flag.to_string().as_bytes()
        ).into();
        input_hash_vec.push(flag_hash);
        H256::multi_hash(&input_hash_vec)
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Transaction {
            inputs: vec![],
            outputs: vec![],
            flag: TxFlag::Domestic,
        }
    } 
}



impl Transaction {
    //generate a random transaction
    pub fn gen_rand_tx() -> Self {
        let mut rng = rand::thread_rng();
        let sender_addr: [u8; 32] = rng.gen();
        let sender_addr_hash: H256 = (&sender_addr).into();
        let rand_addr: [u8; 32] = rng.gen();
        let rand_addr_hash = (&rand_addr).into();
        let input = UtxoInput {
            sender_addr: sender_addr_hash,
            tx_hash: H256::default(),
            value: 0,
            index: 0,
            sig_ref: Vec::new()
        };
        let output = UtxoOutput {
            receiver_addr: rand_addr_hash,
            value: 0,
            public_key_ref: Vec::new()
        };
        let inputs: Vec<UtxoInput> = vec![input];
        let outputs: Vec<UtxoOutput> = vec![output];

        Transaction {
            inputs,
            outputs,
            flag: TxFlag::Initial,
        }
    }
    pub fn get_mem_size(&self) -> usize {
        let mut input_mem_size = 0;
        for input in self.inputs.iter() {
            input_mem_size += input.get_mem_size();
        }
        let mut output_mem_size = 0;
        for output in self.outputs.iter() {
            output_mem_size += output.get_mem_size();
        }
        input_mem_size + output_mem_size + std::mem::size_of::<TxFlag>()
    }
    /// Create digital signature of a transaction
    pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
        let serialized_tx: Vec<u8> = bincode::serialize(t).unwrap();
        key.sign(serialized_tx.as_slice())
    }

    /// Verify digital signature of a transaction, using public key instead of secret key
    pub fn verify(t: &Transaction, public_key_ref: &[u8], sig_ref: &[u8]) -> bool {
        let peer_public_key = signature::UnparsedPublicKey::new(
            &signature::ED25519, 
            public_key_ref
        );
        let serialized_tx: Vec<u8> = bincode::serialize(t).unwrap();
        let res = peer_public_key.verify(serialized_tx.as_slice(), sig_ref);
        match res {
            Ok(()) => {
                true
            }
            Err(_) => {
                false
            }
        }
    }

    pub fn verify_owner(
        tx: &Transaction,  
        input_txs: Vec<&Transaction>, 
    ) -> bool {
        if tx.inputs.len() != input_txs.len() {
            return false;
        }
        for i in 0..tx.inputs.len() {
            let input = &tx.inputs[i];
            let input_tx = input_txs[i];
            if !Self::verify(
                input_tx,
                &input_tx.outputs[input.index as usize].public_key_ref,
                &input.sig_ref,
            ) {
                return false;
            }
        }
        true
    }

    pub fn get_related_hash(&self, flag: TxFlag) -> H256 {
        let mut tx = self.clone();
        tx.flag = flag;
        tx.hash()
    }

    pub fn create_initial_tx(user: (&H256, &Ed25519KeyPair), value: usize) -> Transaction {
        let input = UtxoInput::default();
        let output = UtxoOutput {
            receiver_addr: user.0.clone(),
            value: value as u32,
            public_key_ref: user.1.public_key().as_ref().to_vec(),
        };
        Transaction {
            inputs: vec![input],
            outputs: vec![output],
            flag: TxFlag::Initial,
        }
    }

    pub fn create_empty_tx(num_input: usize, num_output: usize) -> Transaction {
        let inputs: Vec<UtxoInput> = (0..num_input)
            .into_iter()
            .map(|i| UtxoInput::default())
            .collect();
        let outputs: Vec<UtxoOutput> = (0..num_output)
            .into_iter()
            .map(|i| UtxoOutput::default())
            .collect();
        Transaction {
            inputs,
            outputs,
            flag: TxFlag::Empty,
        }
        
    }

    pub fn consume(
        utxos: Vec<(&Transaction, usize)>, //tx, index 
        senders: Vec<(&H256, &Ed25519KeyPair)>, //user_addr, user_key, sent_coin
        receivers: Vec<(&H256, &Ed25519KeyPair, usize)>, //user_addr, user_key, received_coin
        flag: TxFlag,
    ) -> Option<Transaction> {

        let mut inputs: Vec<UtxoInput> = vec![];
        let mut sent_coins = 0;
        for i in 0..utxos.len() {
            let x = utxos[i];
            let sender = senders[i];
            let tx = x.0;
            let index = x.1;
            let input = match &tx.flag {
                &TxFlag::Initial => {
                    UtxoInput {
                        sender_addr: sender.0.clone(),
                        tx_hash: tx.hash(),
                        value: tx.outputs[index].value,
                        index: index as u32,
                        sig_ref: Transaction::sign(tx, sender.1).as_ref().to_vec(),
                    }
                }
                &TxFlag::Domestic => {
                    UtxoInput {
                        sender_addr: sender.0.clone(),
                        tx_hash: tx.hash(),
                        value: tx.outputs[index].value,
                        index: index as u32,
                        sig_ref: Transaction::sign(tx, sender.1).as_ref().to_vec(),
                    }
                }
                &TxFlag::Output => {
                    UtxoInput {
                        sender_addr: sender.0.clone(),
                        tx_hash: tx.hash(),
                        value: tx.outputs[index].value,
                        index: index as u32,
                        sig_ref: Transaction::sign(tx, sender.1).as_ref().to_vec(),
                    }
                }
                &TxFlag::Reject => {
                    UtxoInput {
                        sender_addr: sender.0.clone(),
                        tx_hash: tx.hash(),
                        value: tx.inputs[index].value,
                        index: index as u32,
                        sig_ref: vec![],
                    }
                }
                _ => {
                    return None;
                }
            };
            inputs.push(input.clone());
            sent_coins += input.value as usize;
        }

        let mut outputs: Vec<UtxoOutput> = vec![];
        let mut received_coins = 0;
        for x in receivers {
            let output  = UtxoOutput {
                receiver_addr: x.0.clone(),
                value: x.2 as u32,
                public_key_ref: x.1.public_key().as_ref().to_vec(),
            };
            outputs.push(output);
            received_coins += x.2;
        }

        if sent_coins != received_coins {
            return None;
        }
        

        Some(Transaction {
            inputs,
            outputs,
            flag,
        })
                
    }
}








// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

//#[cfg(test)]
//mod tests {
//    use super::*;
//    use crate::types::key_pair;
//    use ring::signature::KeyPair;
//
//    #[test]
//    fn sign_verify() {
//        let t = generate_random_transaction();
//        let key = key_pair::random();
//        let signature = sign(&t, &key);
//        assert!(verify(&t, key.public_key().as_ref(), signature.as_ref()));
//    }
//    #[test]
//    fn sign_verify_two() {
//        let t = generate_random_transaction();
//        let key = key_pair::random();
//        let signature = sign(&t, &key);
//        let key_2 = key_pair::random();
//        let t_2 = generate_random_transaction();
//        assert!(!verify(&t_2, key.public_key().as_ref(), signature.as_ref()));
//        assert!(!verify(&t, key_2.public_key().as_ref(), signature.as_ref()));
//    }
//
//    #[test]
//    fn sign_after_verify() {
//        let key = key_pair::random();
//        let inputs_1: Vec<UtxoInput> = Vec::new();
//        let mut outputs_1: Vec<UtxoOutput> = Vec::new();
//        let out = UtxoOutput {
//            receiver_addr: String::new(),
//            value: 10,
//            public_key_ref: key.public_key().as_ref().to_vec(),
//        };
//        outputs_1.push(out);
//        let tx1 = Transaction {
//            inputs: inputs_1,
//            outputs: outputs_1,
//            flag: TxFlag::Normal,
//        };
//        
//        let tx1_sig = sign(&tx1, &key);
//        let mut inputs_2: Vec<UtxoInput> = Vec::new();
//        let outputs_2: Vec<UtxoOutput> = Vec::new();
//        let input = UtxoInput {
//            tx_hash: tx1.hash(),
//            value: 10,
//            index: 0,
//            sig_ref: tx1_sig.as_ref().to_vec(),
//        };
//        inputs_2.push(input);
//        
//        let tx_id_2 = gen_random_tx_id();
//        let tx2 = Transaction {
//            inputs: inputs_2,
//            outputs: outputs_2,
//            flag: TxFlag::Normal,
//        };
//
//        //Now we gonna verify these two transactions
//        for input in tx2.inputs.iter() {
//            let sig_vec = input.sig_ref.clone();
//            let index: usize = input.index as usize;
//
//            let outs: Vec<UtxoOutput> = tx1.outputs.clone();
//            let output: &UtxoOutput = outs.get(index).unwrap();
//            let pub_key = output.public_key_ref.clone();
//            assert!(verify(&tx1, pub_key.as_slice(), sig_vec.as_slice()));
//        }
//        
//
//        
//    }
//}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
