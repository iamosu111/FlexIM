extern crate rand;
use std::collections::HashMap;

use log::info;
use ndarray::Array1;
use rand::distributions::{Distribution, WeightedIndex};
use rand::rngs::ThreadRng;
use rand::Rng;
use serde::{Serialize, Deserialize};
use serde_json::value::Index;
use crate::{KeyType, BTreeEnum, IdType, IndexConfigs, KEY_USAGE_COUNTER, ReadInterface, WriteInterface};
use crate::btree::update_indices_based_on_config;
use super::*;
// use lazy_static::lazy_static;
// use std::{collections::HashMap, sync::Mutex};
// lazy_static! {
//     static ref GLOBAL_INDEX_CONFIGS: Mutex<HashMap<KeyType, Vec<IndexConfig>>> = Mutex::new(HashMap::new());
// }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexConfigs_map(pub HashMap<KeyType, Vec<IndexConfig>>);
impl IndexConfigs_map {
    pub fn add_config(&mut self, key: KeyType, config: IndexConfig) {
        self.0.entry(key).or_default().push(config);
    }
    pub fn new() -> Self {
        IndexConfigs_map(HashMap::new())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// 索引配置结构
pub struct IndexConfig {
   pub performance: f64, // 性能评分
   pub storage_cost: f64, // 存储成本
//    pub is_active: bool, // 是否为候选索引
   pub block_height: IdType, //区块高度
   pub attribute: String, //索引类型 
}
impl From<IndexConfigs_map> for Vec<IndexConfigs> {
    fn from(map: IndexConfigs_map) -> Self {
        map.0.into_iter().map(|(attribute, config)| {
            IndexConfigs {
                attribute,
                config,
            }
        }).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct IndexSelectionBandit {
    arms: Vec<IndexConfig>,
    temperature: f64, // Boltzmann Exploration 温度参数
    budget: f64, // 存储预算
    // episode: usize //运行周期
}

impl IndexConfig {
    pub fn from_b_tree(performance: f64, btree: &BTreeEnum, block_height: IdType, attribute: &String) -> Result<IndexConfig, bincode::Error> {
        match bincode::serialize(&btree) {
            Ok(serialized) => {
                let storage_cost = serialized.len() as f64;// 单位为字节
                Ok(IndexConfig {
                    performance,
                    storage_cost,
                    // is_active: false,
                    block_height,
                    attribute: attribute.clone(),
                })
            },
            Err(e) => Err(e),
        }
    }
}

impl IndexSelectionBandit {
    // 初始化 CMAB 模型
    fn new(arms: Vec<IndexConfig>, temperature: f64, budget: f64) -> Self {
        Self {
            arms,
            temperature,
            budget,
            // episode: 0,
        }
    }

    fn update_arms(&mut self, newIndexConfig: Vec<IndexConfig>){
        self.arms.extend(newIndexConfig);
    }

    // 使用 Boltzmann Exploration 选择活跃的臂
    fn choose_arm(&mut self, block_frequency: &Array1<f64>) -> Result<Vec<IndexConfig>> {
        let mut selected_index_configuration = Vec::new();
        let mut remaining_budget = self.budget;
        let mut rng = rand::thread_rng();
        let temperature=self.temperature;
        // 获取活跃臂的索引和对应的估计奖励
        let mut active_arms: Vec<_> = self.arms.iter()
            .enumerate()
            .filter_map(|(index, arm)| {
                // 将 arm.block_height 转换为 usize
                let block_height_usize: usize = match arm.block_height.try_into() {
                    Ok(height) => height,
                    Err(_) => return None, // 如果转换失败，则跳过此臂
                };
    
                if block_height_usize < block_frequency.len() && arm.storage_cost <= remaining_budget {
                    let weighted_performance = (arm.performance * block_frequency[block_height_usize] / self.temperature).exp();
                    Some((index, weighted_performance))
                } else {
                    None // 如果超出索引范围或预算，则跳过此臂
                }
            })
            .collect();
        
        // 如果没有活跃的臂，则返回 None
        if active_arms.is_empty() {
            panic!("Storage budget is too less");
        }
        
        // 计算每个活跃臂的选择概率
        // let probabilities: Vec<f64> = active_arms.iter()
        //     .map(|(_index, &reward)| {
        //         if storage_cost <= remaining_budget {
        //             (performance / self.temperature).exp()
        //         } else {
        //             0.0 // 超出预算的配置不予考虑
        //         }
        //     })
        //     .collect();
        
        // let total: f64 = probabilities.iter().sum();

        while remaining_budget > 0.0 && !active_arms.is_empty() {
            // 直接使用性能指数作为权重
            let weights: Vec<f64> = active_arms.iter().map(|&(_index, exp_perf)| exp_perf).collect();
            let dist = WeightedIndex::new(&weights).unwrap();
            let chosen_index = dist.sample(&mut rng);

            // 使用索引更新选择和预算
            let (arm_index, exp_perf) = active_arms[chosen_index];
            let chosen_arm = self.arms[arm_index].clone();
            selected_index_configuration.push(chosen_arm.clone());
            remaining_budget -= chosen_arm.storage_cost;

            // 移除被选择的臂
            active_arms.swap_remove(chosen_index);

        }
        // self.episode+=1;
        // 更新温度参数
        // self.update_temperature();
       Ok(selected_index_configuration) 
    }

    // fn update_temperature(&mut self) {
    //     // 随着迭代次数的增加，温度逐渐降低
    //     let decay_rate = 0.005; // 温度衰减率
    //     self.temperature *= 1.0 - (decay_rate * self.episode as f64);
    // }
    // // 其他方法，比如更新臂的奖励，激活或停用臂等...
}

pub struct QueryCost {
    pub(crate) n_pages: f64,       // 页数，初始值为
    pub(crate) c_page: f64,        // 每页成本
    pub(crate) c_tuple: f64,       // 每元组成本
    pub(crate) n_total_tuple: f64, // 元组总数，定位区块内总key_value对数
}

impl QueryCost {
    // 成本函数
    pub fn cost(&self, lambda: f64, sigma: f64) -> f64 {
        // sigma 初始值为选择率， lambda初始值为
        self.n_pages * lambda * self.c_page + self.c_tuple * sigma * self.n_total_tuple
    }

    // 批量梯度计算
    pub fn batch_gradient(&self, lambda: f64, sigma: f64, observed_costs: &[f64]) -> Result<(f64, f64), &'static str> {
        if observed_costs.is_empty() {
            return Err("observed_costs array is empty");
        }

        let mut grad_lambda = 0.0;
        let mut grad_sigma = 0.0;

        for &observed_cost in observed_costs {
            let predicted_cost = self.cost(lambda, sigma);
            let error = predicted_cost - observed_cost;

            grad_lambda += error * self.n_pages * self.c_page;
            grad_sigma += error * self.c_tuple * self.n_total_tuple;
        }

        // 平均梯度
        let len = observed_costs.len() as f64;
        grad_lambda /= len;
        grad_sigma /= len;

        Ok((grad_lambda, grad_sigma))
    }
    
    pub fn gradient_descent(&self, initial_lambda: f64, initial_sigma: f64, learning_rate: f64, iterations: usize, observed_costs: &[f64]) -> Result<(f64, f64), &'static str> {
        if learning_rate <= 0.0 || learning_rate > 1.0 {
            return Err("Invalid learning rate. It should be in (0.0, 1.0]");
        }

        let mut lambda = initial_lambda;   
        let mut sigma = initial_sigma;
    
        for _ in 0..iterations {
            let (grad_lambda, grad_sigma) = self.batch_gradient(lambda, sigma, observed_costs)?;
    
            // 更新 lambda 和 sigma
            lambda -= learning_rate * grad_lambda;
            sigma -= learning_rate * grad_sigma;
        }
    
        Ok((lambda, sigma))
    }
}

pub fn index_management(chain: &mut (impl ReadInterface + WriteInterface))-> Result<()> {
    info!("index management begin!");
    // frequency analysis
    let cpu_timer = howlong::ProcessCPUTimer::new();
    let parameter=chain.get_parameter()?;
    let time_series=convert_to_normalized_matrix(parameter.block_count.try_into().unwrap());
    let alpha=0.4;
    let beta=0.6;
    let frequency=holt_linear_exponential_smoothing(&time_series, alpha, beta);
   
    // workloads_analysis
    let mut key_usage = KEY_USAGE_COUNTER.lock().unwrap();
    let keys: Vec<KeyType> = key_usage.keys().cloned().collect();
    let mut arms_map=Vec::new();
    for key in keys {
        let configs = chain.read_index_config(key)?;
        arms_map.extend(configs.config);
    }
    // ... 构建 IndexSelectionBandit ...
    let temperature = 0.3; // 示例值
    let budget = (100*1024*1024) as f64; // 100MB
    key_usage.clear();
    let mut Bandit = IndexSelectionBandit {
        arms: arms_map,
        temperature,
        budget,
    };  
    let index_configuration= Bandit.choose_arm(&frequency)?;
    update_indices_based_on_config(&index_configuration,chain)?;
    info!("index management end, use time {}",cpu_timer.elapsed());
    Ok(())
}

