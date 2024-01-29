use log::{info, debug};
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
    collections::HashMap,
};
use crate::{        
    manifoldchain::{
        multichain::Multichain,
        configuration::Configuration,
        network::{
            server::Handle as ServerHandle,
            message::Message,
            worker::{SampleIndex},
        }
    },
};
use rand::Rng;


pub struct Context {
    multichain: Multichain,
    config: Configuration,
    server: ServerHandle,
}


pub fn new(multichain: &Multichain,
    server: &ServerHandle,
    config: &Configuration) -> Context 
{
    Context {
        multichain: multichain.clone(),
        server: server.clone(),
        config: config.clone(),
    }
}



impl Context {
    //need to polish here
    pub fn start(mut self) {
       thread::Builder::new()
            .name("Sample-Verifier".to_string())
            .spawn(move || {
                self.monitor_sample();
            })
            .unwrap();
        info!("Sample monitor started");
    }
    fn monitor_sample(&mut self) {
        loop {
            //check if there are any unverified blocks, if yes, request the samples
            let unverified_blocks = self.multichain.get_unverified_blocks();
            if !unverified_blocks.is_empty() {
                //self.finished_block_chan
                //    .send(MinerMessage::GetSamples(unverified_blocks))
                //    .unwrap();
                let mut rq_samples: Vec<SampleIndex> = vec![];
                info!("Miner worker get {} samples", unverified_blocks.len());
                for (blk_hash, shard_id) in unverified_blocks {
                    let mut rng = rand::thread_rng();
                    let tx_index: usize = rng.gen_range(0..self.config.block_size);
                    rq_samples.push((blk_hash, tx_index as u32, shard_id as u32)); 
                }    
                self.server.broadcast(Message::GetSamples(rq_samples));
            } else {
                //info!("no unverified blocks");
            }
            let interval = time::Duration::from_micros(30000000);
            thread::sleep(interval);
        }
    }
}



