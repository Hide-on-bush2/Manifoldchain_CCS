use serde::{Serialize, Deserialize};

use crate::{
        types::{
        hash::H256, 
    },
    manifoldchain::{
        block::{
            Block,
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
            versa_block::{
                VersaBlock,
                VersaHash,
                ExclusiveFullBlock,
                InclusiveFullBlock,
            }
        },
        transaction::Transaction,
        testimony::Testimony,
        fraudproof::FraudProof,
        network::worker::{SampleIndex, Sample},
    }
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Ping(String),
    Pong(String),
    ////Typical blocks
    //NewBlockHash(Vec<(VersaHash, u32)>),
    //GetBlocks(Vec<(VersaHash, u32)>),
    //Blocks(Vec<VersaBlock>),
    //Transactions
    NewTransactionHash((Vec<H256>, u32)),
    GetTransactions((Vec<H256>, u32)),
    Transactions((Vec<Transaction>, u32)),
    //Testimony
    NewTestimonyHash((Vec<H256>, u32)),
    GetTestimonies((Vec<H256>, u32)),
    Testimonies((Vec<Testimony>, u32)),
    //Exclusive Block
    NewExBlockHash((Vec<H256>, u32)),
    GetExBlocks((Vec<H256>, u32)),
    ExBlocks((Vec<ExclusiveBlock>, u32)),
    //Inclusive Block
    NewInBlockHash((Vec<H256>, u32)),
    GetInBlocks((Vec<H256>, u32)),
    InBlocks((Vec<InclusiveBlock>, u32)),
    //Exclusive Block
    NewExFullBlockHash((Vec<H256>, u32)),
    GetExFullBlocks((Vec<H256>, u32)),
    ExFullBlocks((Vec<ExclusiveFullBlock>, u32)),
    //Exclusive Block
    NewInFullBlockHash((Vec<H256>, u32)),
    GetInFullBlocks((Vec<H256>, u32)),
    InFullBlocks((Vec<InclusiveFullBlock>, u32)),
    //FraudProof
    NewFraudProofHash(Vec<H256>),
    GetFraudProofs(Vec<H256>),
    FraudProofs(Vec<FraudProof>),
    //Data Availability Sample
    NewSamples(Vec<SampleIndex>),
    GetSamples(Vec<SampleIndex>), //(block_hash, tx_index)
    Samples(Vec<(SampleIndex, Vec<Sample>)>), 
    //key: block_hash, tx_index, value: (sample_index, sample) 
    //missing block
    NewMissBlockHash((Vec<H256>, u32)),
}
