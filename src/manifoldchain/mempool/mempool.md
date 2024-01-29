# Mempool 

Mempool receives format-verified transactions and testimonies from network and matches testimonies to its corresponding transaction. Uporn requests from Miner, it returns transactions with their corresponding testimonies.

## Member

`txs_map`:
* Type: `HashMap<H256, Transaction>`
* Description: Mapping from a transaction's hash value to its exact value

`txs_queue`:
* Type: `VecDeque<H256>`
* Description: Uporn receiving a transaction, push it to the back ofthe queue. Uporn requesting from Miner, pop a transaction from the front of the queue.

`tx2tmy`:
* Type: `HashMap<H256, H256>`
* Description: Mapping from a transaction's hash value to the hash value of its corresponding testimony. The member matches each pair of associated transaction and testimony.

`testimony_map`:
* Type: `HashMap<H256, Testimony>`
* Description: Mapping from a testimony's hash value to its exact value

## Static Function

`new`:
* Inputs: `None`
* Outputs: `Mempool`
* Description: Create a new, empty Mempool

## Interface (Member Function)

`insert_tx`:
* Inputs: Transaction (`Transaction`)
* Outputs: Success or not (`bool`)
* Description: Inserting a transaction into Mempool and update all information involed.
* Pseudocode:
```
function insert_tx(tx) -> boolean {
    hash = tx.hash()
    if self.txs_map.contains_key(hash) 
        return false
    else
        self.txs_map.insert(hash, tx)
        self.txs_queue.push(hash)
        return true
    end if
}
```

`get_tx`:
* Inputs: Hash value (`H256`)
* Outputs: Transaction or None (`Option<Transaction>`)
* Description: Return a transaction whose hash value equals to the input if it exits, ortherwise return `None`
* Pseudocode:
```
function get_tx(tx_hash) -> Option<Transaction> {
    if self.txs_map.contains_key(tx_hash) 
        return Some(self.txs_map.get_value(tx_hash))
    else 
        return None
    end if
}

```

`delete_txs`:
* Inputs: Hash value set (`Vec<H256>`)
* Outputs: Success or not (`bool`)
* Description: Delete all transactions whose hash value included in the provided hash value set.
* Pseudocode:
```
function delete_txs(tx_hash_set) -> bool {
    for tx_hash in tx_hash_set
        delete item whose key is tx_hash in self.txs_map
        delete tx_hash in self.txs_queue
    end for
}
```

`pop_one_tx`:
* Inputs: `None`
* Outputs: Transaction and its associated testmony if exits or None (`(Option<Transaction>, Option<Testimony>)`)
* Description: Pop a transaction from the front of the queue if the queue is not empty, and return `None` if the queue is empty. If the transaction's corresponding testimony exits in the Mempool (not all transactions are cross-shard transactions and have associated testimonies), return it as well.
* Pseudocode
```
function pop_one_tx() -> (Option<Transaction>, Option<Testimony>) {
    if self.txs_queue is empty 
        return (None, None)
    else 
        tx_hash = self.txs_queue.pop()
        tx = self.txs_map.get_value(tx_hash)
        delete item whose key is tx_hash in self.txs_map

        if tx_hash exits in keys of self.tx2tmy 
            tmy_hash = self.tx2tmy.get_value(tx_hash)
            tmy = self.testimony_map.get_value(tmy_hash)

            delete item whose key is tmy_hash in self.testimony_map
            delete item whose key is tx_hash in self.tx2tmy

            return (Some(tx), Some(tmy))
        else
            return (Some(tx), None)
        endif
    end if
}
```

`add_testimony`:
* Inputs: Testimony (`Testimony`)
* Outputs: Hash value of corresponding transaction if exits, otherwise None (`Option<H256>`)
* Description: 
    * The current implementation requires that each testimony in the Mempool should have its corresponding transaction. If the input testimony's corresponding transaction exits in the Mempool, update all associated information and return the transaction's hash value, otherwise return None.

    * When updating the associated information of a new testimony, it happens that there's already an old testimony corresponding to the same transaction. That is because a cross-shard transaction may contain various UTXO inputs from different shard. A shard can only generate a partial testimony because it does not contain essentail information of other shards. In this case, we need to combine multiple testimonies into a full testimony.
* Pseudocode:
```
function add_testimony(tmy) -> Option<H256> {
    tx_hash = tmy.get_tx_hash()
    if tx_hash exits in keys of self.txs_map 
        tmy_hash = tmy.hash()
        if tx_hash exits in keys of self.tx2tmy
            old_tmy_hash = self.tx2tmy.get_value(tx_hash)
            old_tmy = self.testimony_map.get_value(old_tmy_hash)
            tmy = combine(old_tmy, tmy)
            tmy_hash = tmy.hash()
            delete item whose key is tmy_hash in self.testimony_map
        end if
        self.testimony_map.insert(tmy_hash, tmy)
        self.tx2tmy.insert(tx_hash, tmy_hash)
        return Some(tx_hash)
    else
        return None
    end if
}
```

`remove_testimony`:
* Inputs: Hash value of a testimony (`H256`)
* Outputs: Success or not (`bool`)
* Description: Delete all associated information of the testimony
* Pseudocode:
```
function remove_testimony(tmy_hash) -> bool {
    delete item whose key is tmy_hash in self.testimony_map
    delete item whose value is tmy_hash in self.tx2hash
    return true
}
```

`get_testimony`:
* Inputs: Hash value of a testimony (`H256`)
* Outputs: Testimony or None (`Option<Testimony>`)
* Description: Return testimony whose hash value equals to the input hash value. If the testimony does not exit, return None.

`get_testimony_by_tx`:
* Inputs: Hash value of a transaction (`H256`)
* Outputs: Testimony or None (`Option<Testimony>`)
* Description: Return testimony whose corresponding tx's hash value equals to the input hash value. If the testimony does not exit, return None.
* Pseudocode:
```
function get_testimony_by_tx(tx_hash) -> Option<Testimony> {
    if tx_hash exits in keys of self.tx2tmy
        tmy_hash = self.tx2tmy.get_value(tx_hash)
        tmy = self.testimony_map.get_value(tmy_hash)
        Some(tmy)
    else
        None
    end if
}
```




