#!/bin/bash
cd ../../../
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 4 1 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 4 2 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 4 3 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 4 4 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 1 1 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 1 2 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 2 1 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 2 2 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 3 1 100
sudo ./scripts/network_simulation/start_network_node.sh delay 4 5 3 2 100
sudo ip netns exec ramjet-s4-n5 ./target/debug/bitcoin --p2p 10.0.4.10:6000 --api 10.0.4.10:7000 -c 10.0.4.2:6000 -c 10.0.4.4:6000 -c 10.0.4.6:6000 -c 10.0.4.8:6000 -c 10.0.1.2:6000 -c 10.0.1.4:6000 -c 10.0.2.2:6000 -c 10.0.2.4:6000 -c 10.0.3.2:6000 -c 10.0.3.4:6000 --shardId 3 --nodeId 19 --experNumber 0 --shardNum 5 --shardSize 5 --blockSize 2048 --k 6 --domesticRatio 0.7 --eDiff 0000025851eb851eb851eb851eb851eb851eb851eb851eb851eb851eb851eb80 --iDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff