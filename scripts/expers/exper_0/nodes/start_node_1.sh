#!/bin/bash
cd ../../../
sudo ./scripts/network_simulation/start_network_node.sh delay 1 2 1 1 100
sudo ip netns exec ramjet-s1-n2 ./target/debug/bitcoin --p2p 10.0.1.4:6000 --api 10.0.1.4:7000 -c 10.0.1.2:6000 --shardId 0 --nodeId 1 --experNumber 0 --shardNum 5 --shardSize 5 --blockSize 2048 --k 6 --domesticRatio 0.7 --eDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff --iDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff