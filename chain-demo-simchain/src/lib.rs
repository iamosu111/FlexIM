#[macro_use]
extern crate log;

use anyhow::{Context, Result};
use rocksdb::{self, DB};
use std::fs;
use std::path::{Path, PathBuf};
use chain_demo::*;
use rocksdb::{WriteBatch,IteratorMode};
use core::result::Result::Ok;

pub struct SimChain {
    root_path: PathBuf,
    param: Parameter,
    block_header_db: DB,
    block_data_db: DB,
    intra_index_db:DB,
    index_cost_db:DB,
    inter_index_db:DB,
    index_config_db:DB,
    tx_db: DB,
}

impl SimChain {
    pub fn create(path: &Path, param: Parameter) -> Result<Self> {
        info!("create db at {:?}", path);
        fs::remove_dir_all(path).context(format!("failed to remove dir {:?}", path))?;
        fs::create_dir_all(path).context(format!("failed to create dir {:?}", path))?;
        fs::write(
            path.join("param.json"),
            serde_json::to_string_pretty(&param)?
        )?;
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        Ok(Self {
            root_path: path.to_owned(),
            param,
            block_header_db: DB::open(&opts, path.join("blk_header.db"))?,
            block_data_db: DB::open(&opts, path.join("blk_data.db"))?,
            intra_index_db: DB::open(&opts, path.join("intra_index.db"))?,
            index_cost_db: DB::open(&opts, path.join("index_cost.db"))?,
            inter_index_db:DB::open(&opts,path.join("inter_index_db"))?,
            index_config_db:DB::open(&opts,path.join("index_config_db"))?,
            tx_db: DB::open(&opts, path.join("tx.db"))?,
        })
    }

    pub fn open(path: &Path) -> Result<Self> {
        info!("open db at {:?}", path);

        Ok(Self {
            root_path: path.to_owned(),
            param: serde_json::from_str::<Parameter>(&fs::read_to_string(path.join("param.json"))?)?,
            block_header_db: DB::open_default(path.join("blk_header.db"))?,
            block_data_db: DB::open_default(path.join("blk_data.db"))?,
            intra_index_db: DB::open_default(path.join("intra_index.db"))?,
            tx_db: DB::open_default(path.join("tx.db"))?,
            index_cost_db: DB::open_default(path.join("index_cost.db"))?,
            inter_index_db: DB::open_default(path.join("inter_index_db"))?,
            index_config_db: DB::open_default(path.join("index_config_db"))?,
        })
    }
}

#[async_trait::async_trait]
impl LightNodeInterface for SimChain{
    async fn lightnode_get_parameter(&self) -> Result<Parameter> {
        self.get_parameter()
    }
    async fn lightnode_read_block_header(&self, id: IdType) -> Result<BlockHeader> {
        self.read_block_header(id)
    }
}

impl ReadInterface for SimChain {
    fn get_parameter(&self) -> Result<Parameter>{
        Ok(self.param.clone())
    }
    fn read_block_header(&self, id: IdType) -> Result<BlockHeader>{
        let data = self
            .block_header_db
            .get(id.to_le_bytes())?
            .context("failed to read block header")?;
        Ok(bincode::deserialize::<BlockHeader>(&data[..])?)
    }
    fn read_block_data(&self, id: IdType) -> Result<BlockData>{
        let data = self
            .block_data_db
            .get(id.to_le_bytes())?
            .context("failed to read block data")?;
        Ok(bincode::deserialize::<BlockData>(&data[..])?)
    }
    fn read_intra_index(&self, id: IdType) -> Result<IntraIndex> {
        let data_result = self.intra_index_db.get(id.to_le_bytes());
    
        match data_result {
            Ok(Some(data)) => Ok(bincode::deserialize::<IntraIndex>(&data[..])?),
            Ok(None) => Ok(IntraIndex::new(id)), // 当键不存在时返回一个新的 IntraIndex 实例
            Err(e) => Err(e).context("failed to read intra index"),
        }
    }
    fn read_intra_indexs_size(&self) -> usize {
        let mut res:usize=0;
        let iter=self.intra_index_db.iterator(IteratorMode::Start);
        for (key, value) in iter {
            res+=key.len();
            res+=value.len();
        }
        res
    }
    // fn read_intra_indexs(&self) -> Result<Vec<IntraIndex>>{
    //     let mut intra_indexs: Vec<IntraIndex> = Vec::new();
    //     for blockId in &self.param.inter_index_timestamps {
    //         info!("read_inter_indexs timestamps {}",timestamp.to_owned());
    //         inter_indexs.push(self.read_inter_index(timestamp.to_owned())?);
    //     }
    //     Ok(inter_indexs)
    // }
    // fn read_intra_index_node(&self, id: IdType) -> Result<IntraIndexNode>;
    // fn read_skip_list_node(&self, id: IdType) -> Result<SkipListNode>;
    fn read_transaction(&self, id: IdType) -> Result<Transaction>{
        let data = self
            .tx_db
            .get(id.to_le_bytes())?
            .context("failed to read transaction")?;
        Ok(bincode::deserialize::<Transaction>(&data[..])?)
    }
    fn read_inter_index(&self, timestamp: TsType) -> Result<InterIndex>{
        let data = self
            .inter_index_db
            .get(timestamp.to_le_bytes())?
            .context("failed to read inter index")?;
        Ok(bincode::deserialize::<InterIndex>(&data[..])?)
    }
    fn read_inter_indexs(&self) -> Result<Vec<InterIndex>>{
        let mut inter_indexs: Vec<InterIndex> = Vec::new();
        for timestamp in &self.param.inter_index_timestamps {
            info!("read_inter_indexs timestamps {}",timestamp.to_owned());
            inter_indexs.push(self.read_inter_index(timestamp.to_owned())?);
        }
        Ok(inter_indexs)
    }

