use std::{
    fs::{File, self},
    io::{Write, Error},
    env,
    num::ParseIntError,
};
use serde::{Serialize, Deserialize};

const EXPER_NUMBER: usize = 28;
const SHARD_NUM: usize = 5;
const SHARD_SIZE: usize = 5;
const PROPAGATION_DELAY: usize = 100;//ms
const INCLUSIVE_DIFF: usize = 15;

#[test]
fn test_decode() {
    let diff = String::from("00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    let res = decode_hex(diff.as_str()).unwrap();
    println!("{:?}", res);
}
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}


#[test]
fn setup() {
    let args: Vec<String> = env::args().collect();
    let exper_number: usize = args[2].parse().unwrap();
    let config_data = read_config(exper_number);
    generate_exper_bash(exper_number, &config_data).unwrap();
    generate_start_bash(exper_number, &config_data);
    generate_start_nodes_bash(exper_number, &config_data);
    generate_end_bash(exper_number, &config_data);
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigData {
    shard_num: usize, //how many shards totally
    shard_size: usize, //how many nodes in each shard
    block_size: usize, //the number of txs in each block
    confirmation_depth: usize, //the confirmation depth, k
    mining_interval: usize, //the interval(ms) between two mining operations
    tx_generation_interval: usize, //the interval(ms) between two tx generations
    runtime: usize, //how long the experiment will run (10 s)
    domestic_ratio: f64,
    iteration: usize, //the iteration of this experiment
    inclusive_diff: String, //inclusive difficulty (all shard shares the same inclusive diff)
    exclusive_diffs: Vec<String>, //exclusive difficulties across all shards
    propagation_delay: usize, //the propagation delay shared by all communications
    bandwidths: Vec<Vec<usize>>, //the bandwidths of all nodes, outer-shards inter-nodes
    description: String, //the README of this experiment
}


fn read_config(exper_number: usize) -> ConfigData {
    let path = format!("./scripts/expers/exper_{}/config.json", exper_number);
    let config_content = fs::read_to_string(path).expect("Couldn't find the file");
    let config_data: ConfigData = serde_json::from_str(&config_content).unwrap();
    config_data
}

fn generate_exper_bash(exper_number: usize, config: &ConfigData) -> Result<(), Error> {
    let basic_path = format!("./scripts/expers/exper_{}/", exper_number);
    let nodes_path = format!("{}nodes/", basic_path.clone());
    //create a dir to store the startup scripts for nodes
    fs::create_dir_all(nodes_path.clone()).unwrap_or_else(|why| {
        println!("! {:?}", why.kind());
    });
    let log_path = format!("./log/exper_{}/iter_{}/exec_log/", exper_number, config.iteration);
    //create a dir to store the directory for logs
    fs::create_dir_all(log_path).unwrap_or_else(|why| {
        println!("! {:?}", why.kind());
    });
    let back_to_root = String::from("#!/bin/bash\ncd ../../../\n");
    for shard_id in 0..config.shard_num {
        let exclusive_diff = config.exclusive_diffs[shard_id].clone();
        for node_index in 0..config.shard_size {
            let mut add_node_delay: Vec<String> = vec![];
            for inter_node_index in 0..node_index {
                add_node_delay.push(format!(
                    "sudo ./scripts/network_simulation/start_network_node.sh delay {} {} {} {} {}\n",
                    shard_id+1, node_index+1,
                    shard_id+1, inter_node_index+1,
                    config.propagation_delay, 
                ));
            }
            for past_shard in 0..shard_id {
                add_node_delay.push(format!(
                    "sudo ./scripts/network_simulation/start_network_node.sh delay {} {} {} {} {}\n",
                    shard_id+1, node_index+1,
                    past_shard+1, 1,
                    config.propagation_delay,
                ));
                add_node_delay.push(format!(
                    "sudo ./scripts/network_simulation/start_network_node.sh delay {} {} {} {} {}\n",
                    shard_id+1, node_index+1,
                    past_shard+1, 2,
                    config.propagation_delay,
                ));
            }
            let node_id = shard_id * config.shard_size + node_index;
            let basic_cmd = format!(
                "sudo ip netns exec ramjet-s{}-n{} ./target/debug/bitcoin --p2p 10.0.{}.{}:6000 --api 10.0.{}.{}:7000", 
                shard_id+1, node_index+1,
                shard_id+1, (node_index+1)*2,
                shard_id+1, (node_index+1)*2,
            );
            let mut connect_nodes: Vec<String> = vec![];
            for inter_node_index in 0..node_index {
                connect_nodes.push(format!(
                    "-c 10.0.{}.{}:6000",
                    shard_id+1, (inter_node_index+1)*2,
                ));
            }
            for past_shard in 0..shard_id {
                connect_nodes.push(format!(
                    "-c 10.0.{}.2:6000", past_shard+1
                ));
                connect_nodes.push(format!(
                    "-c 10.0.{}.4:6000", past_shard+1
                ));
            }
            let shard_id_cmd = format!("--shardId {}", shard_id);
            let node_id_cmd = format!("--nodeId {}", node_id);
            let exper_number_cmd = format!("--experNumber {}", exper_number);
            let shard_num_cmd = format!("--shardNum {}", config.shard_num);
            let shard_size_cmd = format!("--shardSize {}", config.shard_size);
            let block_size_cmd = format!("--blockSize {}", config.block_size);
            let confirmation_depth_cmd = format!("--k {}", config.confirmation_depth);
            let domestic_ratio_cmd = format!("--domesticRatio {}", config.domestic_ratio);
            let total_diff_cmd = format!("--eDiff {}", exclusive_diff);
            let inclusive_diff_cmd = format!("--iDiff {}", config.inclusive_diff);
            let mut final_cmd: String = back_to_root.clone();
            for delay in add_node_delay {
                final_cmd = format!("{}{}", final_cmd, delay);
            }
            final_cmd = format!("{}{}", final_cmd, basic_cmd);
            for connect_cmd in connect_nodes {
                final_cmd = format!("{} {}", final_cmd, connect_cmd);
            }
            final_cmd = format!("{} {}", final_cmd, shard_id_cmd);
            final_cmd = format!("{} {}", final_cmd, node_id_cmd);
            final_cmd = format!("{} {}", final_cmd, exper_number_cmd);
            final_cmd = format!("{} {}", final_cmd, shard_num_cmd);
            final_cmd = format!("{} {}", final_cmd, shard_size_cmd);
            final_cmd = format!("{} {}", final_cmd, block_size_cmd);
            final_cmd = format!("{} {}", final_cmd, confirmation_depth_cmd);
            final_cmd = format!("{} {}", final_cmd, domestic_ratio_cmd);
            final_cmd = format!("{} {}", final_cmd, total_diff_cmd);
            final_cmd = format!("{} {}", final_cmd, inclusive_diff_cmd);
            let path = format!("{}start_node_{}.sh", nodes_path.clone(), node_id);
            let mut output = File::create(path)?;
            write!(output, "{}", final_cmd)?;
        }
    }
    Ok(())
}

fn generate_start_bash(exper_number: usize, config: &ConfigData) {
    let content = format!(
"#!/bin/bash
shard_num={}
shard_size={}
mining_interval={}
tx_generation_interval={}
runtime={}
iter={}
exper_number={}
sudo rm -r ../../../DB/*
./start_nodes.sh
sleep 120
cd ../../virtual_network_operation/
for ((k=0; k<$shard_num; k++))
do
  for ((h=0; h<$shard_size; h++))
  do
    ./start_miner.sh $k $h $mining_interval 
    ./start_tx_generator.sh $k $h $tx_generation_interval 
  done
done
c=0
while [ $c -lt $runtime ]; do
  sleep 10
  c=$[$c+1]
  echo \"$c\"
  #log_count=$(( $c % 200 ))
  #if [ $log_count = 0 ]; then
      #for ((k=0; k<$shard_num; k++))
      #do
	      #for ((h=0; h<$shard_size; h++))
	      #do
		      #./ask_to_log.sh $k $h &
	      #done
      #done
  #fi
done
./end_node.sh $shard_num $shard_size
sleep 5
cd ../expers/exper_$exper_number/
mv ../../../log/exper_$exper_number/*.txt ../../../log/exper_$exper_number/iter_$iter/
mv ../../../log/exper_$exper_number/*.log ../../../log/exper_$exper_number/iter_$iter/exec_log/
cp ./config.json ../../../log/exper_$exper_number/iter_$iter/
sleep 10", 
        config.shard_num, 
        config.shard_size, 
        config.mining_interval, 
        config.tx_generation_interval,
        config.runtime,
        config.iteration,
        exper_number,
    );
    let basic_path = format!("./scripts/expers/exper_{}/", exper_number);
    let path = format!("{}start.sh", basic_path);
    let mut output = File::create(path).unwrap();
    write!(output, "{}", content).unwrap();
}

fn generate_start_nodes_bash(exper_number: usize, config: &ConfigData) {
    let mut network_node_cmds: Vec<String> = vec![];
    for i in 0..config.shard_num {
        for j in 0..config.shard_size {
            let sid = i + 1;
            let nid = j + 1;
            let bandwidth = config.bandwidths[i][j];
            let cmd = format!("sudo ../../network_simulation/start_network_node.sh add {} {} {}\n", 
                sid, nid, bandwidth
            );
            network_node_cmds.push(cmd);
        }
    }
    let mut cmd = String::from("#!/bin/bash\n");
    for network_cmd in network_node_cmds {
        cmd = format!("{}{}", cmd, network_cmd);
    }
    let start_node_cmd = format!(
"for ((i=0; i<{}; i++))
do
  for ((j=0; j<{}; j++))
  do
    node_id=$[i*{}+j]
    ./nodes/start_node_$node_id.sh 2>&1 | tee ../../../log/exper_{}/$node_id.log &
  done
done",
        config.shard_num,
        config.shard_size,
        config.shard_size,
        exper_number,
    );
    cmd = format!("{}{}", cmd, start_node_cmd);
    let basic_path = format!("./scripts/expers/exper_{}/", exper_number);
    let path = format!("{}start_nodes.sh", basic_path);
    let mut output = File::create(path).unwrap();
    write!(output, "{}", cmd).unwrap();
}

fn generate_end_bash(exper_number: usize, config: &ConfigData) {
    let content = format!(
"#!/bin/bash

shard_num={}
shard_size={}
exper_number={}
iter={}

cd ../../virtual_network_operation/
./end_node.sh $shard_num $shard_size
sleep 5
cd ../expers/exper_$exper_number/
sudo mv ../../../log/exper_$exper_number/*.txt ../../../log/exper_$exper_number/iter_$iter/
sudo mv ../../../log/exper_$exper_number/*.log ../../../log/exper_$exper_number/iter_$iter/exec_log/
sudo cp ./config.json ../../../log/exper_$exper_number/iter_$iter/
sleep 10",
        config.shard_num, 
        config.shard_size,
        exper_number,
        config.iteration,
    );
    let basic_path = format!("./scripts/expers/exper_{}/", exper_number);
    let path = format!("{}end.sh", basic_path);
    let mut output = File::create(path).unwrap();
    write!(output, "{}", content).unwrap();
}

