use crate::{
    types::{
        hash::{H256, Hashable},
    },
    manifoldchain::{
        block::{
            Info,
            Content,
            exclusive_block::ExclusiveBlock,
            inclusive_block::InclusiveBlock,
            transaction_block::TransactionBlock,
            consensus_block::ConsensusBlock,
            versa_block::{
                ExclusiveFullBlock,
                InclusiveFullBlock,
                VersaBlock,
            }
        },
        testimony::{
            Testimony,
        },
        transaction::{
            Transaction,
            TxFlag,
        },
        configuration::Configuration,  
        validator::{
            Validator,
            CrossUtxoStatus,
        },
        database::Database,
    }
};
use std::{
    cmp,
    collections::{HashMap, VecDeque},
    fs::File,
    io::{Write, Error},
    time::SystemTime,
};
use log::{debug, info};
use chrono::{DateTime, Local};

#[derive(Clone)]
pub struct Node {
    pub val: H256,
    pub children: Vec<Box<Node>>,
    pub height: usize,
    pub longest_height: usize,
}

#[derive(Clone, PartialEq, Debug)]
pub enum VerStatus {
    Unverified,
    Verified,
    Pruned,
}

pub type State = HashMap<(H256, u32), (Transaction, Option<Testimony>)>;

impl Hashable for State {
    fn hash(&self) -> H256 {
        let mut hash_vec: Vec<H256> = vec![];
        for (key, _) in self.iter() {
            let hash_str: String = key.0.into();
            let key_str = format!("{}{}",
                hash_str,
                key.1
            );
            let key_hash: H256 = ring::digest::digest(
                &ring::digest::SHA256, key_str.as_bytes()
            ).into();
            hash_vec.push(key_hash);
        }
        H256::multi_hash(&hash_vec)
    }
}

pub struct Blockchain {
    //hash2blk: HashMap<H256, VersaBlock>, //blk_hash -> block
    hash2blk: Database<VersaBlock>,
    //Rust does not allow two pointers to point to the same variable
    hash2node: HashMap<H256, Node>, //blk_hash -> node
    hash2ver_status: HashMap<H256, VerStatus>, //blk_hash -> verified or not
    root: Box<Node>,
    tx_map: HashMap<H256, Vec<(H256, usize)>>, //tx_hash -> (block_hash, index), one tx may exit in
    //multiple blocks
    //states: HashMap<H256, State>, //block_hash -> static state
    states: Database<State>,
    leaves: Vec<H256>,
    unverified_blocks: HashMap<(H256, usize), bool>,
    //to facilitate the insertion of two same blocks with the same parent
    //as sharing mining enables a block to have multiple  parents
    //they should be identified by a "parent-child" pair
    //the map is utilized to skip the same pair
    dp_map: HashMap<(H256, H256), bool>,//(parent, child) 
    pub longest_chain_hash: H256,
    pub longest_verified_chain_hash: H256,
    pub height: usize,
    pub verified_height: usize,
    pub shard_id: usize,
    pub config: Configuration,
}

//prune the branches which are not growing on the longest chain. 
//should return the pruned blocks's hash to delete the corresponding states
impl Node {
    pub fn pre_traverse(root: &Box<Node>) -> Vec<H256> {
        let mut res: Vec<H256> = vec![root.val.clone()];
        for child in root.children.iter() {
            let t = Self::pre_traverse(child);
            res.extend(t);
        }
        res
    }
    pub fn insert(
        root: &mut Box<Node>, 
        parent: &H256, 
        hash: H256, 
        k: usize
    ) -> Option<Box<Node>>
    {
        if (&root.val).eq(parent) {
            //check whether the node exits
            //if exits, return that node and nothing would be deleted
            for n in root.children.iter() {
                if n.val == hash {
                    return Some(n.clone());
                }
            }
            //creating a new node. As there is only one new child, nothing would
            //be deleted
            let new_node: Box<Node> = Box::new(Node{
                val: hash,
                children: Vec::new(),
                height: root.height + 1,
                longest_height: root.height + 1
            });
            root.children.push(new_node.clone());
            if new_node.longest_height > root.longest_height {
                root.longest_height = new_node.longest_height;
            }
            Some(new_node)
        } else {
            let mut return_node: Option<Box<Node>> = None;
            for item in root.children.iter_mut() {
                let sub_return_node = Self::insert(item, parent, hash, k);
                match sub_return_node {
                    Some(res) => {
                        //If the new node is extending the longest chain, we gonna 
                        //delete something
                        if res.longest_height > root.longest_height {
                            root.longest_height = res.longest_height;
                        }
                        return_node = Some(res);
                        break;
                    }
                    None => {}
                }
                //Anyway, sub_pruned_nodes is Some only if sub_return_node is Some
                //but for beauty I split the logics of them
                
            }
            
            return_node 
        }
    }

    pub fn get_leaves(root: &Box<Node>) -> Vec<H256> {
        if root.children.is_empty() {
            let res: Vec<H256> = vec![root.val.clone()];
            res
        } else{
            let mut res: Vec<H256> = Vec::new();
            for child in root.children.iter() {
                let leaves = Self::get_leaves(child);
                res.extend(leaves);
            }
            res 
        }
    }
    //if pruning succeed, return all deleted hash, otherwise return None
    pub fn prune(root: &mut Box<Node>, hash: &H256) -> Option<Vec<H256>> {
         if root.children.is_empty() {
            None
        } else {
            let mut is_prune = false;
            let mut deleted_hash: Option<Vec<H256>> = None;
            for child in root.children.iter() {
                if (&child.val).eq(hash) {
                    is_prune = true;
                    deleted_hash = Some(Self::pre_traverse(child));
                    break;
                }
            }
            if is_prune {
                root.children.retain(|x| !(&x.val).eq(hash));
                root.longest_height = Self::get_longest_height(root);
            } else {
                for child in root.children.iter_mut() {
                    deleted_hash = Self::prune(child, hash);
                    if deleted_hash.is_some() {
                        root.longest_height = Self::get_longest_height(root);
                        break;
                    }
                }
            }
            deleted_hash
        }
    }

    fn get_longest_height(root: &Box<Node>) -> usize {
        if root.children.is_empty() {
            root.height
        } else {
            let mut longest_height = root.height;
            for child in root.children.iter() {
                longest_height = cmp::max(
                    longest_height, 
                    Self::get_longest_height(child)
                );
            }
            longest_height
        }
    }

    fn get_longest_chain_hash(root: &Box<Node>) -> (H256, usize) {
        if root.children.is_empty() {
            (root.val.clone(), root.height)
        } else {
            let mut longest_height = root.height;
            let mut longest_hash = root.val.clone();
            for child in root.children.iter() {
                let (sub_hash, sub_height) = Self::get_longest_chain_hash(child);
                if sub_height > longest_height {
                    longest_height = sub_height;
                    longest_hash = sub_hash;
                }
            }
            (longest_hash, longest_height)
        }
    }

    //As there are multiple nodes with the same hash, this function only return the longest one
    //old version
    //pub fn get_path(root: &Box<Node>, hash: &H256) -> Option<Vec<H256>> {
    //    if (&root.val).eq(hash) {
    //        let mut res: Vec<H256> = Vec::new();
    //        res.push(hash.clone());
    //        Some(res)
    //    } else {
    //        let mut res: Vec<H256> = Vec::new();
    //        for item in root.children.iter() {
    //            match Self::get_path(item, hash) {
    //                Some(ret) => {
    //                    res.push(root.val.clone());
    //                    res.extend(ret);
    //                    break;
    //                }
    //                None => {}
    //            }
    //        }
    //        if res.is_empty() {
    //            None
    //        } else {
    //            Some(res)
    //        }
    //    }
    //}
    //new version
    pub fn get_path(root: &Box<Node>, hash: &H256) -> Option<Vec<H256>> {
        if (&root.val).eq(hash) {
            let mut res: Vec<H256> = Vec::new();
            res.push(hash.clone());
            Some(res)
        } else {
            let mut longest_res: Vec<H256> = vec![];
            for item in root.children.iter() {
                match Self::get_path(item, hash) {
                    Some(ret) => {
                        if ret.len() > longest_res.len() {
                            longest_res = ret;
                        }
                    }
                    None => {}
                }
            }
            if longest_res.is_empty() {
                None
            } else {
                let mut res = vec![];
                res.push(root.val.clone());
                res.extend(longest_res);
                Some(res)
            }
        }
    }


    pub fn print_tree(root: &Box<Node>) {
        let mut queue: VecDeque<&Box<Node>> = VecDeque::new();
        queue.push_back(root);
        while !queue.is_empty() {
            let mut tvec: Vec<&Box<Node>> = Vec::new();
            while let Some(node) = queue.pop_back() {
                tvec.push(node);
            }
            for item in tvec.iter() {
                print!("{} ", hex::encode(&item.val.0));
                for item2 in item.children.iter() {
                    queue.push_back(item2);
                }
            }
            println!("");
        }
    }

    pub fn get_node_by_hash(root: &Box<Node>, hash: &H256) -> Option<Box<Node>> {
        if root.val == *hash {
            Some(root.clone())
        } else {
            for child in root.children.iter() {
                match Self::get_node_by_hash(child, hash) {
                    Some(node) => {
                        return Some(node);
                    }
                    None => {}
                }
            }
            None
        }
    }

    pub fn get_leaves_start_from(root: &Box<Node>, hash: &H256) -> Option<Vec<H256>> {
        if root.val == *hash {
            Some(Self::get_leaves(root))
        } else {
            for child in root.children.iter() {
                match Self::get_leaves_start_from(child, hash) {
                    Some(leaves) => {
                        return Some(leaves);
                    }
                    None => {}
                }
            }
            None
        }
    }

    pub fn get_longest_verified_fork(
        root: &Box<Node>, 
        ver_status: &HashMap<H256, VerStatus>
    ) -> Option<(H256, usize)> {
        match ver_status.get(&root.val) {
            Some(res) => {
                if let VerStatus::Verified = res {
                    let mut longest_verified_hash = root.val.clone();
                    let mut max_height = root.height;
                    for child in root.children.iter() {
                        match Self::get_longest_verified_fork(child, ver_status) {
                            Some((hash, height)) => {
                                if max_height < height {
                                    longest_verified_hash = hash;
                                    max_height = height;
                                } else if max_height == height {
                                    if hash < longest_verified_hash {
                                        longest_verified_hash = hash;
                                        max_height = height;
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                    Some((longest_verified_hash, max_height))
                } else {
                    None
                }
            }
            None => None
        }
    }
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new(config: &Configuration, shard_id: usize) -> Self {
        //create genesis block
        let (ex_blk, tx_blk) = ExclusiveBlock::generate(
            H256::default(), //verified_parent
            shard_id, //shard_id
            0, //nonce
            H256::default(), //difficulty
            vec![], //txs
            vec![], //tmys
            vec![], //inter_parents
            vec![], //global_parents
        );

        let mut cons_blk = ConsensusBlock::default();
        cons_blk.set_shard_id(shard_id);
        let cons_hash = cons_blk.hash();
        let ex_blk = ExclusiveBlock::create(
            cons_blk,
            cons_hash,
            vec![],
        );


        
        let genesis_hash = ex_blk.hash();
        let genesis_block = VersaBlock::ExBlock(ex_blk);


        let now = SystemTime::now();
        //let mut hash2blk: HashMap<H256, VersaBlock> = HashMap::new();
        let mut hash2blk: Database<VersaBlock> = 
          Database::<VersaBlock>::new(format!("{:?}/blockchain/hash2blk", now));
        let _ = hash2blk.insert(genesis_hash.clone(), genesis_block.clone());
        let root = Box::new(Node {
            val: genesis_hash.clone(),
            children: Vec::new(),
            height: 0,
            longest_height: 0,
        });
        let longest_chain_hash = genesis_hash.clone();
        let longest_verified_chain_hash = genesis_hash.clone();
        let height = 0 as usize;
        let verified_height = 0 as usize;
        let mut hash2node: HashMap<H256, Node> = HashMap::new();
        hash2node.insert(genesis_hash.clone(), (*root).clone());

        let mut hash2ver_status: HashMap<H256, VerStatus> = HashMap::new();
        hash2ver_status.insert(genesis_hash.clone(), VerStatus::Verified);

        //intitialize a empty state to the genesis block
        let initial_state: State = HashMap::new();
        //let mut states: HashMap<H256, State> = HashMap::new();
        let mut states: Database<State> = 
            Database::<State>::new(format!("{:?}/blockchain/states", now));
        let _ = states.insert(genesis_hash.clone(), initial_state.clone());

        let leaves: Vec<H256> = vec![genesis_hash.clone()];

        Blockchain {
            hash2blk,
            hash2node,
            hash2ver_status,
            root,
            tx_map: HashMap::new(),
            states,
            unverified_blocks: HashMap::new(),
            dp_map: HashMap::new(),
            longest_chain_hash,
            longest_verified_chain_hash,
            height,
            verified_height,
            config: config.clone(),
            leaves,
            shard_id,
        }
    }

    pub fn get_longest_verified_fork(&self) -> H256 {
        self.longest_verified_chain_hash.clone()
    }
    
    fn delete_block(&mut self, hash: &H256) {
        self.hash2blk.remove(hash);
        self.hash2node.remove(hash);
        self.hash2ver_status.remove(hash);
        self.states.remove(hash);
        //self.tx_map.retain(|_, val| *hash != val.0);
    }

    //pub fn insert_block(&mut self, block: VersaBlock) 
    //-> Result<Option<(VersaBlock, usize)>, String> 
    //{
    //    let blk_hash = block.hash();
    //    if let Some(_) = self.hash2blk.get(&blk_hash) {
    //        return Err(String::from("block already exit"));
    //    }

    //    let parents: Vec<H256> = match block.clone() {
    //        VersaBlock::ExBlock(ex_block) => {
    //            let mut valid_parents: Vec<H256> = vec![];
    //            for parent in ex_block.get_inter_parents() {
    //                //check whether the parent exits
    //                if let Some(_) = self.hash2blk.get(&parent) {
    //                    //check whether the block is pruned
    //                    let ver_sta = self.hash2ver_status
    //                        .get(&parent)
    //                        .unwrap()
    //                        .clone();
    //                    if let VerStatus::Pruned = ver_sta {
    //                        continue;
    //                    }
    //                    valid_parents.push(parent);
    //                }
    //            }
    //            valid_parents
    //        }
    //        VersaBlock::ExFullBlock(ex_full_block) => {
    //            let mut valid_parents: Vec<H256> = vec![];
    //            for parent in ex_full_block.get_inter_parents() {
    //                //check whether the parent exits
    //                if let Some(_) = self.hash2blk.get(&parent) {
    //                    //check whether the block is pruned
    //                    let ver_sta = self.hash2ver_status
    //                        .get(&parent)
    //                        .unwrap()
    //                        .clone();
    //                    if let VerStatus::Pruned = ver_sta {
    //                        continue;
    //                    }
    //                    valid_parents.push(parent);
    //                }
    //            }
    //            valid_parents

    //        }
    //        VersaBlock::InBlock(in_block) => {
    //            let possible_parents = in_block
    //                .get_global_parents_map()
    //                .get(&self.shard_id)
     //               .unwrap()
     //               .clone();
     //           let mut valid_parents: Vec<H256> = vec![];
     //           for parent in possible_parents {
     //               //check whether the parent exits
     //               if let Some(_) = self.hash2blk.get(&parent) {
     //                   //check whether the block is pruned
     //                   let ver_sta = self.hash2ver_status
     //                       .get(&parent)
     //                       .unwrap()
     //                       .clone();
     //                   if let VerStatus::Pruned = ver_sta {
     //                       continue;
     //                   }
     //                   valid_parents.push(parent)
     //               }
     //           }
     //           valid_parents
     //       }
     //       VersaBlock::InFullBlock(in_full_block) => {
     //           let possible_parents = in_full_block
     //               .get_global_parents_map()
     //               .get(&self.shard_id)
     //               .unwrap()
     //               .clone();
     //           let mut valid_parents: Vec<H256> = vec![];
     //           for parent in possible_parents {
     //               //check whether the parent exits
     //               if let Some(_) = self.hash2blk.get(&parent) {
     //                   //check whether the block is pruned
     //                   let ver_sta = self.hash2ver_status
     //                       .get(&parent)
     //                       .unwrap()
     //                       .clone();
     //                   if let VerStatus::Pruned = ver_sta {
     //                       continue;
     //                   }
     //                   valid_parents.push(parent)
     //               }
     //           }
     //           valid_parents

     //       }
     //   };

     //   //check whether the parent set is empty
     //   if parents.is_empty() {
     //       return Err(String::from("no parents"));
     //   }
     //   

     //   for parent_hash in parents {
     //       let possible_node = Node::insert(
     //           &mut self.root,
     //           &parent_hash,
     //           blk_hash.clone(),
     //           self.config.k
     //       );

     //       if let None = possible_node {
     //           continue;
     //       }

     //       let new_node = possible_node.unwrap();

     //       //update hash2node
     //       self.hash2node.insert(blk_hash.clone(), (*new_node).clone());
     //       
     //       let mut extend_or_not = false;
     //       
     //       match block {
     //           VersaBlock::ExBlock(_) 
     //               => self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Unverified),
     //           VersaBlock::InBlock(_) 
     //               => self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Unverified),
     //           VersaBlock::ExFullBlock(_)
     //               => self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified),
     //           VersaBlock::InFullBlock(_)
     //               => self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified),
     //       };


     //       //update basic information
     //       self.hash2blk.insert(
     //           blk_hash.clone(),
     //           block.clone()
     //       );

     //       //update the longest chain information
     //       if new_node.height > self.height {
     //           self.height = new_node.height;
     //           self.longest_chain_hash = new_node.val.clone();
     //           extend_or_not = true;
     //       } 
     //           
     //       //update related state information
     //       let mut state = self.states
     //           .get(&parent_hash)
     //           .unwrap()
     //           .clone();

     //       //Exclusive block and inclusive block inherits their parent's state
     //       match block {
     //           VersaBlock::ExBlock(_) => {
     //               self.states.insert(blk_hash.clone(), state);
     //               continue;
     //           }
     //           VersaBlock::InBlock(_) => {
     //               self.states.insert(blk_hash.clone(), state);
     //               continue;
     //           }
     //           _ => {}
     //       };


     //       let txs = block.get_txs_ref().unwrap();
     //       let tmys = block.get_tmys().unwrap();
     //       (0..txs.len()).for_each(|i| {
    //            let tx = &txs[i];
    //            let tx_hash = tx.hash();
    //            match self.tx_map.get(&tx_hash) {
    //                Some(old_locations) => {
    //                    let mut new_locations = old_locations.clone();
    //                    new_locations.push((blk_hash.clone(), i));
    //                    self.tx_map.insert(tx_hash, new_locations);
    //                }
    //                None => {
    //                    self.tx_map.insert(tx_hash, vec![(blk_hash.clone(), i)]);
    //                }
    //            }
    //            self.update_state(tx, &mut state, &tmys);
    //        });
    //        self.states.insert(blk_hash, state);
    //    }
    //    //update the longest verified chain hash 
    //    let (longest_verified_hash, height) = Node::get_longest_verified_fork(
    //        &self.root,
    //        &self.hash2ver_status
    //    ).unwrap();
    //    self.longest_verified_chain_hash = longest_verified_hash;

    //    //update the longest verified chain information
    //    //update the confirmation information
    //    let mut possible_confirmed_block: Option<(VersaBlock, usize)> = None;
    //    if height > self.verified_height {
    //        self.verified_height = height;
    //        let history = self.all_blocks_end_with_block(&longest_verified_hash).unwrap();
    //        let confirmed_index = match height >= self.config.k {
    //            true => height - self.config.k,
    //            false => 0,
    //        };
    //        let confirmed_hash = history[confirmed_index];
    //        let confirmed_block = self.get_block(&confirmed_hash).unwrap();
    //        possible_confirmed_block = Some((confirmed_block, confirmed_index));
    //    }


    //    //update the unverified leaves
    //    self.leaves = Node::get_leaves_start_from(
    //        &self.root,
    //        &self.longest_verified_chain_hash
    //    ).unwrap();
    //    Ok(possible_confirmed_block)
    //}


    pub fn insert_block_with_parent(&mut self, block: VersaBlock, parent: &H256) 
        -> Result<Option<(VersaBlock, usize)>, String> 
    {
        let blk_hash = block.hash();
        let shard_id = block.get_shard_id();

        //check dp map if it is already inserted
        //if let Some(_) = self.dp_map.get(&(parent.clone(), blk_hash.clone())) {
        //    return Err(String::from("Block already exits"));
        //}

        //if let Some(_) = self.hash2blk.get(&blk_hash) {
        //    return Err(String::from("Block already exits"));
        //}
        let parents: Vec<H256> = match block.clone() {
            VersaBlock::ExBlock(ex_block) => {
                let mut valid_parents: Vec<H256> = vec![];
                for parent in ex_block.get_inter_parents() {
                    //check whether the parent exits
                    if let Some(_) = self.hash2blk.get(&parent) {
                        //check whether the block is pruned
                        let ver_sta = self.hash2ver_status
                            .get(&parent)
                            .unwrap()
                            .clone();
                        if let VerStatus::Pruned = ver_sta {
                            continue;
                        }
                        valid_parents.push(parent);
                    }
                }
                valid_parents
            }
            VersaBlock::ExFullBlock(ex_full_block) => {
                let mut valid_parents: Vec<H256> = vec![];
                for parent in ex_full_block.get_inter_parents() {
                    //check whether the parent exits
                    if let Some(_) = self.hash2blk.get(&parent) {
                        //check whether the block is pruned
                        let ver_sta = self.hash2ver_status
                            .get(&parent)
                            .unwrap()
                            .clone();
                        if let VerStatus::Pruned = ver_sta {
                            continue;
                        }
                        valid_parents.push(parent);
                    }
                }
                valid_parents

            }
            VersaBlock::InBlock(in_block) => {
                let possible_parents = in_block
                    .get_global_parents_map()
                    .get(&self.shard_id)
                    .unwrap()
                    .clone();
                let mut valid_parents: Vec<H256> = vec![];
                for parent in possible_parents {
                    //check whether the parent exits
                    if let Some(_) = self.hash2blk.get(&parent) {
                        //check whether the block is pruned
                        let ver_sta = self.hash2ver_status
                            .get(&parent)
                            .unwrap()
                            .clone();
                        if let VerStatus::Pruned = ver_sta {
                            continue;
                        }
                        valid_parents.push(parent)
                    }
                }
                valid_parents
            }
            VersaBlock::InFullBlock(in_full_block) => {
                let possible_parents = in_full_block
                    .get_global_parents_map()
                    .get(&self.shard_id)
                    .unwrap()
                    .clone();
                let mut valid_parents: Vec<H256> = vec![];
                for parent in possible_parents {
                    //check whether the parent exits
                    if let Some(_) = self.hash2blk.get(&parent) {
                        //check whether the block is pruned
                        let ver_sta = self.hash2ver_status
                            .get(&parent)
                            .unwrap()
                            .clone();
                        if let VerStatus::Pruned = ver_sta {
                            continue;
                        }
                        valid_parents.push(parent)
                    }
                }
                valid_parents

            }
        };
        //check whether the valid parent set contains the given parent
        if !parents.contains(parent) {
            return Err(String::from("Wrong parent"));
        }
         
        let possible_node = Node::insert(
            &mut self.root,
            parent,
            blk_hash.clone(),
            self.config.k
        );
        if let None = possible_node {
            return Err(String::from("Insertion fail"));
        }

        //if insertion succeeds, add it to the dp_map
        self.dp_map.insert((parent.clone(), blk_hash.clone()), true);

        let new_node = possible_node.unwrap();
        //update hash2node
        self.hash2node.insert(blk_hash.clone(), (*new_node).clone());
       
        //need to modify here
        if let None = self.hash2ver_status.get(&blk_hash) {
            match block {
                VersaBlock::ExBlock(_) 
                    => {
                        if shard_id == self.config.shard_id {
                            self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified);
                        } else {
                            self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Unverified);
                            self.unverified_blocks.insert((blk_hash.clone(), block.get_shard_id()), true);
                        }
                    }
                VersaBlock::InBlock(_) 
                    => {
                        if shard_id == self.config.shard_id {
                            self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified);
                        } else {
                            self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Unverified);
                            self.unverified_blocks.insert((blk_hash.clone(), block.get_shard_id()), true);
                        }
                    }
                //VersaBlock::ExBlock(_) 
                //    => self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified),
                //VersaBlock::InBlock(_) 
                //    => self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified),
                VersaBlock::ExFullBlock(_)
                    => {
                        self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified);
                    }
                VersaBlock::InFullBlock(_)
                    => {
                        self.hash2ver_status.insert(blk_hash.clone(), VerStatus::Verified);
                    }
            };
        }

        //update the longest verified chain hash 
        let (longest_verified_hash, height) = Node::get_longest_verified_fork(
            &self.root,
            &self.hash2ver_status
        ).unwrap();
        self.longest_verified_chain_hash = longest_verified_hash;
        
        //update the unverified leaves
        self.leaves = Node::get_leaves_start_from(
            &self.root,
            &self.longest_verified_chain_hash
        ).unwrap();
        //update the longest verified chain information
        //update the confirmation information
        let mut possible_confirmed_block: Option<(VersaBlock, usize)> = None;
        if height > self.verified_height {
            self.verified_height = height;
            let history = self.all_blocks_end_with_block(&longest_verified_hash).unwrap();
            let confirmed_index = match height >= self.config.k {
                true => height - self.config.k,
                false => 0,
            };
            let confirmed_hash = history[confirmed_index];
            let confirmed_block = self.get_block(&confirmed_hash).unwrap();
            possible_confirmed_block = Some((confirmed_block, confirmed_index));
        }

        //update basic information
        let _ = self.hash2blk.insert(
            blk_hash.clone(),
            block.clone()
        );

        //update the longest chain information
        if new_node.height > self.height {
            self.height = new_node.height;
            self.longest_chain_hash = new_node.val.clone();
        } else if new_node.height == self.height {
            if new_node.val < self.longest_chain_hash {
                self.longest_chain_hash = new_node.val.clone();
            }
        }
            
        
        //if the state already exits, there is no need to rewrite it
        //because the same block extended on different parents share the
        //same state
        if let None = self.states.get(&blk_hash) {
            //update related state information
            let mut state = self.states
                .get(&parent)
                .unwrap()
                .clone();

            //Exclusive block and inclusive block inherits their parent's state
            match block {
                VersaBlock::ExBlock(_) => {
                    let _ = self.states.insert(blk_hash.clone(), state);
                    return Ok(possible_confirmed_block);
                }
                VersaBlock::InBlock(_) => {
                    let _ = self.states.insert(blk_hash.clone(), state);
                    return Ok(possible_confirmed_block);
                }
                _ => {}
            };

            let txs = block.get_txs_ref().unwrap();
            let tmys = block.get_tmys().unwrap();
            (0..txs.len()).for_each(|i| {
                let tx = &txs[i];
                let tx_hash = tx.hash();
                match self.tx_map.get(&tx_hash) {
                    Some(old_locations) => {
                        let mut new_locations = old_locations.clone();
                        new_locations.push((blk_hash.clone(), i));
                        self.tx_map.insert(tx_hash, new_locations);
                    }
                    None => {
                        self.tx_map.insert(tx_hash, vec![(blk_hash.clone(), i)]);
                    }
                }
                self.update_state(tx, &mut state, &tmys);  
            });
            let _ = self.states.insert(blk_hash.clone(), state);
        }

        Ok(possible_confirmed_block)
    }

