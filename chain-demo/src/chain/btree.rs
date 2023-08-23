use std::collections::{BTreeMap};
use anyhow::Ok;

use super::*;

// enum Attribute{
//     id,
//     block_id,
//     key,
//     trans_in,
//     trans_value,
//     time_stamp,
// }
pub fn index_build_block (attribute: &[usize], height: IdType,chain: &mut (impl ReadInterface + WriteInterface))-> Result<(IntraIndex,Vec<u64>)> {
    let mut intra_index= Vec::new();
    let mut index_cost=Vec::new();
    let mut attributes=Vec::new();
        for k in 0..attribute.len(){
            if attribute[k]==1 {
                attributes.push(k);
                let (b_tree,cost)=create_btree_with_evaluation(k,chain,height)?;
                intra_index.push(b_tree);
                index_cost.push(cost);
            }
    }
    let intra_btree = IntraIndex{
        blockId: height,
        attribute:attributes,
        intraindex: intra_index,
    };
    Ok((intra_btree,index_cost))
}
pub fn index_build(attribute: &[usize], heights: &[IdType],chain: &mut (impl ReadInterface + WriteInterface))-> Result<()> {
    if heights.len()==0 {
        Ok(());
    }
    
    for x in 0..heights.len() {
        let mut intra_index= Vec::new();
        let mut attributes=Vec::new();
        for k in 0..attribute.len(){
            if attribute[k]==1 {
                attributes.push(k);
                let b_tree=create_btree(k,chain,heights[x])?;
                intra_index.push(b_tree);
            }
        }
        let intra_btree = IntraIndex{
            blockId: heights[x],
            attribute:attributes,
            intraindex: intra_index,
        };
        chain.write_intra_index(intra_btree)?;
    }

    Ok(())
}

// create btree for a certain attribute
pub fn create_btree (attribute: usize, chain: &impl ReadInterface, height: IdType) -> Result<BTreeMap<u64,Transaction>>{
    let mut btree=BTreeMap::new();
    let block_data=chain.read_block_data(height)?;
    let mut key_list=extract_key(attribute, block_data);

    if key_list.len() != block_data.tx_ids.len() {
        panic!("key_list's len is not equal to block_data's len")
    } 
    for i in 0..block_data.txs.len() {
        btree.entry(key_list[i].clone()).or_insert(block_data.txs[i].clone());
    }

   Ok(btree) 
}

pub fn create_btree_with_evaluation (attribute: usize, chain: &impl ReadInterface, height: IdType) -> Result<(BTreeMap<u64,Transaction>,u64)>{
    let mut btree=BTreeMap::new();
    let block_data=chain.read_block_data(height)?;
    let mut key_list=extract_key(attribute, block_data);
    let mut index_cost:u64=0;
    //todo: index_evaluation
    if key_list.len() != block_data.tx_ids.len() {
        panic!("key_list's len is not equal to block_data's len")
    } 
    for i in 0..key_list.len() {
        btree.entry(key_list[i].clone()).or_insert(block_data.txs[i].clone());
    }

   Ok((btree,index_cost)) 
}

pub fn extract_key<P> (attribute: usize, block_data: BlockData)-> Vec<P>{
    let mut data=Vec::new();
    match attribute {
        0 => data=block_data.tx_ids,
        1 => data=block_data.txs.iter().map(|x|x.value.address).collect(),
        2 => data=block_data.txs.iter().map(|x|x.value.trans_value).collect(),
        _ => panic!("attribute error!"),
    }
    data
}