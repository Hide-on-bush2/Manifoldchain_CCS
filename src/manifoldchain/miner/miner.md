# Miner

Miner gets transactions and testimonies from Mempool, performs double-spending checking via Validator, and runs PoW to generate a valid block. Uporn a valid block is generated, it is passed to Multichain for insertion and it is simultaneously passed to Network for broadcasting.

## Member

`multichain`
* Type: `Multichain`
* Description: Miner get necessary information from Multichain, such as longest chain hash, unverified leaves, and states, to choose valid transactions from Mempool for packaging.

`mempool`
* Type: `Mempool`
* Description: The source of transactions and testimonies.

`validator`
* Type: `Validator`
* Description: Provide verification functionalities.

`config`
* Type: `Configuration`

## Sharing Mining

Different from Nakamoto's Consensus, sharing mining works on multiple parents simutaneously

Pesudocode:

```
function pow(txs, tmys, chains) -> (ConsensusBlock, TransactionBlock) [
    timestamp = now()
    difficulty = self.config.difficulty
    
    //Generate transaction block
    tx_block = TransactionBlock::create(txs, tmys)

    tx_merkle_root = tx_block.get_tx_merkle_root()
    tmy_merkle_root = tx_block.get_tmy_merkle_root()

    //Generate chain merkle root
    chain_merkle_tree = MerkleTree::create(chains)
    chain_merkle_root = chain_merkle_tree.root()

    //Try nonce
    nonce = random()

    //Create block header
    header = BlockHeader::create(
        nonce,
        difficulty,
        self.shard_id,
        timestamp,
        tx_merkle_root
    );

    //Generate consensus block
    cons_block = ConsensusBlock::create(
        header,
        tmy_merkle_root,
        chain_merkle_root
    );

    return (cons_block, tx_block)
]
```

## Main Loop Function

Pseudocode:

```
function main_loop() {
    while(true)
        sleep(1)
        //Check whether the transactions in mempool are enough
        if self.mempool.get_size() < self.config.block_size 
            continue
        end if

        longest_chain_hash = self.multichain.get_longest_chain_hash()
        //Get latest static state and dynamic state
        static_state = self.multichain.get_state_state(longest_chain_hash)
        dynamic_state = self.multichain.get_dynamic_state
        (longest_chain_hash)

        counter = 0
        //Create empty txs and tmys for generating blocks
        txs = Vec::new()
        tmys = Vec::new()

        //Create empty invalid_txs and invalid_tmys to push those transactions and testimonies that are not chosen
        invalid_txs = Vec::new()
        invalid_tmys = Vec::new()

        //Create an empty set to avoid double-spending inside the same block, e.g., there are two same inputs in the new generated block
        set = HashMap<H256, bool>::new()

        while counter < self.config.block_size
            (tx, tmy) = self.mempool.pop_one_tx()

            //If the mempool is empty, break the loop
            if tx is None 
                break
            end if

            //1. Check whether there exits a testimony unit for each cross-shard input of tx in tmy
            //2. Check whether the double spending occurs inside the block
            for input in tx.inputs
                if tmy_unit of input does not exit in tmy
                    continue outside loop
                end if
                if input.hash() exits in keys of set
                    continue outside loop
                end if
            end for

            //check whether all inputs of tx use the UTXO in static_state or dynamic_state
            if self.validator.check_tx_from_state(
                tx,
                tmy,
                static_state,
                dynamic_state
            ) 
                counter += 1
                txs.push(tx)
                if tmy is not None
                    tmys.push(tmy)
                end if
            else
                invalid_txs.push(tx)
                if tmy is not None
                    invalid_tmys.push(tmy)
                end if
            end if
        end while

        push all txs in invalid_txs back to self.mempool
        push all tmys in invalid_tmys back to self.mempool

        if counter < self.config.block_size
            push txs back to self.mempool
            push tmys back to self.mempool
        end if

        //Get all unverified forks across all shards
        chains_hash = self.multichain.get_all_available_forks()

        //Run PoW to generate a block
        (cons_block, tx_block) = pow(
            txs,
            tmys,
            chains
        )

        blk_hash = cons_block.hash()

        //Check the validity of PoW
        if blk_hash > self.config.difficulty 
            continue
        end if

        //If the hash value is smaller than a set threshold, it is an exclusive block,
        //otherwides it is an inclusive block
        if blk_hash < self.config.threshold
            ex_full_block = ExclusiveFullBlock::create(
                cons_block,
                tx_block
            );
            broadcast ex_full_block
            insert ex_full_block to self.multichain
        else
            in_full_block = InclusiveFullBlock::create(
                cons_block,
                tx_block
            )
            broadcast in_full_block
            insert in_full_block to sekf.multichain
        end if

        //Generate testimonies for out-shard transactions
        new_tmys = Vec::new()
        for i in 0-txs.len()
            tx = txs[i] 
            outputs = tx.outputs
            if no output in outputs is cross-shard
                continue
            end if

            tmy_units = Vec::new()
            for input in tx.inputs
                tmy_unit = TestimonyUnit::create(
                    input.hash(),
                    blk_hash,
                    tx_block.get_tx_merkle_proof(),
                    i
                )
                tmy_units.push(tmy_unit)
            end for
            tmy = Testimony::create(
                tx_hash,
                tmy_units
            )
            new_tmys.push(tmy)
        end for
        broadcast new_tmys
    end while
}
```


