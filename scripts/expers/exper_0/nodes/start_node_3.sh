#!/bin/bash
cd ../../../
sudo ./scripts/network_simulation/start_network_node.sh delay 1 4 1 1 100
sudo ./scripts/network_simulation/start_network_node.sh delay 1 4 1 2 100
sudo ./scripts/network_simulation/start_network_node.sh delay 1 4 1 3 100
sudo ip netns exec ramjet-s1-n4 ./target/debug/bitcoin --p2p 10.0.1.8:6000 --api 10.0.1.8:7000 -c 10.0.1.2:6000 -c 10.0.1.4:6000 -c 10.0.1.6:6000 --shardId 0 --nodeId 3 --experNumber 0 --shardNum 5 --shardSize 5 --blockSize 2048 --k 6 --domesticRatio 0.7 --eDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff --iDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff