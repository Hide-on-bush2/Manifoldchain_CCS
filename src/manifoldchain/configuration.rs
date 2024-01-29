use crate::types::hash::H256;


#[derive(Debug, Default, Clone)]
pub struct Configuration {
    pub difficulty: H256,
    pub thredshold: H256,
    pub block_size: usize,
    pub k: usize,
    pub initial_balance: usize,
    pub initial_utxo_num: usize,
    pub user_size: usize,
    pub num_tx_recv: usize, //the number of receivers of a transaction when generating txs
    pub shard_id: usize,
    pub node_id: usize,
    pub max_shard_num: usize,
    pub shard_num: usize,
    pub shard_size: usize,
    pub tx_merkle_proof_len: usize,
    pub network_delay: usize,
    pub exper_number: usize,
    pub domestic_tx_ratio: f64,
}

impl Configuration {
    pub fn new() -> Self {
        //let mut difficulty_vec = [255u8; 32];
//      //  let difficulty_zero_number = 1;
//      //  for i in 0..difficulty_zero_number {
//      //      difficulty_vec[i] = total_diff as u8;
//      //  }
        //let mut thredshold_vec = [255u8; 32];
//      //  let thredshold_zero_number = 1;
//      //  for i in 0..thredshold_zero_number {
//      //      thredshold_vec[i] = inclusive_diff as u8;
//      //  }
        //difficulty_vec[0] = 0;
        //difficulty_vec[1] = total_diff as u8;
        //thredshold_vec[0] = 0;
        //thredshold_vec[1] = inclusive_diff as u8;
        Configuration {
            difficulty: H256::default(),
            thredshold: H256::default(), //spliting exclusive block and inclusive block
            block_size: 2048,
            k: 6,
            initial_balance: 1000,
            initial_utxo_num: 3,
            user_size: 3,
            num_tx_recv: 3,
            shard_id: 0,
            node_id: 0,
            max_shard_num: 256,
            shard_num: 0,
            shard_size: 0,
            tx_merkle_proof_len: 1,
            network_delay: 0,
            exper_number: 0,
            domestic_tx_ratio: 0.7,
        }
    }
}
