# Blockchain

Blockchain module maintains a ledger for one shard. It stores a tree for recording all historical forks, and updates states (available UTXO set) uporn each insertion. Blockchain implements sufficient interfaces for block insertion, fork pruning, and obtaining chain information.

## Blockchain Tree

We use a tree to store all historical forks occur in the past. Each node in the tree represents a versa block. The structure of node in the tree are as follows:

* `val` (`H256`): the hash of the versa block
* `children` (`Vec<Box<Node>>`): a set containing all pointers that point to its children blocks
* `height` (`usize`): the height of the node/block starting from the genesis block
* `longest_height` (`usize`): the longest length of all forks starting from the node/block

## State

A state in Blockchain represents all available UTXO in a fork. We maintain a state for each block in the Blockchain, which means that a block's state represents all available UTXO in the fork which starts from the genesis block and ends in this block. A state is defined as `HashMap<(H256, u32), (Transaction, Option<Testimony>)>`. The key `(H256, u32)` is a transaction output, including the transaction's hash and the index of the output in the transaction, the value `(Transaction, Option<Testimony>)` includes the exact value of the transaction and its possibly existing testimony. Each item of a state provide all neccessary information to generate a new transaction that takes it as input.

**Static state and dynamic state.** There are some transaction outputs generated from all in-shard inputs, we call these outputs static outputs, and call the UTXOs grouped from static outputs static_state. Similarly, there are some transaction outputs generated patially from out-shard inputs, we call these outputs dynamic outputs, and call the UTXOs grouped from dynamic outputs dynamic state. 

The static state is fixed when it is created, while the dynamic state depends on other shards' views. The dynamic state becomes invalid when the associated transactions' testimonies become invalid. In our implementation, we maintain a `static_state` and a `dynamic_state` for each block.

## Member 

`hash2blk`
* Type: `HashMap<H256, VersaBlock>`
* Description: Mapping from versa blocks' hash to their exact values

`root`
* Type: `Box<Node>`
* Description: The pointer points to the root of the tree, the node of the genesis block

`hash2node`
* Type: `HashMap<H256, Node>`
* Description: Mapping from versa blocks' hash to their nodes for fast reading (we do not need to traverse the tree when we want to know the location of the node).

`hash2ver_status`
* Type: `HashMap<H256, bool>`
* Description: Mapping from versa blocks' hash to their verification status. `true` represents verified and `false` represents unverified. For a full block including exclusive full block and inclusive full block, its status is always `true` because it can be verified one it was inserted to Blockchain; for an exlusive block or inclusive block, its status is `false` when it was inserted. The status whill be changed to `true` when it get verified in the future. If the verification fails the block and its children will be removed from the Blockchain.

`tx_map`
* Type: `HashMap<H256, (H256, usize)>`
* Description: Mapping from transaction's hash to the hash of the block it locates at, and its index in the block. By maintaining this data, we can quikly check whether a transaction exits in the Blockchain. It is usally used in verifying a coming transaction in Network module. If it is already in the Blockchain, drop it.

`static_states`
* Type: `HashMap<H256, State>`
* Description: Mapping from block's hash to its static state

`dynamic_states`
* Type: `HashMap<H256, State>`
* Description: Mapping from block's hash to its dynamic state

`leveas`
* Type: `Vec<H256>`
* Description: Including all unverified forks extending from the longest verified one. 

`longest_chain_hash`
* Type: `H256`
* Description: The hash of last block of the longest fork

`longest_verified_chain_hash`
* Type: `H256`
* Description: The hash of last block of the longest verified fork (containing only verified blocks)

`heigt`
* Type: `usize`
* Descripton: The maximum value of blocks' heights

`config`
* Type: `Configuration`
* Description: Basic configuration of blockchain, such as block size

## Static Function

`new`
* Inputs: None
* Outputs: `Blockchain`
* Description: Creating a Blockchain containing only genesis block

## Interface (Public Member Function)

