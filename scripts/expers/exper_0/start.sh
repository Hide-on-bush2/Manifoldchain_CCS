#!/bin/bash
shard_num=5
shard_size=5
mining_interval=0
tx_generation_interval=25000000
runtime=2000
iter=0
exper_number=0
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
  echo "$c"
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
sleep 10