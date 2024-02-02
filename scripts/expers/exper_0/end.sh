#!/bin/bash

shard_num=5
shard_size=5
exper_number=0
iter=0

cd ../../virtual_network_operation/
./end_node.sh $shard_num $shard_size
sleep 5
cd ../expers/exper_$exper_number/
sudo mv ../../../log/exper_$exper_number/*.txt ../../../log/exper_$exper_number/iter_$iter/
sudo mv ../../../log/exper_$exper_number/*.log ../../../log/exper_$exper_number/iter_$iter/exec_log/
sudo cp ./config.json ../../../log/exper_$exper_number/iter_$iter/
sleep 10