`insert_exfullblock`
* Inputs: `ExclusiveFullBlock`
* Outputs: Sucess or not `bool`
* Description: This interface takes an exclusive block as argument and insert it to the Blockchain. If its parent block exits, then insertion is expected to succeed, we then update all related status; if its parent block does not exit, the insertion fails. 
* Pseudocode: 
```
function insert_exfullblock(ex_full_block) -> (bool, bool) {
    blk_hash = ex_full_block.hash()
    //Check whether it already exits in the blockchain
    if blk_hash exits in keys of self.hash2blk 
        return false
    end if

    parent_hash = ex_full_block.get_parent()
    //Insert the block to the tree
    node = insert_block_to_tree(self.root, parent_hash)

    //Insertion fails
    if node is None
        return false
    end if

    //Insertion succeeds
    //Update verification status
    //Because it is a full block, its status is initially true
    self.hash2ver_status.insert(blk_hash, true)

    //Update the longest verified chain hash
    travers the tree and set the current self.longest_verified_chain_hash

    //Update mapping hash maps
    self.hash2blk.insert(blk_hash, ex_full_block)
    self.hash2node.insert(blk_hash, node)

    //Update the longest chain hash and height
    if node.height > self.height 
        self.height = node.height
        self.longest_chain_hash = blk_hash
    end if

    //Update the state
    txs = ex_full_block.get_txs()
    tmys = ex_full_block.get_tmys()

    //Get the state of the parent block,
    //because the new block's state inherits from its parent 
    static_state = self.static_states.get(parent_hash)
    dynamic_state = self.dynamic_states.get(parent_hash)

    for i in 0-txs.len()
        tx = txs[i]
        tx_hash = tx.hash()
        //Update tx_map
        self.tx_map.insert(tx_hash, tx)

        
        inputs = tx.get_inputs()
        is_dynamic = false
        for input in inputs
            //Delete all used UTXO in the block
            static_state.remove_key((tx_hash, input.index))
            dynamic_state.remove_key((tx_hash, input.index))

            //Check whether there exits an input from other shards
            if input.shard_id != self.shard_id
                is_dynamic = true
            end if
        end for

        outputs = tx.get_outputs()
        for j in 0-outputs.len()
            output = outputs[j]

            //If the output does not belong to the current shard, skip it
            if output.shard_id != self.shard_id
                continue
            end if

            if is_dynamic
                tmy = tmys.get_tmy_by_tx(tx_hash)
                dynamic_state.insert(
                    (tx_hash, j),
                    (tx, Some(tmy))
                )
            else
                static_state.insert(
                    (tx_hash, j),
                    (tx, None)
                )
            end if
        end for
    end for

    //Insert the new state to hash map
    self.static_states.insert(blk_hash, static_state)
    self.dynamic_states.insert(blk_hash, dynamic_state)
    return true
}
```

`insert_infullblock`
* Inputs: `InclusiveFullBlock`
* Outputs: Sucess or not `bool`
* Description: This interface takes an inclusive full block as argument and insert it to the Blockchain. If its parent block exits, then insertion is expected to succeed, we then update all related status; if its parent block does not exit, the insertion fails. 
* Pseudocode: Similar to `insert_exfullblock`

`insert_exblock`
* Inputs: `ExclusiveBlock`
* Outputs: Sucess or not
* Description: This interface takes an exclusive block as argument and insert it to the Blockchain. If its parent block exits, then insertion is expected to succeed, we then update all related status; if its parent block does not exit, the insertion fails. Different from `insert_exfullblock` and `insert_infullblock`, `ExclusiveBlock` does not have transactions, so we do not need to update the State.
* Pseudocode:
```
function insert_exfullblock(ex_full_block) -> (bool, bool) {
    blk_hash = ex_full_block.hash()
    //Check whether it already exits in the blockchain
    if blk_hash exits in keys of self.hash2blk 
        return false
    end if

    parent_hash = ex_full_block.get_parent()
    //Insert the block to the tree
    node = insert_block_to_tree(self.root, parent_hash)

    //Insertion fails
    if node is None
        return false
    end if

    //Insertion succeeds
    //Update verification status
    //Because it is a full block, its status is initially true
    self.hash2ver_status.insert(blk_hash, true)

    //Update the longest verified chain hash
    travers the tree and set the current self.longest_verified_chain_hash

    //Update mapping hash maps
    self.hash2blk.insert(blk_hash, ex_full_block)
    self.hash2node.insert(blk_hash, node)

    //Update the longest chain hash and height
    if node.height > self.height 
        self.height = node.height
        self.longest_chain_hash = blk_hash
    end if

    //Get the state of the parent block,
    //because the new block's state inherits from its parent 
    static_state = self.static_states.get(parent_hash)
    dynamic_state = self.dynamic_states.get(parent_hash)

    //Because an exclusive block does not have transactions, its state directly inherits from its parent
    self.static_states.insert(blk_hash, static_state)
    self.dynamic_states.insert(blk_hash, dynamic_state)
    return true
}
```

`insert_inblock`
* Inputs: `InclusiveBlock`
* Outputs: Sucess or not
* Description: This interface takes an inclusive block as argument and insert it to the Blockchain. If its parent block exits, then insertion is expected to succeed, we then update all related status; if its parent block does not exit, the insertion fails. Different from `insert_exfullblock` and `insert_infullblock`, `InclusiveBlock` does not have transactions, so we do not need to update the State.
* Pseudocode: Similar to `insert_exblock`