    fn update_state(&self, tx: &Transaction, state: &mut State, tmys: &HashMap<H256, Testimony>) {
        let tx_hash = tx.hash();
        match tx.flag {
            TxFlag::Empty => {}
            TxFlag::Initial => {
                //For an initial tx, it does not consume any utxos
                state.insert((tx_hash.clone(), 0), (tx.clone(), None));
            }
            TxFlag::Domestic => {
                //For an domestic tx, all inputs and outputs corresponds to the current
                //shard
                //remove all inputs from current state
                for input in tx.inputs.iter() {
                    state.remove(&(
                        input.tx_hash.clone(),
                        input.index
                    ));
                }
                //add all outputs to state
                for j in 0..tx.outputs.len() {
                    state.insert(
                        (tx_hash.clone(), j as u32),
                        (tx.clone(), None)
                    );
                }
            }
            TxFlag::Input => {
                //For an input-tx, we dont care the outputs,
                //we only remove the corresponding utxos specified by the inputs from state
                for input in tx.inputs.iter() {
                    //skip inputs not corresponding to the current shard
                    if Validator::get_shard_id(
                        &input.sender_addr,
                        self.config.shard_num
                    ) != self.config.shard_id {
                        continue;
                    }
                    state.remove(&(
                        input.tx_hash.clone(),
                        input.index
                    ));
                }
            }
            TxFlag::Output => {
                //For a valid output-tx, we dont care the inputs,
                //we only add the corresponding utxos specified by the outputs to state
                //but this state is unstable, it depends on the corresponding testimony
                for j in 0..tx.outputs.len() {
                    let output = &tx.outputs[j];
                    if Validator::get_shard_id(
                        &output.receiver_addr,
                        self.config.shard_num
                    ) != self.config.shard_id {
                        continue;
                    }
                    state.insert(
                        (tx_hash.clone(), j as u32),
                        (tx.clone(), Some(tmys.get(&tx_hash).unwrap().clone()))
                    );
                }
            }
            TxFlag::Accept => {
                //For an accept-tx, we do nothing because the "locking" and "spent" status
                //of an input utxo are the same
            }
            TxFlag::Reject => {
                //For a reject-tx, we return the "locking" utxos
                //the utxo of a reject-tx is kind of special, it is not specified by the
                //outputs but by the inputs. In other words, the corresponding utxo is
                //return by creating another utxo speficied by the inputs of a reject-tx,
                //and its validity is verified by the associated testimony which proves the
                //inclusion of the block where the corresponding output-tx locates at in
                //the longest chain
                for j in 0..tx.inputs.len() {
                    let input = &tx.inputs[j];
                    if Validator::get_shard_id(
                        &input.sender_addr,
                        self.config.shard_num
                    ) != self.config.shard_id {
                        continue;
                    }
                    //reinsert the utxo to state
                    state.insert(
                        (tx_hash.clone(), j as u32),
                        (tx.clone(), Some(tmys.get(&tx_hash).unwrap().clone()))
                    );
                }
                
            }
        }
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        self.longest_chain_hash.clone()
    }

