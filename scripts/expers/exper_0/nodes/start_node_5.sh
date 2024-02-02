#!/bin/bash
cd ../../../
sudo ./scripts/network_simulation/start_network_node.sh delay 2 1 1 1 100
sudo ./scripts/network_simulation/start_network_node.sh delay 2 1 1 2 100
sudo ip netns exec ramjet-s2-n1 ./target/debug/bitcoin --p2p 10.0.2.2:6000 --api 10.0.2.2:7000 -c 10.0.1.2:6000 -c 10.0.1.4:6000 --shardId 1 --nodeId 5 --experNumber 0 --shardNum 5 --shardSize 5 --blockSize 2048 --k 6 --domesticRatio 0.7 --eDiff 000000e7ae147ae147ae147ae147ae147ae147ae147ae147ae147ae147ae1479 --iDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff