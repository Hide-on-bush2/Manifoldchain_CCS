use serde::Serialize;
use crate::{
    manifoldchain::{
        multichain::Multichain,
        miner::Handle as MinerHandle,
        network::{
            server::Handle as NetworkServerHandle,
            message::Message,
        },
        transaction::{
            generator::Handle as GeneratorHandle,
            Transaction,
            TxFlag,
        },
        mempool::Mempool,
        validator::{
            Validator,
            CrossUtxoStatus,
        },
        configuration::Configuration,
    },
    types::{
        hash::{
            H256,
            Hashable,
        }
    },
};

use log::{info};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
};
use tiny_http::{
    Header,
    Response,
    Server as HTTPServer,
};
use url::Url;

pub struct Server {
    handle: HTTPServer,
    miner: MinerHandle,
    network: NetworkServerHandle,
    multichain: Multichain,
    generator: GeneratorHandle,
    mempool: Arc<Mutex<Mempool>>,
    config: Configuration,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

macro_rules! respond_result {
    ( $req:expr, $success:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let payload = ApiResponse {
            success: $success,
            message: $message.to_string(),
        };
        let resp = Response::from_string(serde_json::to_string_pretty(&payload).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}
macro_rules! respond_json {
    ( $req:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let resp = Response::from_string(serde_json::to_string(&$message).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}

impl Server {
    pub fn start(
        addr: std::net::SocketAddr,
        miner: &MinerHandle,
        network: &NetworkServerHandle,
        multichain: &Multichain,
        generator: &GeneratorHandle,
        mempool: &Arc<Mutex<Mempool>>,
        config: &Configuration,
    ) {
        let handle = HTTPServer::http(&addr).unwrap();
        let server = Self {
            handle,
            miner: miner.clone(),
            network: network.clone(),
            multichain: multichain.clone(),
            generator: generator.clone(),
            mempool: Arc::clone(mempool),
            config: config.clone(),
        };
        thread::Builder::new()
            .name("api-server".to_string())
            .spawn(move || {
                for req in server.handle.incoming_requests() {
                    let miner = server.miner.clone();
                    let network = server.network.clone();
                    let multichain = server.multichain.clone();
                    let generator = server.generator.clone();
                    let mempool = Arc::clone(&server.mempool);
                    let config = server.config.clone();
                    let validator = Validator::new(
                        &multichain,
                        &mempool,
                        &config,
                    );
                    thread::spawn(move || {
                        // a valid url requires a base
                        let base_url = Url::parse(&format!("http://{}/", &addr)).unwrap();
                        let url = match base_url.join(req.url()) {
                            Ok(u) => u,
                            Err(e) => {
                                respond_result!(req, false, format!("error parsing url: {}", e));
                                return;
                            }
                        };
                        match url.path() {
                            "/miner/start" => {
                                let params = url.query_pairs();
                                let params: HashMap<_, _> = params.into_owned().collect();
                                let lambda = match params.get("lambda") {
                                    Some(v) => v,
                                    None => {
                                        respond_result!(req, false, "missing lambda");
                                        return;
                                    }
                                };
                                let lambda = match lambda.parse::<u64>() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        respond_result!(
                                            req,
                                            false,
                                            format!("error parsing lambda: {}", e)
                                        );
                                        return;
                                    }
                                };
                                miner.start(lambda);
                                respond_result!(req, true, "ok");
                            }
                            "/miner/end" => {
                                miner.exit();
                                respond_result!(req, true, "ok");
                            }
                            "/mempool/get_txs" => {
                                let txs = mempool.lock().unwrap().get_all_tx_hash();
                                let v_string: Vec<String> = txs.into_iter().map(|h| h.to_string()).collect();
                                respond_json!(req, v_string);
                            }
                            "/tx-generator/start" => {
                                let params = url.query_pairs();
                                let params: HashMap<_,_> = params.into_owned().collect();
                                let theta = match params.get("theta") {
                                    Some(v) => v,
                                    None => {
                                        respond_result!(req, false, "missing theta");
                                        return;
                                    }
                                };
                                let theta = match theta.parse::<u64>() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        respond_result!(
                                            req, 
                                            false,
                                            format!("error parsing theta: {}", e)
                                        );
                                        return;
                                    }
                                };
                                generator.start(theta);
                                respond_result!(req, true, "ok");
                            }
                            "/tx-generator/end" => {
                                generator.exit();
                                respond_result!(req, true, "ok");
                            }
                            "/network/ping" => {
                                network.broadcast(Message::Ping(String::from("Test ping")));
                                respond_result!(req, true, "ok");
                            }
                            "/blockchain/log" => {
                                multichain.log_to_file_with_shard(config.shard_id);
                                respond_result!(req, true, "ok");
                            }
                            "/blockchain/longest-chain" => {
                                let v = multichain.all_blocks_in_longest_chain();
                                let v_string: Vec<String> = v
                                    .into_iter()
                                    .map(|h| {
                                        let str = h.to_string();
                                        let left_slice = &str[0..3];
                                        let right_slice = &str[61..64];
                                        format!("{left_slice}..{right_slice}")
                                    })
                                    .collect();
                                respond_json!(req, v_string);
                            }
                            "/blockchain/longest-chain-with-time" => {
                                let v = multichain.all_blocks_in_longest_chain_with_time();
                                let mut v_string: Vec<String> = v
                                    .into_iter()
                                    .map(|h| {
                                        let hash_value = h.0;
                                        let time = h.1;
                                        
                                        let hash_str = hash_value.to_string();
                                        let left_slice = &hash_str[0..3];
                                        let right_slice = &hash_str[61..64];
                                        let block_des = format!("{left_slice}..{right_slice}");

                                        format!("{block_des}:{time}")
                                    }).collect();
                                let forking_rate = multichain.get_forking_rate();
                                v_string.push(format!("forking_rate: {:.2}", forking_rate));
                                respond_json!(req, v_string);
                            }
                            "/blockchain/longest-chain-with-shard" => {
                                let params = url.query_pairs();
                                let params: HashMap<_, _> = params.into_owned().collect();
                                let shard_id = match params.get("shard-id") {
                                    Some(v) => v,
                                    None => {
                                        respond_result!(req, false, "missing shard id");
                                        return;
                                    }
                                };
                                let shard_id = match shard_id.parse::<usize>() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        respond_result!(
                                            req, 
                                            false, 
                                            format!("error parsing shard id: {}", e)
                                        );
                                        return;
                                    }
                                };

                                let v = multichain.all_blocks_in_longest_chain_with_shard(shard_id);
                                let v_string: Vec<String> = v
                                    .into_iter()
                                    .map(|h| {
                                        let str = h.to_string();
                                        let left_slice = &str[0..3];
                                        let right_slice = &str[61..64];
                                        format!("{left_slice}..{right_slice}")
                                    })
                                    .collect();
                                respond_json!(req, v_string);
                            }
                            "/blockchain/longest-chain-tx" => {
                                let v = multichain.get_all_txs_in_longest_chain();
                                //let v_string: Vec<String> = v
                                //    .into_iter()
                                //    .map(|tx| tx.flag.to_string())
                                //    .collect();
                                let mut v_string: Vec<String> = vec![];
                                for tx in v {
                                    if let TxFlag::Empty = tx.flag {
                                        continue;
                                    }
                                    v_string.push(tx.flag.to_string());
                                }
                                respond_json!(req, v_string);
                            }
                            "/blockchain/longest-chain-tx-count" => {
                                // unimplemented!()
                                respond_result!(req, false, "unimplemented!");
                            }
                            "/blockchain/available-utxo" => {
                                let params = url.query_pairs();
                                let params: HashMap<_, _> = params.into_owned().collect();
                                let user = match params.get("user") {
                                    Some(v) => v,
                                    None => {
                                        info!("missing user");
                                        respond_result!(req, false, "missing user");
                                        return;
                                    }
                                };
                                let user = match user.parse::<String>() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        info!("error passing user");
                                        respond_result!(
                                            req,
                                            false,
                                            format!("error parsing user: {}", e)
                                        );
                                        return;
                                    }
                                };
                                let user: H256 = user.into();
                                let utxos = Self::get_available_utxo(
                                    &multichain,
                                    &validator,
                                    &config,
                                    &user,
                                );
                                respond_json!(req, utxos);
                            }
                            _ => {
                                info!("invalid HTTP request");
                                let content_type =
                                    "Content-Type: application/json".parse::<Header>().unwrap();
                                let payload = ApiResponse {
                                    success: false,
                                    message: "endpoint not found".to_string(),
                                };
                                let resp = Response::from_string(
                                    serde_json::to_string_pretty(&payload).unwrap(),
                                )
                                .with_header(content_type)
                                .with_status_code(404);
                                req.respond(resp).unwrap();
                            }
                        }
                    });
                }
            }).unwrap();
        info!("API server listening at {}", &addr);
    }

    fn get_available_utxo(
        multichain: &Multichain, 
        validator: &Validator, 
        config: &Configuration, 
        payer: &H256
    ) -> Vec<(Transaction, u32)> {
        if Validator::get_shard_id(
            payer,
            config.shard_num,
        ) != config.shard_id {
            return vec![];
        }

        let mut available_utxos: Vec<(Transaction, u32)> = Vec::new();
        let longest_verified_block = multichain.get_longest_chain_hash();
        let states = multichain.get_states();
        let state = states.get(&longest_verified_block).unwrap();
        for item in state.iter() {
            let tx_hash = &item.0.0;
            let tx_index = item.0.1 as usize;
            let tx = &item.1.0;
            let possbile_tmy = item.1.1.clone();
            if tx.outputs[tx_index].receiver_addr == *payer {
                match possbile_tmy {
                    Some(tmy) => {
                        let mut is_utxo_confirmed = true;
                        match &tx.flag {
                            &TxFlag::Output => {
                                for input in tx.inputs.iter() {
                                    let ori_shard_id = Validator::get_shard_id(
                                        &input.sender_addr,
                                        config.shard_num
                                    );
                                    match validator.validate_cross_utxo(
                                        tx,
                                        &input.hash(),
                                        &tmy,
                                        ori_shard_id,
                                        CrossUtxoStatus::Confirmed,
                                    ) {
                                        Ok(_) => {}
                                        Err(_) =>{
                                            is_utxo_confirmed = false;
                                            break;
                                        }
                                    }
                                }  
                            }
                            &TxFlag::Reject => {
                                 for output in tx.outputs.iter() {
                                    let ori_shard_id = Validator::get_shard_id(
                                        &output.receiver_addr,
                                        config.shard_num
                                    );
                                    match validator.validate_cross_utxo(
                                        tx,
                                        &output.hash(),
                                        &tmy,
                                        ori_shard_id,
                                        CrossUtxoStatus::Confirmed,
                                    ) {
                                        Ok(_) => {}
                                        Err(_) =>{
                                            is_utxo_confirmed = false;
                                            break;
                                        }
                                    }
                                }                               
                            }
                            _ => {}
                        }
                        if is_utxo_confirmed {
                            available_utxos.push((tx.clone(), tx_index as u32));
                        }
                        
                    }
                    None => {
                        available_utxos.push((tx.clone(), tx_index as u32));
                    }
                }
            }
        }
        available_utxos
    }
}
