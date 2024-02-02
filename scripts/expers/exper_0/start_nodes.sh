#!/bin/bash
sudo ../../network_simulation/start_network_node.sh add 1 1 5000
sudo ../../network_simulation/start_network_node.sh add 1 2 5000
sudo ../../network_simulation/start_network_node.sh add 1 3 5000
sudo ../../network_simulation/start_network_node.sh add 1 4 5000
sudo ../../network_simulation/start_network_node.sh add 1 5 5000
sudo ../../network_simulation/start_network_node.sh add 2 1 10000
sudo ../../network_simulation/start_network_node.sh add 2 2 10000
sudo ../../network_simulation/start_network_node.sh add 2 3 10000
sudo ../../network_simulation/start_network_node.sh add 2 4 10000
sudo ../../network_simulation/start_network_node.sh add 2 5 10000
sudo ../../network_simulation/start_network_node.sh add 3 1 20000
sudo ../../network_simulation/start_network_node.sh add 3 2 20000
sudo ../../network_simulation/start_network_node.sh add 3 3 20000
sudo ../../network_simulation/start_network_node.sh add 3 4 20000
sudo ../../network_simulation/start_network_node.sh add 3 5 20000
sudo ../../network_simulation/start_network_node.sh add 4 1 40000
sudo ../../network_simulation/start_network_node.sh add 4 2 40000
sudo ../../network_simulation/start_network_node.sh add 4 3 40000
sudo ../../network_simulation/start_network_node.sh add 4 4 40000
sudo ../../network_simulation/start_network_node.sh add 4 5 40000
sudo ../../network_simulation/start_network_node.sh add 5 1 60000
sudo ../../network_simulation/start_network_node.sh add 5 2 60000
sudo ../../network_simulation/start_network_node.sh add 5 3 60000
sudo ../../network_simulation/start_network_node.sh add 5 4 60000
sudo ../../network_simulation/start_network_node.sh add 5 5 60000
for ((i=0; i<5; i++))
do
  for ((j=0; j<5; j++))
  do
    node_id=$[i*5+j]
    ./nodes/start_node_$node_id.sh 2>&1 | tee ../../../log/exper_0/$node_id.log &
  done
done