    fn read_index_config(&self,attribute: KeyType) -> Result<IndexConfigs>{
        let data = self
            .index_config_db
            .get(attribute.as_bytes())?
            .context("failed to read index config")?;
        Ok(bincode::deserialize::<IndexConfigs>(&data[..])?)
    }
}

impl WriteInterface for SimChain {
    fn set_parameter(&mut self, param: Parameter) -> Result<()>{
        self.param = param;
        let data = serde_json::to_string_pretty(&self.param)?;
        fs::write(self.root_path.join("param.json"), data)?;
        Ok(())
    }
    fn write_block_header(&mut self, header: BlockHeader) -> Result<()>{
        let bytes = bincode::serialize(&header)?;
        self.block_header_db
            .put(header.block_id.to_le_bytes(), bytes)?;
        Ok(())
    }
    fn write_block_data(&mut self, data: BlockData) -> Result<()>{
        let bytes = bincode::serialize(&data)?;
        self.block_data_db
            .put(data.block_id.to_le_bytes(), bytes)?;
        Ok(())
    }
    fn write_intra_index(&mut self, index: IntraIndex) -> Result<()>{
        let bytes = bincode::serialize(&index)?;
        self.intra_index_db
            .put(index.blockId.to_le_bytes(), bytes)?;
        Ok(())
    }
    fn update_intra_index(&mut self, indexs: Vec<IntraIndex>) -> Result<()>{
        let mut batch = WriteBatch::default();

        // 添加删除操作到批处理中
        for (key, _) in self.intra_index_db.iterator(IteratorMode::Start) {
            batch.delete(key);
        }
    
        // 遍历 Vec<IntraIndex> 并将每个 IntraIndex 添加到批处理中
        for index in indexs {
            let bytes = bincode::serialize(&index)?;
            batch.put(index.blockId.to_le_bytes(), bytes);
        }
    
        // 原子地应用批处理
        self.intra_index_db.write(batch)?;
        Ok(())
    }
    // fn write_intra_index_node(&mut self, node: IntraIndexNode) -> Result<()>;
    // fn write_skip_list_node(&mut self, node: SkipListNode) -> Result<()>;
    fn write_transaction(&mut self, tx: Transaction) -> Result<()>{
        let bytes = bincode::serialize(&tx)?;
        self.tx_db
            .put(tx.id.to_le_bytes(), bytes)?;
        Ok(())
    }
    fn write_inter_index(&mut self, index: InterIndex) -> Result<()>{
        let bytes = bincode::serialize(&index)?;
        self.inter_index_db
            .put(index.start_timestamp.to_le_bytes(), bytes)?;
        Ok(())
    }
    fn write_index_config(&mut self, config: IndexConfigs) -> Result<()>{
        let bytes = bincode::serialize(&config)?;
        self.index_config_db
            .put(config.attribute.as_bytes(), bytes)?;
        Ok(())
    }
}