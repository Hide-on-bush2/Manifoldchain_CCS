#[cfg(test)]
#[macro_use]
extern crate hex_literal;

pub mod types;
pub mod bitcoin;
pub mod manifoldchain;
pub mod tests;

use crate::{
    bitcoin::{
        configuration::Configuration as BitcoinConfiguration,
        transaction::{
            Mempool as BitcoinMempool,
            generator::{
                self as BitcoinGenerator,
            },
        },
        network::{
            server as BitcoinNetworkServer,
            worker::Worker as BitcoinNetworkWorker,
        },
        api::Server as BitcoinApiServer,
        miner::{
            self as BitcoinMiner,
            worker::Worker as BitcoinMinerWorker,
        },
        blockchain::Blockchain as BitcoinBlockchain,
    },
    manifoldchain::{
        configuration::Configuration as ManifoldConfiguration,
        mempool::Mempool as ManifoldMempool,
        transaction::{
            generator::{
                self as ManifoldGenerator,
            },
        },
        network::{
            server as ManifoldNetworkServer,
            worker::Worker as ManifoldNetworkWorker,
        },
        api::Server as ManifoldApiServer,
        miner::{
            self as ManifoldMiner,
            worker::Worker as ManifoldMinerWorker,
        },
        blockchain::Blockchain as ManifoldBlockchain,
        multichain::Multichain,
        confirmation::Confirmation,
        verifier::{
            self as Verifier,
        }
    },
    types::hash::{H256},
};
use crossbeam::channel::{
    unbounded,
    Receiver,
    Sender,
    TryRecvError,
};
use clap::clap_app;
use smol::channel;
use log::{error, info, debug};
use std::{
    net, 
    process, 
    thread, 
    time, 
    sync::{Arc, Mutex},
    num::ParseIntError,
    convert::TryInto,
};
use env_logger::Env;