    /// Get all blocks' hashes of the longest chain, ordered from genesis to the tip
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        Node::get_path(&self.root, &self.longest_chain_hash)
                .unwrap()
    }

    pub fn get_verify_status(&self, hash: &H256) -> Option<VerStatus> {
        match self.hash2ver_status.get(hash) {
            Some(ver_sta) => Some(ver_sta.clone()),
            None => None,
        }
    }

    //Get all blocks' hashs of the path end with specific hash
    pub fn all_blocks_end_with_block(&self, hash: &H256) -> Option<Vec<H256>> {
        Node::get_path(&self.root, hash)
    }

    // get the block from H256
    pub fn get_block(&self, hash: &H256) -> Option<VersaBlock> {
        match self.hash2blk.get(hash) {
            Some(block_ref) => {
                Some(block_ref.clone())
            }
            None => {
                None
            }
        }
    }

    //get the static states
    pub fn get_states(&self) -> HashMap<H256, State> {
        self.states.into_map()
    }


    //get the unverified leaves
    pub fn get_leaves(&self) -> Vec<H256> {
        self.leaves.clone()
    }

    pub fn get_tx_in_longest_chain(&self, tx_hash: &H256) -> Option<Transaction> {
        if let Some(locations) = self.tx_map.get(tx_hash) {
            let longest_chain_blks: Vec<H256> = self.all_blocks_in_longest_chain();
            for location in locations.iter() {
                let blk_hash = &location.0;
                let index = location.1;
                if longest_chain_blks.contains(blk_hash) {
                    let versa_blk = self.hash2blk.get(blk_hash).unwrap();
                    match versa_blk {
                        VersaBlock::ExFullBlock(exfullblock) => {
                            let blk_hash = exfullblock.hash();
                            if longest_chain_blks.contains(&blk_hash) {
                                let txs = exfullblock.get_txs_ref();
                                let tx = txs.get(index).unwrap().clone();
                                return Some(tx);
                            } 
                        }
                        VersaBlock::InFullBlock(infullblock) => {
                            let blk_hash = infullblock.hash();
                            if longest_chain_blks.contains(&blk_hash) {
                                let txs = infullblock.get_txs_ref();
                                let tx = txs.get(index).unwrap().clone();
                                return Some(tx);
                            }
                        }
                        _ => {
                            return None;
                        }
                    }                   
                } else {
                    return None;
                }
            }
            return None;
        } else {
            None
        }
    }
    

    
    pub fn get_consensus_block(&self, hash: &H256) -> Option<ConsensusBlock> {
        match self.hash2blk.get(hash) {
            Some(versa_block) => {
                match versa_block {
                    VersaBlock::ExBlock(ex_block) => Some(ex_block.get_cons_block()),
                    VersaBlock::InBlock(in_block) => Some(in_block.get_cons_block()),
                    VersaBlock::ExFullBlock(ex_full_block) 
                        => Some(ex_full_block.ex_block.get_cons_block()),
                    VersaBlock::InFullBlock(in_full_block) 
                        => Some(in_full_block.in_block.get_cons_block()),
                    _ => None
                }
            }
            None => None
        } 
    }

    pub fn is_block_confirmed(&self, hash: &H256, k: usize) -> bool {
        match Node::get_node_by_hash(&self.root, hash) {
            Some(node) => {
                node.longest_height - node.height >= k
            }
            None => {
                false
            }
        } 
    }

    pub fn is_block_in_longest_chain(&self, hash: &H256) -> bool {
        match Node::get_node_by_hash(&self.root, hash) {
            Some(node) => node.longest_height == self.height,
            None => false
        }
    }

    pub fn get_unverified_blocks(&self) -> Vec<(H256, usize)> {
        self.unverified_blocks
            .clone()
            .into_keys()
            .collect()
    }

    pub fn prune_fork(&mut self, hash: &H256) {
        match Node::prune(&mut self.root, hash) {
            Some(deleted_blks) => {
                for blk in deleted_blks {
                    self.delete_block(&blk);
                }
                //update the longest verified chain hash 
                let (longest_verified_hash, height) = Node::get_longest_verified_fork(
                    &self.root,
                    &self.hash2ver_status
                ).unwrap();
                self.longest_verified_chain_hash = longest_verified_hash;
                self.verified_height = height;

                //update the unverified leaves
                self.leaves = Node::get_leaves_start_from(
                    &self.root,
                    &self.longest_verified_chain_hash
                ).unwrap();

                //update the longest chain hash
                let (longest_hash, longest_height) = Node::get_longest_chain_hash(&self.root);
                self.height = longest_height;
                self.longest_chain_hash = longest_hash;
            }
            None => {}
        }
    }

    pub fn get_block_with_tx(&self, tx_hash: &H256) -> Option<(VersaBlock, usize)> {
        match self.tx_map.get(tx_hash) {
            Some(locations) => {
                let longest_chain_blks: Vec<H256> = self.all_blocks_in_longest_chain();
                for location in locations.iter() {
                    let blk_hash = &location.0;
                    let tx_index = location.1;
                    if longest_chain_blks.contains(blk_hash) {
                        let blk = self.hash2blk.get(blk_hash).unwrap().clone();
                        return Some((blk, tx_index));
                    } else {
                        return None;
                    }
                }
                None 
            },
            None => None,
        }
    }

    pub fn verify_block(&mut self, block_hash: &H256) 
        -> Result<Option<(VersaBlock, usize)>, String> 
    {
        if let None = self.hash2ver_status.get(block_hash) {
            return Err(String::from("Block does not exit"));
        }
        let current_status = self.hash2ver_status.get(block_hash).unwrap().clone();
        match current_status {
            VerStatus::Unverified => {
                info!("block get verified");
                self.hash2ver_status.insert(block_hash.clone(), VerStatus::Verified);
                //remove from unverified_blocks
                //let shard_id = self.hash2blk.get(&block_hash).unwrap().get_shard_id();
                //self.unverified_blocks.remove(&(block_hash.clone(), shard_id));
                self.unverified_blocks.retain(|key, value| {
                    block_hash != &key.0 
                });
                //update the longest verified chain hash 
                let (longest_verified_hash, height) = Node::get_longest_verified_fork(
                    &self.root,
                    &self.hash2ver_status
                ).unwrap();
                self.longest_verified_chain_hash = longest_verified_hash;
                let mut possible_confirmed_block: Option<(VersaBlock, usize)> = None;
                if height > self.verified_height {
                    let history = self.all_blocks_end_with_block(&longest_verified_hash).unwrap();
                    let confirmed_index = match height >= self.config.k {
                        true => height - self.config.k,
                        false => 0,
                    };
                    let confirmed_hash = history[confirmed_index];
                    let confirmed_block = self.get_block(&confirmed_hash).unwrap();
                    let block = self.get_block(&block_hash).unwrap();
                    
                    possible_confirmed_block = Some((confirmed_block, confirmed_index));
                }
                self.verified_height = height;

                //update the unverified leaves
                self.leaves = Node::get_leaves_start_from(
                    &self.root,
                    &self.longest_verified_chain_hash
                ).unwrap();
                return Ok(possible_confirmed_block);
            }
            _ => return Err(String::from("the status is not unverified")),
        }
    }

    pub fn get_block_height(&self, block_hash: &H256) -> Option<usize> {
        match self.hash2node.get(block_hash) {
            Some(node) => {
                Some(node.height)
            }
            None => None,
        }
    }

    pub fn get_all_txs_in_longest_chain(&self) -> Vec<Transaction> {
        let mut txs: Vec<Transaction> = vec![];
        let history = self.all_blocks_in_longest_chain();
        for block_hash in history {
            let block = self.hash2blk.get(&block_hash).unwrap();
            match block {
                VersaBlock::ExFullBlock(ex_full_block) => {
                    let curr_txs = ex_full_block.get_txs();
                    txs.extend(curr_txs);
                }
                VersaBlock::InFullBlock(in_full_block) => {
                    let curr_txs = in_full_block.get_txs();
                    txs.extend(curr_txs);
                }
                _ => {}
            } 
        }
        txs
    }

    pub fn log_to_file(&self) -> Result<(), Error> {
        let main_chain_blocks = self.all_blocks_in_longest_chain();
        let main_chain_block_num = main_chain_blocks.len() as f64;
        let nodes = Node::pre_traverse(&self.root);
        let total_block_num = nodes.len() as f64;
        let forking_rate = main_chain_block_num / total_block_num;

        let path = format!("./log/exper_{}/{}.txt", self.config.exper_number, self.config.node_id);
        let mut output = File::create(path)?;
        let mut ex_block_num = 0;
        let mut in_block_num = 0;
        for block in main_chain_blocks.iter() {
            let versa_block = self.hash2blk.get(block).unwrap();
            if let VersaBlock::InBlock(_) = versa_block.clone() {
                in_block_num += 1;
                continue;
            }
            ex_block_num += 1;
            let timestamp = versa_block.get_timestamp();
            let datetime: DateTime<Local> = timestamp.into();
            let formatted_datetime = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
            write!(output, "block {:?} created at {}\n", versa_block.hash(), formatted_datetime);
        } 
        write!(
            output, 
            "forking_rate: {:.2} total_block_num: {} main_chain_block_num: {}\n", 
            forking_rate, total_block_num, main_chain_block_num
        )?;
        write!(
            output, 
            "ex_block_num: {:.2} in_block_num: {}\n",
            ex_block_num, in_block_num
        )?;

        for i in 0..main_chain_blocks.len()-self.config.k {
            let block = main_chain_blocks.get(i).unwrap();
            let versa_block = self.hash2blk.get(block).unwrap();
            if let VersaBlock::InBlock(_) = versa_block.clone() {
                continue;
            }
            if let VersaBlock::ExBlock(_) = versa_block.clone() {
                continue;
            }
            let package_time = versa_block.get_timestamp();
            let package_time: DateTime<Local> = package_time.into();
            let package_time = package_time.format("%Y-%m-%d %H:%M:%S").to_string();

            //get confirmation time
            let block_after_k = main_chain_blocks.get(i + self.config.k).unwrap();
            let versa_block_after_k = self.hash2blk.get(block_after_k).unwrap();
            let confirmed_time = versa_block_after_k.get_timestamp();
            let confirmed_time: DateTime<Local> = confirmed_time.into();
            let confirmed_time = confirmed_time.format("%Y-%m-%d %H:%M:%S").to_string();

            let txs = versa_block.get_txs().unwrap();
            for tx in txs {
                let tx_hash = tx.hash();
                match tx.flag {
                    TxFlag::Initial => {}
                    TxFlag::Empty => {}
                    TxFlag::Domestic => {
                        write!(output, "domestic tx {:?} packaged at {} confirmed at {}\n", tx_hash, package_time, confirmed_time);
                    }
                    TxFlag::Input => {
                        write!(output, "input tx {:?} packaged at {} confirmed at {} ", tx_hash, package_time, confirmed_time);
                        //write!(output, "input shards: ");
                        //for input_tx in tx.inputs {
                        //    let shard_id = Validator::get_shard_id(&input_tx.sender_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        //write!(output, "output shards: ");
                        //for output_tx in tx.outputs {
                        //    let shard_id = Validator::get_shard_id(&output_tx.receiver_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        write!(output, "\n");
                    }
                    TxFlag::Output => {
                        write!(output, "output tx {:?} packaged at {} confirmed at {} ", tx_hash, package_time, confirmed_time)?;
                        //write!(output, "input shards: ")?;
                        //for input_tx in &tx.inputs {
                        //    let shard_id = Validator::get_shard_id(&input_tx.sender_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        //write!(output, "output shards: ");
                        //for output_tx in &tx.outputs {
                        //    let shard_id = Validator::get_shard_id(&output_tx.receiver_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        let mut corr_input_tx = tx.clone();
                        corr_input_tx.flag = TxFlag::Input;
                        write!(output, "corresponding input tx is {:?}\n", corr_input_tx.hash())?;
                    } 
                    TxFlag::Accept => {
                        write!(output, "accept tx {:?} packaged at {} confirmed at {} ", tx_hash, package_time, confirmed_time);
                        //write!(output, "input shards: ");
                        //for input_tx in &tx.inputs {
                        //    let shard_id = Validator::get_shard_id(&input_tx.sender_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        //write!(output, "output shards: ");
                        //for output_tx in &tx.outputs {
                        //    let shard_id = Validator::get_shard_id(&output_tx.receiver_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        let mut corr_output_tx = tx.clone();
                        corr_output_tx.flag = TxFlag::Output;
                        write!(output, "corresponding output tx is {:?}\n", corr_output_tx.hash())?;
                    }
                    TxFlag::Reject => {
                        write!(output, "reject tx {:?} packaged at {} confirmed at {} ", tx_hash, package_time, confirmed_time)?;
                        //write!(output, "input shards: ")?;
                        //for input_tx in &tx.inputs {
                        //    let shard_id = Validator::get_shard_id(&input_tx.sender_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        //write!(output, "output shards: ");
                        //for output_tx in &tx.outputs {
                        //    let shard_id = Validator::get_shard_id(&output_tx.receiver_addr, self.config.shard_num);
                        //    write!(output, "{} ", shard_id)?;
                        //}
                        let mut corr_output_tx = tx.clone();
                        corr_output_tx.flag = TxFlag::Output;
                        write!(output, "corresponding output tx is {:?}\n", corr_output_tx.hash())?;
                    }
                }
            }
        }


        Ok(())
    }


    pub fn all_blocks_in_longest_chain_with_time(&self) -> Vec<(H256, String)> {
        let mut res: Vec<(H256, String)> = vec![];
        let main_chain_blocks = self.all_blocks_in_longest_chain();
        
        for block in main_chain_blocks.iter() {
            let versa_block = self.hash2blk.get(block).unwrap();

            if let VersaBlock::InBlock(_) = versa_block.clone() {
                continue;
            }
            let timestamp = versa_block.get_timestamp();
            let datetime: DateTime<Local> = timestamp.into();
            let formatted_datetime = datetime.format("%Y-%m-%d %H:%M:%S").to_string();

            res.push((block.clone(), formatted_datetime));
        }
        res
    }

    pub fn get_forking_rate(&self) -> f64 {
        let main_chain_blocks = self.all_blocks_in_longest_chain();
        let main_chain_block_num = main_chain_blocks.len() as f64;
        let nodes = Node::pre_traverse(&self.root);
        let total_block_num = nodes.len() as f64;
        let forking_rate = main_chain_block_num / total_block_num;
        forking_rate
    }

}

