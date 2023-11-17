
use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use curve25519_dalek::ristretto::CompressedRistretto;
use serde::{Serialize, Deserialize};
use super::*;

pub mod transaction;
pub mod btree;
pub use transaction::*;

pub mod index;
pub use index::*;

pub mod utils;
pub use utils::*;

pub mod build;
pub use build::*;

pub mod query;
pub use query::*;

pub mod verify;
pub use verify::*;

pub type IdType = u64;
// Timestamp size 4 bytes
pub type TsType = u64; 
// public key size 4 bytes
pub type PkType = CompressedRistretto;
//key
pub type KeyType = String;
//transaction valßßue
pub type TxType = u64;
// FloatType especially for linear regression
pub type FloatType = f64;

pub static BLOOM_CAPACITY: usize = 50000;
pub static BLOOM_FP: f64 = 0.01;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub error_bounds: FloatType,
    pub inter_index: bool,
    pub intra_index: bool,
    pub start_block_id: u64,
    pub block_count: u64,
    pub inter_index_timestamps: Vec<TsType>,
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum BTreeEnum {
    U64(BTreeMap<u64, Transaction>),
    String(BTreeMap<String, Transaction>),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexConfigs {
    pub attribute: KeyType,
    pub config: Vec<IndexConfig>,
}
// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// pub struct IndexConfigs_mid {
//     pub block_height:IdType,
//     pub config: HashMap<KeyType,IndexConfig>,
// }
// trait Extract<T> {
//     fn extract(&self) -> Option<&T>;
// }

// impl Extract<BTreeMap<u64, Transaction>> for BTreeEnum {
//     fn extract(&self) -> Option<&BTreeMap<u64, Transaction>> {
//         if let BTreeEnum::U64(btree_map) = self {
//             Some(btree_map)
//         } else {
//             None
//         }
//     }
// }

// impl Extract<BTreeMap<String, Transaction>> for BTreeEnum {
//     fn extract(&self) -> Option<&BTreeMap<String, Transaction>> {
//         if let BTreeEnum::String(btree_map) = self {
//             Some(btree_map)
//         } else {
//             None
//         }
//     }
// }
pub enum ExtractedData{
    TxIds(Vec<u64>),
    Addresses(Vec<String>),
    TransValues(Vec<u64>),
}
#[async_trait::async_trait]
pub trait LightNodeInterface {
    async fn lightnode_get_parameter(&self) -> Result<Parameter>;
    async fn lightnode_read_block_header(&self, id: IdType) -> Result<BlockHeader>;
}

pub trait ReadInterface {
    fn get_parameter(&self) -> Result<Parameter>;
    fn read_block_header(&self, id: IdType) -> Result<BlockHeader>;
    fn read_block_data(&self, id: IdType) -> Result<BlockData>;
    fn read_intra_index(&self, timestamp: TsType) -> Result<IntraIndex>;
    fn read_intra_indexs(&self) -> Result<Vec<IntraIndex>>;
    fn read_transaction(&self, id: IdType) -> Result<Transaction>;
    fn read_inter_index(&self, timestamp: TsType) -> Result<InterIndex>;
    fn read_inter_indexs(&self) -> Result<Vec<InterIndex>>;
    fn read_index_config(&self,attribute:KeyType) -> Result<IndexConfigs>;
}

pub trait WriteInterface {
    fn set_parameter(&mut self, param: Parameter) -> Result<()>;
    fn write_block_header(&mut self, header: BlockHeader) -> Result<()>;
    fn write_block_data(&mut self, data: BlockData) -> Result<()>;
    fn write_intra_index(&mut self, index: IntraIndex) -> Result<()>;
    fn write_transaction(&mut self, tx: Transaction) -> Result<()>;
    fn write_inter_index(&mut self, index: InterIndex) -> Result<()>;
    fn write_index_config(&self,config:IndexConfigs) -> Result<()>;
}

#[cfg(test)]
mod tests;
