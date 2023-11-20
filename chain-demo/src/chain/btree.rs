use std::{collections::{BTreeMap,HashMap, HashSet}, time::Instant};
use anyhow::Ok;
use serde_json::{to_string, value::Index};

use super::*;

// enum Attribute{
//     id,
//     block_id,
//     key,
//     trans_in,
//     trans_value,
//     time_stamp,
// }
pub fn index_build_block(attribute: &[String], height: IdType, chain: &mut (impl ReadInterface + WriteInterface), configs_map:&mut IndexConfigs_map) -> Result<IntraIndex> {
    let mut index_map = HashMap::new();
    let mut index_cost = Vec::new();
    for k in attribute.iter() {
            let (b_tree, cost) = create_btree_with_evaluation(k, chain, height)?;
            let config=IndexConfig::from_b_tree(cost, &b_tree, height, k)?;
            index_map.insert(k.clone(), b_tree);
            index_cost.push(cost);
            configs_map.add_config(k.clone(), config);
    }

    let intra_btree = IntraIndex {
        blockId: height,
        index: index_map,
    };

    Ok((intra_btree))
}


pub fn index_build(
    attribute: &[String],
    heights: &[IdType],
    chain: &mut (impl ReadInterface + WriteInterface),
) -> Result<()> {
    if heights.is_empty() {
        return Ok(());
    }

    for &height in heights.iter() {
        let mut intra_index = Vec::new();
        let mut index_map = HashMap::new();
        for attr in attribute.iter() {
                let b_tree = create_btree(attr, chain, height)?; 
                index_map.insert(attr.clone(), b_tree.clone());
                intra_index.push(b_tree.clone());
        }
        let intra_btree = IntraIndex {
            blockId: height,
            index: index_map,
        };
        chain.write_intra_index(intra_btree)?;
    }

    Ok(())
}

pub fn update_indices_based_on_config(
    configs: &[IndexConfig],
    chain: &mut (impl ReadInterface + WriteInterface)
) -> Result<()> {
    let mut needed_indices = HashMap::new();

    // 确定每个 block_height 需要的索引
    for config in configs {
        needed_indices
            .entry(config.block_height)
            .or_insert_with(Vec::new)
            .push(config.attribute.clone());
    }

    // 遍历每个 block_height，对索引进行更新
    for (&block_height, required_attrs) in &needed_indices {
        let mut existing_intra_index = chain.read_intra_index(block_height)?;

        // 确定需要创建的索引
        let to_create = required_attrs
            .iter()
            .filter(|attr| !existing_intra_index.index.contains_key(*attr))
            .cloned()
            .collect::<Vec<_>>();

        // 确定需要删除的索引
        let to_delete: Vec<String> = existing_intra_index.index.keys()
            .filter(|existing_attr| !required_attrs.contains(existing_attr))
            .cloned()
            .collect();

        // 删除不再需要的索引
        for attr in to_delete {
            existing_intra_index.index.remove(&attr);
        }
        // 创建缺失的索引
        for attr in &to_create {
            let b_tree = create_btree(attr, chain, block_height)?;
            existing_intra_index.index.insert(attr.clone(), b_tree);
        }

        // 更新修改后的索引
        chain.write_intra_index(existing_intra_index)?;
    }

    Ok(())
}

// create btree for a certain attribute
pub fn create_btree(
    attribute: &String,
    chain: &impl ReadInterface,
    height: IdType,
) -> Result<BTreeEnum> {  // 注意这里的返回类型
    let block_data = chain.read_block_data(height)?;
    let extracted_data = extract_key(attribute, &block_data);
    
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
    attribute: &String,
    chain: &impl ReadInterface,
    height: IdType,
) -> Result<(BTreeEnum, f64)>
{
    let start = Instant::now();
    // 执行数据库读取操作
    let block_data = chain.read_block_data(height)?;
    let duration = start.elapsed().as_secs_f64() * 1_000.0;
    let (extracted_data,selectivity) = extract_key_cost(attribute, &block_data);
    let query_cost = QueryCost {
        n_pages: 0.0,        
        c_page: 0.0,          
        c_tuple: 0.0001,         // 示例值
        n_total_tuple: block_data.tx_ids.len() as f64, 
    };

    let lambda = 0.0;  // 初始 
    let sigma = selectivity;   // 初始值为选择率

    // 计算 index_cost
    let index_cost = (query_cost.cost(lambda, sigma)+ duration) ;

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

pub fn extract_key_cost(attribute: &String, block_data: &BlockData) -> (ExtractedData,f64) {
    let total_count = block_data.txs.len() as f64;
    match attribute.as_str() {
        "id" => {
            let unique_ids: HashSet<_> = block_data.tx_ids.iter().collect();
            let selectivity = unique_ids.len() as f64 / total_count;
            (ExtractedData::TxIds(block_data.tx_ids.clone()),selectivity)},
        "address" => {
            let addresses_data:Vec<String>= block_data.txs.iter().map(|x| x.value.address.clone()).collect();
            let unique_addresses: HashSet<_> = addresses_data.iter().collect();
            let selectivity = unique_addresses.len() as f64 / total_count;
            (ExtractedData::Addresses(addresses_data),selectivity)
        },
        "value" => {
            let trans_values_data:Vec<u64> = block_data.txs.iter().map(|x| x.value.trans_value).collect();
            let unique_trans_values: HashSet<_> = trans_values_data.iter().collect();
            let selectivity = unique_trans_values.len() as f64 / total_count;
            (ExtractedData::TransValues(trans_values_data),selectivity)
        },
        _ => panic!("attribute error!"),
    }
}

pub fn extract_key(attribute: &String, block_data: &BlockData) -> ExtractedData {
    match attribute.as_str() {
        "id" => ExtractedData::TxIds(block_data.tx_ids.clone()),
        "address" => {
            let addresses_data= block_data.txs.iter().map(|x| x.value.address.clone()).collect();
            ExtractedData::Addresses(addresses_data)
        },
        "value" => {
            let trans_values_data = block_data.txs.iter().map(|x| x.value.trans_value).collect();
            ExtractedData::TransValues(trans_values_data)
        },
        _ => panic!("attribute error!"),
    }
}


