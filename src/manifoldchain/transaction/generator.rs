use crate::{
    types::{
        key_pair,
        hash::{Hashable, H256}
    },
    manifoldchain::{
        network::{
            message::Message,
            server::Handle as ServerHandle,
        },
        transaction::{
            Transaction,
            TxFlag,
            UtxoInput,
            UtxoOutput,
        },
        configuration::Configuration,
        validator::Validator,
        mempool::Mempool,
    },
};
use crossbeam::channel::{
    unbounded,
    Receiver,
    Sender,
    TryRecvError,
};
use std::{
    time,
    thread,
    collections::{
        VecDeque,
        HashMap,
    },
    sync::{Arc, Mutex},
};
use rand::{self, Rng};
use log::{info, debug};
use ring::signature::{Ed25519KeyPair, KeyPair};
use reqwest;

pub enum ControlSignal {
    Start(u64),
    Stop,
    Exit,
    NewNode(String),
}

enum OperatingState {
    Run(u64),
    Pause, 
    ShutDown,
}

pub struct Context {
    //channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    server: ServerHandle,
    users: HashMap<usize, H256>, //shard_id->user
    keys: HashMap<H256, Ed25519KeyPair>,
    config: Configuration,
    nodes: HashMap<String, bool>,
    used_utxo: HashMap<(H256, u32), bool>, //(tx_hash, index) -> bool,
    mempool: Arc<Mutex<Mempool>>,
    initial_bonus: usize,
    api_port: u16, 
}

#[derive(Clone)]
pub struct Handle {
    //channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn create_channel() -> (Sender<ControlSignal>, Receiver<ControlSignal>) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();
    (signal_chan_sender, signal_chan_receiver)
}

pub fn new_ctx(
    chan_receiver: &Receiver<ControlSignal>, 
    server: &ServerHandle, 
    mempool: &Arc<Mutex<Mempool>>, 
    config: &Configuration, 
    api_port: u16
) -> Context {
    
    let mut rng = rand::thread_rng();    
    let mut users: HashMap<usize, H256> = HashMap::new();
    let mut keys: HashMap<H256, Ed25519KeyPair> = HashMap::new();
    
    for i in 0..config.shard_num {
        let mut random_bytes: [u8; 32] = rng.gen();
        let mut j = 31;
        let mut x: usize = i;
        loop {
            random_bytes[j] = (x % 256) as u8;
            x = x / 256;
            j -= 1;
            if x <= 0 || j < 0 {
                break;
            }
        }
        let user_hash: H256 = (&random_bytes).into();
        let key = key_pair::random();
        users.insert(i, user_hash.clone());
        keys.insert(user_hash, key);
    }

    

    //for simplification, this scope is written in a hard-code way
    let nodes: HashMap<String, bool> = HashMap::new();
    //for i in 0..config.shard_num {
    //    for j in 0..config.shard_size {
    //        nodes.insert(i*config.shard_size + j, format!("127.0.0.1:70{}{}", i, j));
    //    }
    //}

    let ctx = Context{
        server: server.clone(),
        control_chan: chan_receiver.clone(),
        operating_state: OperatingState::Pause,
        users,
        keys,
        config: config.clone(),
        nodes,
        used_utxo: HashMap::new(),
        mempool: Arc::clone(mempool),
        api_port,
        initial_bonus: 1,
    };

    ctx
}

