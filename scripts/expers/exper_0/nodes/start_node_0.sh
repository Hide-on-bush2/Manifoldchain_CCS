#!/bin/bash
cd ../../../
sudo ip netns exec ramjet-s1-n1 ./target/debug/bitcoin --p2p 10.0.1.2:6000 --api 10.0.1.2:7000 --shardId 0 --nodeId 0 --experNumber 52 --shardNum 5 --shardSize 5 --blockSize 2048 --k 6 --eDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff --iDiff 0000007fffffffffffffffffffffffffffffffffffffffffffffffffffffffff