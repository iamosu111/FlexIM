use std::collections::{BTreeMap,HashMap};
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
pub fn index_build_block(attribute: &[usize], height: IdType, chain: &mut (impl ReadInterface + WriteInterface)) -> Result<(IntraIndex, Vec<u64>)> {
    let mut index_map = HashMap::new();
    let mut index_cost = Vec::new();

    for k in 0..attribute.len() {
        if attribute[k]==1 {
            let (b_tree, cost) = create_btree_with_evaluation(k, chain, height)?;
            index_map.insert(k, b_tree);
            index_cost.push(cost);
        }
    }

    let intra_btree = IntraIndex {
        blockId: height,
        index: index_map,
    };

    Ok((intra_btree, index_cost))
}


pub fn index_build(
    attribute: &[usize],
    heights: &[IdType],
    chain: &mut (impl ReadInterface + WriteInterface),
) -> Result<()> {
    if heights.is_empty() {
        return Ok(());
    }

    for &height in heights.iter() {
        let mut intra_index = Vec::new();
        let mut index_map = HashMap::new();
        for attr in 0..attribute.len() {
            if attribute[attr] == 1 {
                let b_tree = create_btree(attr, chain, height)?; 
                index_map.insert(attr, b_tree);
                intra_index.push(b_tree);
            }
        }
        let intra_btree = IntraIndex {
            blockId: height,
            index: index_map,
        };
        chain.write_intra_index(intra_btree)?;
    }

    Ok(())
}

// create btree for a certain attribute
pub fn create_btree(
    attribute: usize,
    chain: &impl ReadInterface,
    height: IdType,
) -> Result<BTreeEnum> {  // 注意这里的返回类型
    let block_data = chain.read_block_data(height)?;
    let extracted_data = extract_key(attribute, block_data);
    
    match extracted_data {
        ExtractedData::TxIds(tx_ids_data) => {
            let btree = build_tree(&tx_ids_data, &block_data.txs);
            Ok(BTreeEnum::U64(btree))
        },
        ExtractedData::Addresses(addresses_data) => {
            let btree = build_tree(&addresses_data, &block_data.txs);
            Ok(BTreeEnum::String(btree))
        },
        ExtractedData::TransValues(trans_values_data) => {
            let btree = build_tree(&trans_values_data, &block_data.txs);
            Ok(BTreeEnum::U64(btree))
        },
        _ => panic!("attribute error!"),
    }
}

pub fn create_btree_with_evaluation(
    attribute: usize,
    chain: &impl ReadInterface,
    height: IdType,
) -> Result<(BTreeEnum, u64)>
{
    let block_data = chain.read_block_data(height)?;
    let extracted_data = extract_key(attribute, block_data);
    let mut index_cost: u64 = 0;

    match extracted_data {
        ExtractedData::TxIds(tx_ids_data) => {
            let btree = build_tree(&tx_ids_data, &block_data.txs);
            Ok((BTreeEnum::U64(btree), index_cost))
        }
        ExtractedData::Addresses(addresses_data) => {
            let btree = build_tree(&addresses_data, &block_data.txs);
            Ok((BTreeEnum::String(btree), index_cost))
        }
        ExtractedData::TransValues(trans_values_data) => {
            let btree = build_tree(&trans_values_data, &block_data.txs);
            Ok((BTreeEnum::U64(btree), index_cost))
        }
        _ => panic!("attribute error!"),
    }
}



pub fn build_tree<K>(key: &Vec<K>, value: &Vec<Transaction>)-> BTreeMap<K,Transaction>
where 
     K:Ord+Clone,
{
    let mut btree= BTreeMap::new();
    for i in 0..value.len(){
        btree.entry(key[i].clone()).or_insert(value[i].clone());
    }
    btree
}

pub fn extract_key(attribute: usize, block_data: BlockData) -> ExtractedData {
    match attribute {
        0 => ExtractedData::TxIds(block_data.tx_ids),
        1 => {
            let addresses_data= block_data.txs.iter().map(|x| x.value.address).collect();
            ExtractedData::Addresses(addresses_data)
        },
        2 => {
            let trans_values_data = block_data.txs.iter().map(|x| x.value.trans_value).collect();
            ExtractedData::TransValues(trans_values_data)
        },
        _ => panic!("attribute error!"),
    }
}




