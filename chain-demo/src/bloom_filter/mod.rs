
use std::{collections::hash_map::DefaultHasher, hash::Hash};

pub mod bitset;
pub use bitset::*;

pub mod km_bloom_filter;
pub use km_bloom_filter::*;

pub mod seeded_bloom_filter;
pub use seeded_bloom_filter::*;

pub use km_bloom_filter::KMBloomFilter;
pub use seeded_bloom_filter::SeededBloomFilter;

pub type DefaultBloomFilter = KMBloomFilter<ahash::AHasher, DefaultHasher>;

pub trait BloomFilter {

    fn insert<T: Hash>(&mut self, data: &T);

    fn contains<T: Hash>(&self, data: &T) -> bool;
}


fn optimal_bit_count(desired_capacity: usize, desired_false_positive_probability: f64) -> usize {
    (-(desired_capacity as f64 * desired_false_positive_probability.ln()) / (2.0f64.ln().powi(2)))
        .ceil() as usize
}

fn optimal_number_of_hashers(desired_capacity: usize, bit_count: usize) -> usize {
    ((bit_count as f64 / desired_capacity as f64) * 2.0f64.ln()).round() as usize
}

fn approximate_element_count(
    number_of_hashers: usize,
    bits_per_hasher: usize,
    number_of_ones: usize,
) -> f64 {
    -(bits_per_hasher as f64)
        * (1.0 - (number_of_ones as f64) / ((number_of_hashers * bits_per_hasher) as f64)).ln()
}

fn approximate_false_positive_probability(
    number_of_hashers: usize,
    bits_per_hasher: usize,
    element_count: f64,
) -> f64 {
    (1.0 - std::f64::consts::E.powf(-element_count / bits_per_hasher as f64))
        .powf(number_of_hashers as f64)
}
