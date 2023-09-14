use crate::btree::extract_key;

use super::*;
use anyhow::Ok;
use howlong::Duration;
use log::info;
use rand_core::block;
use serde::{Serialize, Deserialize};
use std::{collections::HashMap, ops::Bound};
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueryParam{
    #[serde(rename = "query_attribute")]
    pub key: Vec<KeyType>,
    #[serde(rename = "range")]
    pub value: Vec<[Option<KeyType>; 2]>,
    pub bloom_filter: bool,
    pub intra_index: bool,
}

/// res_txs for block query transactions, and boundary check.
/// res_sigs for aggregate_sinatures of each block
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverallResult{
    #[serde(rename = "result")]
    pub res_txs: ResultTxs,
    pub res_vos: ResultVos,
    pub query_param: QueryParam,
    pub query_time_ms: u64,
    pub use_inter_index: bool,
    pub use_intra_index: bool,
    pub vo_size:usize,
}

impl OverallResult {
    pub async fn verify(
        &self,
        chain: &impl LightNodeInterface
    )
    -> Result<(VerifyResult, Duration)>{
        let cpu_timer = howlong::ProcessCPUTimer::new();
        let timer = howlong::HighResolutionTimer::new();
        // let res = self.inner_verify(chain).await?;
        let res = self.aggre_verify(chain).await?;
        let time = timer.elapsed();
        info!("verify used time {}",cpu_timer.elapsed());
        
        Ok((res, time))
    }

    // async fn inner_verify(&self, chain: &impl LightNodeInterface) -> Result<VerifyResult>{
    //     let mut result = VerifyResult::default();
    //     let mut signature: Option<Signature>;
    //     let mut block_header: BlockHeader;
    //     let ctx = signing_context(b"");
    //     for (id, txs) in self.res_txs.0.iter(){
    //         signature = self.res_sigs.0.get(id).unwrap().to_owned();
    //         block_header = chain.lightnode_read_block_header(id.to_owned()).await?;
    //         if signature.eq(&Option::None){
    //             //this means no satisfying txs in block(id)
    //             //and the Vec stores boundary conditions 
    //             continue;
    //         }
    //         let mut aggre_string_txs: String = String::from("");
    //         let public_key = PublicKey::recover(block_header.public_key);
    //         for tx in txs {
    //             aggre_string_txs += &serde_json::to_string(&tx).unwrap();
    //         }
    //         //verify failed, malicious actions exist
    //         if public_key.verify(ctx.bytes(aggre_string_txs.as_bytes()), &signature.unwrap()).is_err() {
    //             result.add(InvalidReason::InvalidSignature);
    //         }
    //     }

    //     Ok(result)
    // }