//fn run_bitcoin() {
//    // parse command line arguments
//    let matches = clap_app!(Bitcoin =>
//     (version: "0.1")
//     (about: "Bitcoin client")
//     (@arg verbose: 
//            -v ... 
//            "Increases the verbosity of logging")
//     (@arg peer_addr: 
//            --p2p [ADDR] 
//            default_value("127.0.0.1:6000") 
//            "Sets the IP address and the port of the P2P server")
//     (@arg api_addr: 
//            --api [ADDR] 
//            default_value("127.0.0.1:7000") 
//            "Sets the IP address and the port of the API server")
//     (@arg known_peer: 
//            -c --connect ... [PEER] 
//            "Sets the peers to connect to at start")
//     (@arg p2p_workers: 
//            --("p2p-workers") [INT] 
//            default_value("4") 
//            "Sets the number of worker threads for P2P server")
//    )
//    .get_matches();
//
//    // init logger
//    let verbosity = matches.occurrences_of("verbose") as usize;
//    stderrlog::new().verbosity(verbosity).init().unwrap();
//    let config = BitcoinConfiguration::new();
//    let blockchain = BitcoinBlockchain::new(&config.difficulty);
//    let blockchain = Arc::new(Mutex::new(blockchain));
//    let mempool = BitcoinMempool::new(config.block_size);
//    let mempool = Arc::new(Mutex::new(mempool));
//    // parse p2p server address
//    let p2p_addr = matches
//        .value_of("peer_addr")
//        .unwrap()
//        .parse::<net::SocketAddr>()
//        .unwrap_or_else(|e| {
//            error!("Error parsing P2P server address: {}", e);
//            process::exit(1);
//        });
//
//    // parse api server address
//    let api_addr = matches
//        .value_of("api_addr")
//        .unwrap()
//        .parse::<net::SocketAddr>()
//        .unwrap_or_else(|e| {
//            error!("Error parsing API server address: {}", e);
//            process::exit(1);
//        });
//
//    
//    // create channels between server and worker
//    let (msg_tx, msg_rx) = channel::bounded(10000);
//
//    // start the p2p server
//    let (server_ctx, server) = BitcoinNetworkServer::new(p2p_addr, msg_tx).unwrap();
//    server_ctx.start().unwrap();
//
//    // start the worker
//    let p2p_workers = matches
//        .value_of("p2p_workers")
//        .unwrap()
//        .parse::<usize>()
//        .unwrap_or_else(|e| {
//            error!("Error parsing P2P workers: {}", e);
//            process::exit(1);
//        });
//    let worker_ctx = BitcoinNetworkWorker::new(
//        p2p_workers,
//        msg_rx,
//        &server,
//        &blockchain,
//        &mempool,
//        &config,
//    );
//    worker_ctx.start();
//
//    // start the miner
//    let (miner_ctx, miner, finished_block_chan) = BitcoinMiner::new(&blockchain, &mempool);
//    let miner_worker_ctx = BitcoinMinerWorker::new(&server, finished_block_chan, &blockchain);
//    miner_ctx.start();
//    miner_worker_ctx.start();
//
//    // connect to known peers
//    if let Some(known_peers) = matches.values_of("known_peer") {
//        let known_peers: Vec<String> = known_peers.map(|x| x.to_owned()).collect();
//        let server = server.clone();
//        thread::spawn(move || {
//            for peer in known_peers {
//                loop {
//                    let addr = match peer.parse::<net::SocketAddr>() {
//                        Ok(x) => x,
//                        Err(e) => {
//                            error!("Error parsing peer address {}: {}", &peer, e);
//                            break;
//                        }
//                    };
//                    match server.connect(addr) {
//                        Ok(_) => {
//                            info!("Connected to outgoing peer {}", &addr);
//                            break;
//                        }
//                        Err(e) => {
//                            error!(
//                                "Error connecting to peer {}, retrying in one second: {}",
//                                addr, e
//                            );
//                            thread::sleep(time::Duration::from_millis(1000));
//                            continue;
//                        }
//                    }
//                }
//            }
//        });
//
//    }
//
//    //start the transaction generator
//    let (generator_ctx, generator) = BitcoinGenerator::new(&server, &blockchain, &config);
//    generator_ctx.start();
//
//    // start the API server
//    BitcoinApiServer::start(
//        api_addr,
//        &miner,
//        &server,
//        &blockchain,
//        &generator,
//        &mempool,
//    );
//
//    loop {
//        std::thread::park();
//    }
//}
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}
fn run_manifoldchain() {
    // parse command line arguments
    let matches = clap_app!(Manifoldchain =>
     (version: "0.1")
     (about: "Manifoldchain client")
     (@arg verbose:
            -v ... 
            "Increases the verbosity of logging")
     (@arg peer_addr: 
            --p2p [ADDR] 
            default_value("127.0.0.1:6000") 
            "Sets the IP address and the port of the P2P server")
     (@arg api_addr: 
            --api [ADDR] 
            default_value("127.0.0.1:7000") 
            "Sets the IP address and the port of the API server")
     (@arg known_peer: 
            -c --connect ... [PEER] 
            "Sets the peers to connect to at start")
     (@arg p2p_workers: 
            --("p2p-workers") [INT] 
            default_value("4") 
            "Sets the number of worker threads for P2P server")
    (@arg shard_id:
            --shardId [INT]
            "Sets the shard id of the node")
    (@arg node_id:
            --nodeId [INT]
            "Sets the id of the node")
    (@arg exper_number:
            --experNumber [INT]
            "Sets the number of experiment")
    (@arg shard_num:
            --shardNum [INT]
            "Sets the number of shards")
    (@arg shard_size:
            --shardSize [INT]
            "Sets the size of shards")
    (@arg block_size:
            --blockSize [INT]
            "Sets the size of block")
    (@arg confirmation_depth:
            --k [INT]
            "Sets the confirmation_depth")
    (@arg exclusive_diff:
            --eDiff [STR]
            "Sets the difficulty of mining a block")
    (@arg inclusive_diff:
            --iDiff [STR]
            "Sets the difficulty of mining an inclusive block")
    (@arg domestic_ratio:
            --domesticRatio [FLOAT]
            "The ratio of the domestic txs")
    )
    .get_matches();

    // init logger
    env_logger::from_env(Env::default().default_filter_or("info")).init();
    //let verbosity = matches.occurrences_of("verbose") as usize;
    //stderrlog::new().verbosity(verbosity).init().unwrap();

    // parse p2p server address
    let p2p_addr = matches
        .value_of("peer_addr")
        .unwrap()
        .parse::<net::SocketAddr>()
        .unwrap_or_else(|e| {
            error!("Error parsing P2P server address: {}", e);
            process::exit(1);
        });

    // parse api server address
    let api_addr = matches
        .value_of("api_addr")
        .unwrap()
        .parse::<net::SocketAddr>()
        .unwrap_or_else(|e| {
            error!("Error parsing API server address: {}", e);
            process::exit(1);
        });
    //parse the shard id
    let shard_id = matches
        .value_of("shard_id")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing the shard id: {}", e);
            process::exit(1);
        });
    //parse the shard id
    let node_id = matches
        .value_of("node_id")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing the node id: {}", e);
            process::exit(1);
        });
    //parse the shard id
    let exper_number = matches
        .value_of("exper_number")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing the experiment number: {}", e);
            process::exit(1);
        });
    let shard_num = matches
        .value_of("shard_num")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing the shard_num number: {}", e);
            process::exit(1);
        });
    let shard_size = matches
        .value_of("shard_size")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing the shard size: {}", e);
            process::exit(1);
        });
    let block_size = matches
        .value_of("block_size")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing the block size: {}", e);
            process::exit(1);
        });
    let confirmation_depth = matches
        .value_of("confirmation_depth")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing the confirmation depth: {}", e);
            process::exit(1);
        });
    let exclusive_diff = matches
        .value_of("exclusive_diff")
        .unwrap()
        .parse::<String>()
        .unwrap_or_else(|e| {
            error!("Error parsing the shard size: {}", e);
            process::exit(1);
        });
    let inclusive_diff = matches
        .value_of("inclusive_diff")
        .unwrap()
        .parse::<String>()
        .unwrap_or_else(|e| {
            error!("Error parsing the shard size: {}", e);
            process::exit(1);
        });
    let domestic_ratio = matches
        .value_of("domestic_ratio")
        .unwrap()
        .parse::<f64>()
        .unwrap_or_else(|e| {
            error!("Error parsing the domestic ratio: {}", e);
            process::exit(1);
        });
    let mut config = ManifoldConfiguration::new();
    let exclusive_diff_bytes: [u8; 32] = decode_hex(exclusive_diff.as_str())
        .unwrap()
        .try_into().unwrap();
    let inclusive_diff_bytes: [u8; 32] = decode_hex(inclusive_diff.as_str())
        .unwrap()
        .try_into().unwrap();
    let exclusive_diff_hash: H256 = exclusive_diff_bytes.into();
    let inclusive_diff_hash: H256 = inclusive_diff_bytes.into();
    config.difficulty = exclusive_diff_hash;
    config.thredshold = inclusive_diff_hash;
    config.shard_id = shard_id as usize;
    config.node_id = node_id as usize;
    config.exper_number = exper_number as usize;
    config.shard_num = shard_num as usize;
    config.shard_size = shard_size as usize;
    config.block_size = block_size as usize;
    config.k = confirmation_depth as usize;
    config.domestic_tx_ratio = domestic_ratio as f64;
    let shard_id = format!("{:x}", shard_id);
    info!("configuration: {:?}", config);

    let api_port: u16 = api_addr.port();
    let chains: Vec<Arc<Mutex<ManifoldBlockchain>>> = (0..config.shard_num)
        .into_iter()
        .map(|i| {
            let blockchain = ManifoldBlockchain::new(&config, i);
            Arc::new(Mutex::new(blockchain))
        })
        .collect();
    let chains_ref: Vec<&Arc<Mutex<ManifoldBlockchain>>> = chains
        .iter()
        .collect();
    let multichain = Multichain::create(chains_ref, &config);

    let mempool = ManifoldMempool::new();
    let mempool = Arc::new(Mutex::new(mempool));

    let confirmation = Confirmation::new(&multichain, &config);
    let confirmation = Arc::new(Mutex::new(confirmation));

    // create channels between server and worker
    let (msg_tx, msg_rx) = channel::bounded(10000);


    //create the channel for tx generators
    let (tx_generator_sender, tx_generator_receiver) = ManifoldGenerator::create_channel();

    let tx_generator_handle = ManifoldGenerator::new_handle(&tx_generator_sender);

    // start the p2p server
    let (server_ctx, server) = ManifoldNetworkServer::new(p2p_addr, msg_tx, &tx_generator_handle, config.shard_id).unwrap();
    server_ctx.start().unwrap();
    
    // start the worker
    let p2p_workers = matches
        .value_of("p2p_workers")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing P2P workers: {}", e);
            process::exit(1);
        });
    let worker_ctx = ManifoldNetworkWorker::new(
        p2p_workers,
        msg_rx,
        &server,
        &multichain,
        &mempool,
        &config,
        &confirmation,
    );
    worker_ctx.start();

    // start the miner
    let (miner_ctx, miner, finished_block_chan) = ManifoldMiner::new(&multichain, &mempool, &config);
    let miner_worker_ctx = ManifoldMinerWorker::new(
        &server, 
        finished_block_chan, 
        &multichain,
        &confirmation,
        &config,
    );
    miner_ctx.start();
    miner_worker_ctx.start();


    //start the sample monitor
    let verifier_ctx = Verifier::new(&multichain, &server, &config);
    verifier_ctx.start();

    
    // connect to known peers
    if let Some(known_peers) = matches.values_of("known_peer") {
        let known_peers: Vec<String> = known_peers.map(|x| x.to_owned()).collect();
        let server = server.clone();
        thread::spawn(move || {
            for peer in known_peers {
                loop {
                    let addr = match peer.parse::<net::SocketAddr>() {
                        Ok(x) => x,
                        Err(e) => {
                            error!("Error parsing peer address {}: {}", &peer, e);
                            break;
                        }
                    };
                    match server.connect(addr) {
                        Ok(_) => {
                            info!("Connected to outgoing peer {}", &addr);
                            break;
                        }
                        Err(e) => {
                            error!(
                                "Error connecting to peer {}, retrying in one second: {}",
                                addr, e
                            );
                            thread::sleep(time::Duration::from_millis(1000));
                            continue;
                        }
                    }
                }
            }
        });

    }

    //start the transaction generator
    let generator_ctx = ManifoldGenerator::new_ctx(
        &tx_generator_receiver, 
        &server,
        &mempool, 
        &config, 
        api_port
    );
    generator_ctx.start();

    // start the API server
    ManifoldApiServer::start(
        api_addr,
        &miner,
        &server,
        &multichain,
        &tx_generator_handle,
        &mempool,
        &config,
    );

    loop {
        std::thread::park();
    }
}
fn main() {
    //run_bitcoin();
    run_manifoldchain();
}
