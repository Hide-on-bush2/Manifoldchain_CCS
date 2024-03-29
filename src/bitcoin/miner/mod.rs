pub mod worker;

use log::info;
use crossbeam::channel::{
    unbounded, 
    Receiver, 
    Sender, 
    TryRecvError
};
use std::{
    time::{self, SystemTime}, 
    thread, 
    sync::{Arc, Mutex}, 
};
use crate::{        
    types::{
        merkle::MerkleTree, 
        hash::{H256, Hashable},
    }, 
    bitcoin::{
        blockchain::Blockchain,
        block::Block,
        transaction::{Transaction, Mempool},
    },
};
use rand::Rng;

enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Update, // update the block in mining, it may due to new blockchain tip or new transaction
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    finished_block_chan: Sender<Block>,
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mutex<Mempool>>,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(blockchain: &Arc<Mutex<Blockchain>>, mempool: &Arc<Mutex<Mempool>>) -> (Context, Handle, Receiver<Block>) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();
    let (finished_block_sender, finished_block_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        finished_block_chan: finished_block_sender,
        blockchain: Arc::clone(blockchain),
        mempool: Arc::clone(mempool),
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle, finished_block_receiver)
}

//#[cfg(any(test,test_utilities))]
//fn test_new() -> (Context, Handle, Receiver<Block>) {
//    let difficulty: H256 = (&[255u8; 32]).into(); 
//    let blockchain = Arc::new(Mutex::new(Blockchain::new(&difficulty)));
//    new(&blockchain)
//}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

    pub fn update(&self) {
        self.control_chan.send(ControlSignal::Update).unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn pow(&self, txs: Vec<Transaction>) -> Block {
        let parent: H256 = self.blockchain.lock().unwrap().tip();
        let timestamp: SystemTime = SystemTime::now();
        let difficulty: H256 = self.blockchain.lock().unwrap().get_block(&parent).unwrap().get_difficulty();
        // Scope to be modified begins
        let merkle_tree: MerkleTree<Transaction> = MerkleTree::new(txs.as_slice());
        //Scope to be modified ends
        let nonce: u32 = rand::thread_rng().gen();

        Block::construct(
            parent,
            timestamp,
            difficulty,
            merkle_tree,
            nonce
        )
    }

    fn miner_loop(&mut self) {
        // main mining loop
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Miner shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Miner starting in continuous mode with lambda {}", i);
                            self.operating_state = OperatingState::Run(i);
                        }
                        ControlSignal::Update => {
                            // in paused state, don't need to update
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
                                info!("Miner shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Miner starting in continuous mode with lambda {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Update => {
                                unimplemented!()
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            // TODO for student: actual mining, create a block
            // TODO for student: if block mining finished, you can have something like: self.finished_block_chan.send(block.clone()).expect("Send finished block error");

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }

                //do here
                if let Some(txs) = self.mempool.lock().unwrap().pop_txs() {
                    let block: Block = self.pow(txs);
                    if block.hash() <= block.get_difficulty() {
                        self.finished_block_chan.send(block.clone()).unwrap();
                        //leave the job of inserting new blocks to the workers
                        //self.blockchain.lock().unwrap().insert(&block);
                    }
                }
            }
        }
    }
}

//// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST
//
//#[cfg(test)]
//mod test {
//    use ntest::timeout;
//    use crate::types::hash::Hashable;
//
//    #[test]
//    #[timeout(60000)]
//    fn miner_three_block() {
//        let (miner_ctx, miner_handle, finished_block_chan) = super::test_new();
//        miner_ctx.start();
//        miner_handle.start(0);
//        let mut block_prev = finished_block_chan.recv().unwrap();
//        for _ in 0..2 {
//            let block_next = finished_block_chan.recv().unwrap();
//            assert_eq!(block_prev.hash(), block_next.get_parent());
//            block_prev = block_next;
//        }
//    }
//}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