    async fn aggre_verify(&self, chain: &impl LightNodeInterface) -> Result<VerifyResult>{
        let mut result = VerifyResult::default();
        

        let mut sign_ctx: Vec<String> = Vec::new(); 
        let mut aggre_string_txs: String = String::from("");
        let mut public_keys: Vec<PublicKey> = Vec::new();
        let mut signature:Option<Signature>;
        let mut block_indexs:Vec<IdType> = Vec::from_iter(self.res_sigs.0.iter().map(|(x,_y)|x.to_owned()));
        block_indexs.sort();
        for index in block_indexs.iter(){
            signature = self.res_sigs.0.get(index).unwrap().to_owned();
            if signature.ne(&None) {
                for tx in self.res_txs.0.get(index).unwrap().iter() {
                    aggre_string_txs += &serde_json::to_string(tx).unwrap();
                }
                sign_ctx.push(aggre_string_txs.clone());
                public_keys.push(
                    PublicKey::recover(
                        chain.lightnode_read_block_header(*index)
                        .await
                        .unwrap()
                        .public_key
                    )
                );
                aggre_string_txs.clear();
            } else {
                // boundary txs add to signature
                for tx in self.res_txs.0.get(index).unwrap().iter() {
                    aggre_string_txs += &(String::from(tx.block_id.to_string())
                    + &String::from(tx.key.clone())
                    + &String::from(tx.value.to_string()));
                    sign_ctx.push(aggre_string_txs.clone());
                    public_keys.push(
                        PublicKey::recover(
                            chain.lightnode_read_block_header(*index)
                            .await
                            .unwrap()
                            .public_key
                        )
                    );
                    aggre_string_txs.clear();
                }
            }
        }
        let ctx = signing_context(b"");
        let transcripts = sign_ctx.iter().map(|m| ctx.bytes(m.as_bytes()));
        if self.aggre_sign.as_ref().unwrap().verify(transcripts, &sign_ctx[..], &public_keys[..], false).is_err() {
            result.add(InvalidReason::InvalidSignature);
        }
        Ok(result)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultTxs(pub Vec<BlockTxs>);

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTxs {
    block_id: IdType,
    Txs:HashMap<IdType, Transaction>
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultVos(pub HashMap<IdType, Option<Signature>>);

impl ResultTxs{
    pub fn new() -> Self{
        Self(Vec::new())
    }
}

impl ResultVos{
    pub fn new() -> Self{
        Self(HashMap::new())
    }
}
// #[derive(Debug, Default, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
// pub struct TimeRange([Option<TsType>; 2]);
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryRequest{
    pub key:KeyType,
    pub value: [Option<KeyType>; 2],
}

pub fn extract_request(q_param: &QueryParam)-> Result<Vec<QueryRequest>>{
    let mut requests=Vec::new();
    for x in 0..q_param.key.len(){
        let request= QueryRequest{
            key: q_param.key[x],
            value: q_param.value[x],
        };
        requests.push(request);
    }
    Ok(requests) 
}

pub fn historical_query(q_param: &QueryParam, chain: &impl ReadInterface) 
 -> Result<OverallResult>{
    info!("process query {:?}", q_param);
    if q_param.key.len()!=q_param.value.len(){
        panic!("key's len is not equal to value's len");
    }
    let mut param = chain.get_parameter()?;
    param.intra_index = q_param.intra_index;
    param.inter_index = q_param.bloom_filter;

    let cpu_timer = howlong::ProcessCPUTimer::new();
    let timer = howlong::HighResolutionTimer::new();
    let mut res_txs = ResultTxs::new();
    let mut res_vos = ResultVos::new();

    let mut result = OverallResult {
        res_txs: res_txs.clone(),
        res_vos: res_vos.clone(),
        query_param: q_param.clone(),
        query_time_ms: 0,
        use_inter_index: param.inter_index,
        use_intra_index: param.intra_index,
        vo_size:0
    };
    let mut block_header: Vec<BlockHeader> = Vec::new();
    let mut block_data: Vec<BlockData> = Vec::new();

    //query block_header & block_data within the query range of timestamp
    if q_param.key.contains(&"timestamp".to_string()) {
        query_chain_inter_index(&q_param, chain)?;
    } else {
        query_chain_no_inter_index(&q_param, chain)?;
    }
    //query inside block to check if consist key
    let mut vo_size=0;
    result.vo_size=vo_size;
    info!("used time: {:?}", cpu_timer.elapsed());
    info!("vo_size: {:?}", vo_size);
    Ok(result)
}

/// return BlockData & BlockHeader falls in the timestamp range
fn query_chain_inter_index(
    q_param: &QueryParam,
    chain: &impl ReadInterface,
) -> Result<(ResultTxs)>{
    info!("query using inter_index");
    let mut res_txs = ResultTxs::new();
    let param = chain.get_parameter()?;
    let inter_indexs = chain.read_inter_indexs()?;
    let index_timestamps = inter_indexs.iter().map(|x| x.start_timestamp.to_owned() as TsType).collect::<Vec<TsType>>();
    let requests=extract_request(q_param)?;
    let mut timestamp_range;
    for request in &requests{
        if request.key=="timestamp".to_string(){
            timestamp_range=request.value;
        }
    }
    let left_timestamp = timestamp_range[0].as_ref().and_then(|s| s.parse::<u64>().ok()).unwrap();
    let right_timestamp = timestamp_range[1].as_ref().and_then(|s| s.parse::<u64>().ok()).unwrap();
    // use learned index with err
    let start_inter_index = chain.read_inter_index(variant_binary_search(&index_timestamps[..], left_timestamp))?;
    let end_inter_index = chain.read_inter_index(variant_binary_search(&index_timestamps[..], right_timestamp))?;
    let mut start_id = (start_inter_index.regression_a * left_timestamp as FloatType + start_inter_index.regression_b - param.error_bounds as FloatType) as IdType;
    let mut end_id = (end_inter_index.regression_a * right_timestamp as FloatType + end_inter_index.regression_b + param.error_bounds as FloatType) as IdType;
    // do not exceed block_index boundary
    start_id = start_id.max(param.start_block_id);
    end_id = end_id.min(param.start_block_id + param.block_count - 1);
    info!("start_id {}, end_id {}",start_id, end_id);
    // eliminate err_bounds
    let mut index = end_id;
    while index >= start_id{
        let block_header = chain.read_block_header(index)?;
        if block_header.time_stamp >= left_timestamp
        && block_header.time_stamp <= right_timestamp{
            if q_param.bloom_filter
            && !judge_contain_key(requests, block_header.BMT_root) {
                index-=1;
                continue;
            }
            let block_txs=query_in_block(requests,index,chain)?;
            let block_res = BlockTxs {
                block_id:index,
                Txs: block_txs,
            };
            res_txs.0.push(block_res);
        }
        index -= 1;
    }

    
    Ok((res_txs))
}
fn judge_contain_key(requests: Vec<QueryRequest>, bf: SeededBloomFilter) -> bool {
    for request in &requests {
        if request.key == "timestamp".to_string() {
            continue;
        }

        if request.key == "address" {
            if bf.contains(&request.value[0]) {
                return true
            }
        }else{
             let (left, right) = match (request.value[0].as_ref().and_then(|s| s.parse::<u64>().ok()), request.value[1].as_ref().and_then(|s| s.parse::<u64>().ok())) {
                (Some(l), Some(r)) => (l, r),
                 _ => panic!("inexistence of left bound or right bound from judge_contain_key"), 
                 };

            for i in left..=right { // 遍历左、右边界间的所有值
                 if bf.contains(&i) {
                  return true; // 如果存在布隆过滤器中的值，则返回 true
              }
           }
        }
    }
    false
}

fn query_in_block(
    requests: Vec<QueryRequest>,
    block_id: IdType,
    chain: &impl ReadInterface,
) -> Result<(HashMap<IdType,Transaction>)> {
    let block_data = chain.read_block_data(block_id)?;
    let intraindex=chain.read_intra_index(block_id)?;
    let key_to_index: HashMap<&str, usize> = [
    ("timestamp", 0),
    ("id", 1),
    ("values", 2),
    ("address", 3),].iter().cloned().collect();
    let mut res:HashMap<IdType,Transaction> = HashMap::new();
    for request in &requests{
        if let Some(&index_key) = key_to_index.get(request.key.as_str()) {
            if index_key == 0 {
                continue;
            }
            match intraindex.index.get(&index_key) {
                Some(btree) => query_with_intra_index(&mut res,btree, request.value),
                None => query_no_intra_index(&mut res,request.key,block_data,request.value),
            };
        } else {
            panic!("key is error!");
        }
    }
    Ok((res))
}


fn query_chain_no_inter_index(
    q_param: &QueryParam,
    chain: &impl ReadInterface,
) -> Result<(ResultTxs)>{
    let requests=extract_request(q_param)?;
    let mut res_txs = ResultTxs::new();
    let start_index = chain.get_parameter()?.start_block_id;
    let mut block_index = start_index + chain.get_parameter()?.block_count.clone() - 1;
    while block_index >= start_index as u64 {
        let block_header = chain.read_block_header(block_index)?;
            if q_param.bloom_filter
            && !judge_contain_key(requests, block_header.BMT_root) {
                block_index-=1;
                continue;
            }
            let block_txs=query_in_block(requests,block_index,chain)?;
            let block_res = BlockTxs {
                block_id:block_index,
                Txs: block_txs,
            };
            res_txs.0.push(block_res);
            block_index -= 1;
    }

    Ok(res_txs)
}

fn query_with_intra_index(
    res : &mut HashMap<IdType,Transaction>,
    btree: &BTreeEnum,
    values: [Option<KeyType>; 2],
) -> Result<()> {
    if let BTreeEnum::U64(btree_map) = btree {
        let start = values[0].as_ref().and_then(|s| s.parse::<u64>().ok());
        let end = values[1].as_ref().and_then(|s| s.parse::<u64>().ok());
        let start_bound = start.map_or(Bound::Unbounded, Bound::Included);
        let end_bound = end.map_or(Bound::Unbounded, Bound::Excluded);
        for (_, v) in btree_map.range((start_bound, end_bound)) {
            res.insert(v.id, *v);
        }
    } else if let BTreeEnum::String(btree_map) = btree {
        let start = values[0].as_ref();
        let end = values[1].as_ref();
        let start_bound = start.map_or(Bound::Unbounded, |s| Bound::Included(*s));
        let end_bound = end.map_or(Bound::Unbounded, |s| Bound::Excluded(*s));
        for (k, v) in btree_map.range((start_bound, end_bound)) {
            res.insert(v.id, *v);
        }
    } else {
        panic!("BTreeEnum type mismatch");
    }
    
    Ok(())
}

fn query_no_intra_index (   
    res : &mut HashMap<IdType,Transaction>,
    attribute: KeyType,
    block_data: BlockData,
    values: [Option<KeyType>; 2],
)->Result<()>{
    if attribute == "address".to_string(){
        for x in 0..block_data.txs.len(){
            if block_data.txs[x].value.address <= values[1].unwrap() && block_data.txs[x].value.address >= values[0].unwrap() {
                // if !res.contains_key(&block_data.txs[x].id) {
                    res.insert(block_data.txs[x].id, block_data.txs[x]);
            }
        }
    }else{
        let start = values[0].as_ref().and_then(|s| s.parse::<u64>().ok()).unwrap();
        let end = values[1].as_ref().and_then(|s| s.parse::<u64>().ok()).unwrap();
        match attribute.as_str() {
            "id" => {for x in 0..block_data.txs.len(){
                if block_data.txs[x].id >=start && block_data.txs[x].id<=end {
                    // if !res.contains_key(&block_data.txs[x].id) {
                        res.insert(block_data.txs[x].id, block_data.txs[x]);
                }
            }}
            "value" => {for x in 0..block_data.txs.len(){
                res.insert(block_data.txs[x].id, block_data.txs[x]);
            }
            }
            _ => panic!("attrubute error from query_no_intra_index!")
        }
    }
    Ok(())
}