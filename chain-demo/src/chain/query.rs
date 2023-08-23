use crate::btree::extract_key;

use super::*;
use anyhow::Ok;
use howlong::Duration;
use log::info;
use rand_core::block;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueryParam{
    #[serde(rename = "query_attribute")]
    pub key: Vec<KeyType>,
    #[serde(rename = "range")]
    pub value: Vec<[Option<TsType>; 2]>,
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
pub struct ResultTxs(pub HashMap<IdType, Vec<Transaction>>);

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultVos(pub HashMap<IdType, Option<Signature>>);

impl ResultTxs{
    pub fn new() -> Self{
        Self(HashMap::new())
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
    pub value: [Option<TsType>; 2],
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
        query_chain_inter_index(&q_param, &mut block_header, &mut block_data, chain)?;
    } else {
        query_chain_no_inter_index(&q_param, &mut block_header, &mut block_data, chain)?;
    }
    info!("block_headers len : {:#?}",block_header.len());
    info!("block_datas len : {:#?}",block_data.len());
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
    block_headers: &mut Vec<BlockHeader>,
    block_datas: &mut Vec<BlockData>,
    chain: &impl ReadInterface,
) -> Result<()>{
    info!("query using inter_index");
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
    let left_timestamp = timestamp_range[0].unwrap();
    let right_timestamp = timestamp_range[1].unwrap();
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
            block_headers.push(block_header.to_owned());
            if q_param.bloom_filter
            && !epoch.agg_bloom_filter.contains(&key) {
                index-=1
                continue;
            }
            query_in_block(requests,index,chain);
        }
        index -= 1;
    }

    
    Ok(())
}

fn query_in_block(
    requests: Vec<QueryRequest>,
    block_id: IdType,
    chain: &impl ReadInterface,
) -> Result<VerifyObject> {
    let block_data = chain.read_block_data(block_id)?;
    for request in &requests{
        if request.key == "timestamp".to_string(){
            continue;
        }
        if request.key=="id"
    }
    Ok(vo)
}

/// return BlockData & BlockHeader falls in the timestamp range
fn query_chain_no_inter_index(
    q_param: &QueryParam,
    block_headers: &mut Vec<BlockHeader>,
    block_datas: &mut Vec<BlockData>,
    chain: &impl ReadInterface,
) -> Result<()>{
    let start_index = chain.get_parameter()?.start_block_id;
    let mut block_index = start_index + chain.get_parameter()?.block_count.clone() - 1;
    while block_index >= start_index as u64 {
        let block_header = chain.read_block_header(block_index)?;
        let block_data = chain.read_block_data(block_index)?;
            block_headers.push(block_header.to_owned());
            block_datas.push(block_data.to_owned());
        block_index -= 1;
    }

    Ok(())
}