extern crate ndarray;
use ndarray::{Array2, Axis, array, Array1};

use crate::BLOCK_ACCESS_COUNTER;

pub fn convert_to_normalized_matrix(block_count: usize) -> Array2<f64> {
    let counter = BLOCK_ACCESS_COUNTER.lock().unwrap();
    let row_count = counter.len();

    // 创建一个新的 Array2<f64> 矩阵
    let mut matrix = Array2::zeros((row_count, block_count + 1));

    for (i, row) in counter.iter().enumerate() {
        let row_sum: f64 = row.iter().map(|&x| x as f64).sum();

        for (j, &count) in row.iter().enumerate() {
            let normalized_value = if row_sum != 0.0 {
                count as f64 / row_sum
            } else {
                0.0
            };
            matrix[(i, j)] = normalized_value;
        }
    }

    matrix
}


pub fn holt_linear_exponential_smoothing(matrix: &Array2<f64>, alpha: f64, beta: f64) -> Array1<f64> {
    let (n, m) = matrix.dim();
    if n < 2 {
        return matrix.row(n - 1).to_owned();
    }
    
    let mut last_forecasts = Array1::zeros(n);

    for i in 0..n {
        let mut level = matrix[(i, 0)];
        let mut trend = matrix[(i, 1)] - matrix[(i, 0)];

        for j in 0..m {
            let value = matrix[(i, j)];
            let last_level = level;
            level = alpha * value + (1.0 - alpha) * (level + trend);
            trend = beta * (level - last_level) + (1.0 - beta) * trend;

            if j == m - 1 { // 只在最后一次迭代时保存预测值
                last_forecasts[i] = level;
            }
        }
    }

    last_forecasts
}



// fn holt_linear_exponential_smoothing(matrix: &Vec<Vec<f64>>, alpha: f64, beta: f64) -> Result<Vec<Vec<f64>>, &'static str> {
//     // 确保矩阵至少有一行和两列
//     if matrix.is_empty() || matrix.first().map_or(true, |row| row.len() < 2) {
//         return Err("矩阵至少需要有一行和两列");
//     }

//     // 获取矩阵的行数和列数
//     let n = matrix.len();
//     let m = matrix[0].len();

//     // 检查所有行的长度是否一致
//     if !matrix.iter().all(|row| row.len() == m) {
//         return Err("所有行的长度必须相同");
//     }

//     // 初始化预测结果矩阵
//     let mut forecasts = vec![vec![0.0; m]; n];

//     // 执行霍尔特线性指数平滑
//     for i in 0..n {
//         let mut level = matrix[i][0];
//         let mut trend = matrix[i][1] - matrix[i][0];

//         for j in 0..m {
//             let value = matrix[i][j];
//             let last_level = level;
//             level = alpha * value + (1.0 - alpha) * (level + trend);
//             trend = beta * (level - last_level) + (1.0 - beta) * trend;
//             forecasts[i][j] = level;
//         }
//     }

//     Ok(forecasts)
// }