pub fn new_handle(chan_sender: &Sender<ControlSignal>) -> Handle {
    Handle {
        control_chan: chan_sender.clone(),
    }
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, theta: u64) {
        self.control_chan
            .send(ControlSignal::Start(theta))
            .unwrap();
    }
    pub fn stop(&self) {
        self.control_chan.send(ControlSignal::Stop).unwrap();
    }
    pub fn new_node(&self, node: String) {
        self.control_chan.send(ControlSignal::NewNode(node)).unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("tx generator".to_string())
            .spawn(move ||{
                self.generator_loop();
            }).unwrap();
        info!("Generator initialized into pause mode");
    }

    fn generator_loop(&mut self) {
        let mut is_initial = true;
        let mut no_utxo_count = 0;
        //main generating loop
        loop {
            //check and react to control signals
            match self.operating_state {
                OperatingState::Pause => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Generator shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Generator starting in continuous mode with theta {}", i);
                            self.operating_state = OperatingState::Run(i);
                        }
                        ControlSignal::Stop => {}
                        ControlSignal::NewNode(node) => {
                            info!("Tx generator add new node {}", node);
                            self.nodes.insert(node, true);
                        }
                    };
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        match signal {
                            ControlSignal::Exit => {
                                info!("Generator shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Generator starting in continuous mode with theta {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Stop => {
                                self.operating_state = OperatingState::Pause;
                            }
                            ControlSignal::NewNode(node) => {
                                info!("Tx generator add new node {}", node);
                                self.nodes.insert(node, true);
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Generator control channel detached"),
                }
            };

            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }

                if is_initial {
                    //generate the initial balance for each user
                    for i in 0..self.config.initial_utxo_num {
                        for item in self.keys.iter() {
                            let initial_tx = Transaction::create_initial_tx(
                                (&item.0, &item.1),
                                self.config.initial_balance + self.initial_bonus,
                            );
                            self.initial_bonus += 1;
                            let shard_id = Validator::get_shard_id(
                                &item.0,
                                self.config.shard_num,
                            );
                            self.server.broadcast_with_shard(Message::Transactions((
                                vec![initial_tx.clone()], 
                                shard_id as u32
                            )), shard_id);
                            //if shard_id == self.config.shard_id {
                            //    self.mempool.lock().unwrap().insert_tx(initial_tx);
                            //}
                        }
                    }
                    is_initial = false;
                    continue;
                }


                //generating transactions
                let mut rng = rand::thread_rng();
                let num_node = self.users.len();
                let payer = self.users.get(&self.config.shard_id).unwrap().clone();
                let domestic_ratio: f64 = self.config.domestic_tx_ratio;
                let sample_range: usize = 10000;
                let sample_value: usize = rng.gen_range(0..sample_range);
                let threshold: f64 = (sample_range as f64) * domestic_ratio;
                let sample_value = sample_value as f64;
                
                let mut receivers: Vec<H256> = vec![];
                if sample_value <= threshold {
                    //create domestic tx
                    receivers.push(self.users.get(&self.config.shard_id).unwrap().clone());
                } else {
                    //create cross tx
                    let mut count = 0;
                    while count < self.config.num_tx_recv {
                        let recv_index: usize = rng.gen_range(0..self.config.shard_num);
                        if recv_index == self.config.shard_id {
                            continue;
                        }
                        receivers.push(self.users.get(&recv_index).unwrap().clone());
                        count += 1;
                    }
                }
                
                let coins: Vec<usize> = (0..receivers.len()).map(|_| 1).collect();
                if let Some(tx) = self.create_tx(payer.clone(), receivers.clone(), coins) {
                    //classify the users by shards
                    info!("create non-initial tx");
                    let mut input_shards: HashMap::<usize, bool> = HashMap::new();
                    let mut output_shards: HashMap::<usize, bool> = HashMap::new();
                    let payer_shard_id = Validator::get_shard_id(&payer, self.config.shard_num);
                    input_shards.insert(
                        Validator::get_shard_id(&payer, self.config.shard_num),
                        true,
                    );
                    for receiver in receivers.iter() {
                        output_shards.insert(
                            Validator::get_shard_id(receiver, self.config.shard_num),
                            true,
                        );
                    }
                    //identify whether it is a cross-tx
                    let mut is_cross_tx = false;
                    for item in output_shards.iter() {
                        let shard_id = &item.0;
                        if let None = input_shards.get(shard_id) {
                            is_cross_tx = true;
                            break;
                        }
                    }
                    if is_cross_tx {
                        let mut input_tx = tx.clone();
                        input_tx.flag = TxFlag::Input;
                        for (key, _) in input_shards {
                            self.server.broadcast_with_shard(Message::Transactions((
                                vec![input_tx.clone()],
                                key as u32,
                            )), key);
                            if key == self.config.shard_id {
                                self.mempool.lock().unwrap().insert_tx(input_tx.clone());
                            }
                        }
                        //let mut output_tx = tx.clone();
                        //output_tx.flag = TxFlag::Output;
                        //for (key, _) in output_shards {
                        //    self.server.broadcast_with_shard(Message::Transactions((
                        //        vec![output_tx.clone()],
                        //        key as u32,
                        //    )), key);
                        //    if key == self.config.shard_id {
                        //        self.mempool.lock().unwrap().insert_tx(output_tx.clone());
                        //    }
                        //}
                    } else {
                        self.server.broadcast_with_shard(Message::Transactions((
                            vec![tx.clone()],
                            payer_shard_id as u32,
                        )), payer_shard_id);
                        if payer_shard_id == self.config.shard_id {
                            self.mempool.lock().unwrap().insert_tx(tx.clone());
                        }
                    }
                    no_utxo_count = 0;
                    //self.server.broadcast(Message::Transactions(txs));
                } else {
                    info!("create another new initial tx");
                    let initial_tx = Transaction::create_initial_tx(
                        (&payer, self.keys.get(&payer).unwrap()),
                        self.config.initial_balance + self.initial_bonus,
                    );
                    self.initial_bonus += 1;
                    let payer_shard_id = Validator::get_shard_id(&payer, self.config.shard_num);
                    self.server.broadcast_with_shard(Message::Transactions((
                        vec![initial_tx], 
                        payer_shard_id as u32
                    )), payer_shard_id);
                    
                }
            }
        }
    }

    fn create_tx(&mut self, 
        payer: H256, 
        receivers: Vec<H256>, 
        coins: Vec<usize>) -> Option<Transaction> {
        if receivers.len() != coins.len() {
            //debug!("the size of receives and coins are not equal");
            return None;
        }

        if let Ok(available_utxos_vec) = self.get_utxo_by_api(&payer) {   
            let mut available_utxos: VecDeque<(Transaction, u32)> = VecDeque::from(available_utxos_vec);
            //let blockchain_states = self.blockchain.lock().unwrap().states.clone();
            //let last_blk_hash = self.blockchain.lock().unwrap().longest_chain_hash.clone();
            //let lastest_state = blockchain_states.get(&last_blk_hash).unwrap();
            //for (key, tx) in lastest_state.iter() {
            //    let utxo_index = key.1;
            //    if tx.outputs.get(utxo_index as usize).unwrap().receiver_addr == payer {
            //        available_utxos.push_back((tx.clone(), utxo_index));
            //    }
            //}

            let require_coins: usize = coins.iter().sum();

            let mut available_coins: Vec<(Transaction, u32)> = Vec::new();
            //info!("total {} available_utxos for payer {:?}", available_utxos.len(), payer);
            //for item in available_utxos.iter() {
            //    let value = item.0.outputs[item.1 as usize].value;
            //    debug!("coins: {}", value);
            //}
            let mut curr_coins: usize = 0;
            loop {
                if let Some(utxo) = available_utxos.pop_front() {
                    let tx = &utxo.0;
                    let index = utxo.1 as usize;

                    if let Some(_) = self.used_utxo.get(&(tx.hash(), utxo.1)) {
                        continue;
                    }

                    if let TxFlag::Reject = tx.flag {
                        curr_coins += tx.inputs[index].value as usize;
                    } else {
                        curr_coins += tx.outputs[index].value as usize;
                    }

                    available_coins.push(utxo);
                    if curr_coins >= require_coins {
                        break;
                    } 
                } else {
                    break;
                }
            }
             
            if curr_coins < require_coins {
                info!("coins not enough");
                return None;
            }

            for utxo in available_coins.iter() {
                self.used_utxo.insert((utxo.0.hash(), utxo.1), true);
            }

            
            let available_utxo_hashs: Vec<(H256, u32)> = available_utxos
                .iter()
                .map(|x| (x.0.hash(), x.1) )
                .collect();
            let delete_used_utxo: HashMap<(H256, u32), bool> = self.used_utxo
                .clone()
                .into_iter()
                .filter(|(key, _)| !available_utxo_hashs.contains(&key))
                .collect();
             //not all comming state is related to the same user
            //for (key, _) in delete_used_utxo.iter() {
            //    self.used_utxo.remove(key);
            //}


            let utxos: Vec<(&Transaction, usize)> = available_coins
                .iter()
                .map(|x| (&x.0, x.1 as usize))
                .collect();
            let senders: Vec<(&H256, &Ed25519KeyPair)> = vec![(&payer, self.keys.get(&payer).unwrap()); utxos.len()];
            let mut receivers_coins: Vec<(&H256, &Ed25519KeyPair, usize)> = vec![];
            for i in 0..receivers.len() {
                receivers_coins.push((
                    &receivers[i],
                    self.keys.get(&receivers[i]).unwrap(),
                    coins[i],
                ));
            };
            if curr_coins > require_coins {
                let left_coins = curr_coins - require_coins;
                receivers_coins.push((
                    &payer,
                    self.keys.get(&payer).unwrap(),
                    left_coins,
                ));
            }


            let tx = Transaction::consume(
                utxos,
                senders,
                receivers_coins,
                TxFlag::Domestic,
            ).unwrap();

            Some(tx)
        } else {
            info!("none availabile utxo");
            None
        }
        
    }

    fn get_utxo_by_api(&self, user: &H256) 
        -> Result<Vec<(Transaction, u32)>, Box<dyn std::error::Error>> 
    {
         
        let mut utxos: Vec<(Transaction, u32)> = Vec::new();
        for (val, _) in self.nodes.iter() {
            let req_url = format!("http://{}:{}/blockchain/available-utxo?user={}", val, self.api_port, user);
            let resp = reqwest::blocking::get(req_url)?
                .json::<Vec<(Transaction, u32)>>()?;
            utxos.extend(resp);
        }
        
        Ok(utxos)

    }
}


