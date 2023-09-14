use std::collections::{HashMap, BTreeMap};
use log::info;
use crate::{Digest, btree::index_build,btree::index_build_block};
use super::*;

use crate::SeededBloomFilter;

///
/// 
/// For BlockData
/// --
/// intra-index is sorted & storing the first position of distinguished key.
/// 
/// aggre_signs is a series of aggregated signature based on transaction's unique key.
/// 
/// For BlockHeader
/// --
/// 
pub fn build_block<'a>(
    block_id: IdType,
    pre_hash: Digest,
    raw_txs: impl Iterator<Item = &'a RawTransaction>,
    chain: &mut (impl ReadInterface + WriteInterface),
) -> Result<(BlockHeader)> {    
    // let param = chain.get_parameter()?;
    let txs: Vec<Transaction> = raw_txs.map(|rtx: &RawTransaction| Transaction::create(rtx)).collect();
    let mut _time_stamp: TsType = Default::default();
    let mut tx_ids: Vec<IdType> = Vec::new();
    _time_stamp = txs[0].value.time_stamp;
    let mut attributes: [usize;3]= [0,1,2];
    let mut height=[block_id];
    let mut bloom_filter: SeededBloomFilter = SeededBloomFilter::new(BLOOM_CAPACITY,BLOOM_FP);
    for tx in txs.iter(){
      bloom_filter.insert(&tx.id);
      bloom_filter.insert(&tx.value.address);
      bloom_filter.insert(&tx.value.trans_value);
    }
    let block_header = BlockHeader{
        block_id,
        pre_hash,
        time_stamp: _time_stamp,
        BMT_root: bloom_filter,
        rmt_root: pre_hash,
    };

    let block_data = BlockData {
        block_id,
        tx_ids,
        txs,
    };

    chain.write_block_header(block_header.clone())?;
    chain.write_block_data(block_data.clone())?;
    //todo : Index cost evaluation
    let (block_index,index_cost_value)=index_build_block(&attributes,block_id,chain)?;
    //todo : BMT build
    //todo : RMT build
    let mut index_cost= IndexCost {
        blockId: block_id,
        index_cost: index_cost_value,
    };
    //chain.write_intra_index(block_index)?;
    chain.write_index_cost(index_cost)?;
    Ok((block_header))
}

pub fn build_inter_index(
    block_headers: Vec<BlockHeader>,
    chain: &mut (impl ReadInterface + WriteInterface)
) -> Result<IdType>{
    info!("build inter index");
    let mut inter_indexs: BTreeMap<TsType, InterIndex> = BTreeMap::new();
    let timestamps: Vec<TsType> = Vec::from_iter(block_headers.iter().map(|header| header.time_stamp.to_owned()));
    let heights: Vec<IdType> = Vec::from_iter(block_headers.iter().map(|header| header.block_id.to_owned()));
    let mut param = chain.get_parameter().unwrap();
    let err_bounds = param.error_bounds as FloatType;
    let mut pre_timestamp = timestamps.first().unwrap().to_owned();
    // init inter_index
    inter_indexs.entry(pre_timestamp)
        .or_insert(InterIndex { start_timestamp: pre_timestamp.clone(), regression_a: 1.0, regression_b: 1.0 });
    
    for block_header in block_headers.iter(){
        let mut inter_index = inter_indexs.get(&pre_timestamp).unwrap().to_owned();
        let point_x = block_header.time_stamp as FloatType;
        let point_y = block_header.block_id as FloatType;
        if is_within_boundary(inter_index.regression_a, inter_index.regression_b, point_x, point_y, err_bounds) {
            continue;
        }else {
            // info!("timestamp {:?}", point_x.clone());
            let start_index: usize = timestamps.binary_search(&pre_timestamp).unwrap();
            let end_index_result = timestamps.binary_search(&block_header.time_stamp);
            let end_index: usize =  match end_index_result {
                Ok(_t) => _t,
                Err(_e) => {
                    panic!("problem encounted with binary search timestamp key {:?}",block_header.time_stamp)
                },
            };
            let (regression_a, regression_b) = linear_regression(&timestamps[start_index..end_index + 1], &heights[start_index..end_index + 1]);
            if is_within_boundary(regression_a, regression_b, point_x, point_y, err_bounds) {
                inter_index.regression_a = regression_a;
                inter_index.regression_b = regression_b;
                // update value
                inter_indexs.insert(pre_timestamp.clone(), inter_index.clone());
                continue;
            }else {
                // start new piecewise linear function
                pre_timestamp = block_header.time_stamp.clone();
                // info!("pre_timestamp {:?}",pre_timestamp);
                inter_indexs.entry(pre_timestamp)
                    .or_insert(InterIndex { start_timestamp: pre_timestamp.clone(), regression_a: 1.0, regression_b: 1.0 });
            }

        }   
    }
    
    let mut inter_index_size: IdType = 0;
    //write inter_indexs && count inter_index_size
    for inter_index in inter_indexs.values() {
        // each InterIndex contains 1 TsType & 2 FloatType Storage size eq 3 * 8 = 24 B
        inter_index_size += 24;
        chain.write_inter_index(inter_index.to_owned())?;
        param.inter_index_timestamps.push(inter_index.start_timestamp);
    }
    chain.set_parameter(param.clone())?;
    Ok(inter_index_size)
}
