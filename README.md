# Implementation of Manifolchain

This project aims to design and implement a high-throughput sharding Blockchain system, employing novel mining protocol and verification mechanism.

## Architecture

![](./img/architecture.png)

## Overview of Modules

* **Mempool** receives format-verified transactions and testimonies from network and matches testimonies to its corresponding transaction. Uporn requests from Miner, it returns transactions with their corresponding testimonies.
* **Miner** gets transactions and testimonies from Mempool, performs double-spending checking via Validator, and runs PoW to generate a valid block. Uporn a valid block is generated, it is passed to Multichain for insertion and it is simultaneously passed to Network for broadcasting.
* **Network** is responsible for handling communication among different nodes. It runs Gossip protocol to receive, respond, and broadcast message. There are totally three types of message to handle: transaction, testimony, and block. When receiving messages, it delegates Validator for processing. 
* **Validator** inherits all the validation functionalities, including format validation and double-spending validation from transactions and blocks. It requests necessary information from Multichain for validation. There are totally four validation sources from Miner and Network:
    * Validate transactions from Network: check the format of transactions, including validity of signatures and coins. Verified transactions are pushed into the Mempool.
    * Validate blocks from Network: check both the format validity and double-spending existence based on node's view. Verified blocks are inserted into the Multichain.
    * Validate transactions from Miner: check the double-spending existence based on node's view to make sure that a new-generated block will be accepted by other nodes. (The first validation source already ensures that each transaction pop from Mempool meets format validity.) Validator returns `true` or `false` for each incoming transaction to Miner. 
* **Multichain** provides interfaces to write and read blockchains across all shards. 
* **Blockchain** maintains a ledger for one shard. It stores a tree for recording all historical forks, and updates states (available UTXO set) uporn each insertion. Blockchain implements sufficient interfaces for block insertion, fork pruning, and obtaining chain information. 
* **TX-generator** randomly generates transactions and broadcast them to associated nodes via Network. It is not a key component of Manifoldchain system but just for experiment.
 
