// use crate::{
//     approximate_element_count, approximate_false_positive_probability, bitset::Bitset,
//     optimal_bit_count, optimal_number_of_hashers, BloomFilter,
// };
use ahash::AHasher;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use super::*;

/// A bloom filter that uses a single Hasher that can be seeded to simulate an arbitrary number
/// of hash functions.
///
/// Internally, the implementation uses *ahash::AHasher*.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SeededBloomFilter {
    number_of_hashers: usize,
    bitset: Bitset,
    bits_per_hasher: usize,
}

impl SeededBloomFilter {

    pub fn new(desired_capacity: usize, desired_false_positive_probability: f64) -> Self {
        if desired_capacity == 0 {
            panic!("an empty bloom filter is not defined");
        }
        let bit_count = optimal_bit_count(desired_capacity, desired_false_positive_probability);
        let number_of_hashers = optimal_number_of_hashers(desired_capacity, bit_count);
        let bits_per_hasher = (bit_count as f64 / number_of_hashers as f64).ceil() as usize;
        Self {
            bitset: Bitset::new(bits_per_hasher * number_of_hashers),
            number_of_hashers,
            bits_per_hasher,
        }
    }

    /// Approximate number of elements stored.
    /// Approximation technique taken from Wikipedia:
    /// > Wikipedia, ["Bloom filter"](https://en.wikipedia.org/wiki/Bloom_filter#Approximating_the_number_of_items_in_a_Bloom_filter) [Accessed: 02.12.2020]
    pub fn approximate_element_count(&self) -> f64 {
        approximate_element_count(
            self.number_of_hashers,
            self.bits_per_hasher,
            self.bitset.count_ones(),
        )
    }

    /// Return the current approximate false positive probability which depends on the current
    /// number of elements in the filter.
    ///
    /// The probability is given as a value in the interval [0,1]
    /// Approximation technique taken from Sagi Kedmi:
    /// > S. Kedmi, ["Bloom Filters for the Perplexed"](https://sagi.io/bloom-filters-for-the-perplexed/), July 2017 [Accessed: 02.12.2020]
    pub fn approximate_current_false_positive_probability(&self) -> f64 {
        approximate_false_positive_probability(
            self.number_of_hashers,
            self.bits_per_hasher,
            self.approximate_element_count(),
        )
    }

        /// Creates a union of this bloom filter and 'other', which means 'contains' of the resulting
    /// bloom filter will always return true for elements inserted in either this bloom filter or in
    /// 'other' before creation.
    ///
    /// # Panics
    ///
    /// Panics if the desired capacity or desired false positive probability of 'self' and 'other'
    /// differ.
    /// 
    /// # Examples
    /// 
    /// Union of two bloom filters with the same configuration.
    /// ``
    /// use bloom_filter_simple::{BloomFilter,SeededBloomFilter};
    ///
    /// fn main() {
    ///     // The configuration of both bloom filters has to be the same
    ///     let desired_capacity = 10_000;
    ///     let desired_fp_probability = 0.0001;
    ///
    ///     // We initialize two new SeededBloomFilter 
    ///     let mut filter_one = SeededBloomFilter::new(desired_capacity, desired_fp_probability);
    ///     let mut filter_two = SeededBloomFilter::new(desired_capacity, desired_fp_probability);
    /// 
    ///     // Insert elements into the first filter
    ///     filter_one.insert(&0);
    ///     filter_one.insert(&1);
    /// 
    ///     // Insert elements into the second filter
    ///     filter_two.insert(&2);
    ///     filter_two.insert(&3);
    ///     
    ///     // Now we retrieve the union of both filters
    ///     let filter_union = filter_one.union(&filter_two);
    ///
    ///     // The union will return true for a 'contains' check for the elements inserted 
    ///     // previously into at least one of the constituent filters.
    ///     assert_eq!(true, filter_union.contains(&0));
    ///     assert_eq!(true, filter_union.contains(&1));
    ///     assert_eq!(true, filter_union.contains(&2));
    ///     assert_eq!(true, filter_union.contains(&3));
    /// }
    /// ``
    pub fn union(&self, other: &Self) -> Self {
        if !self.eq_configuration(other) {
            panic!("unable to union k-m bloom filters with different configurations");
        }
        Self {
            number_of_hashers: self.number_of_hashers,
            bitset: self.bitset.union(&other.bitset),
            bits_per_hasher: self.bits_per_hasher,
        }
    }

  
    pub fn intersect(&self, other: &Self) -> Self {
        if !self.eq_configuration(other) {
            panic!("unable to intersect k-m bloom filters with different configurations");
        }
        Self {
            number_of_hashers: self.number_of_hashers,
            bitset: self.bitset.intersect(&other.bitset),
            bits_per_hasher: self.bits_per_hasher,
        }
    }

    /// Checks whether two bloom filters were created with the same desired capacity and desired false
    /// positive probability.
    pub fn eq_configuration(&self, other: &Self) -> bool {
        self.number_of_hashers == other.number_of_hashers
            && self.bits_per_hasher == other.bits_per_hasher
    }

    fn index<T>(i: usize, bits_per_hash: usize, data: &T) -> usize
    where
        T: Hash,
    {
        let mut hasher = AHasher::new_with_keys(i as u128, i as u128);
        data.hash(&mut hasher);
        i * bits_per_hash + hasher.finish() as usize % bits_per_hash
    }
}

impl Debug for SeededBloomFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SeededBloomFilter{{{:?}}}", self.bitset)
    }
}

impl BloomFilter for SeededBloomFilter {
    fn insert<T>(&mut self, data: &T)
    where
        T: Hash,
    {
        for i in 0..self.number_of_hashers {
            self.bitset
                .set(Self::index(i, self.bits_per_hasher, &data), true);
        }
    }

    fn contains<T>(&self, data: &T) -> bool
    where
        T: Hash,
    {
        for i in 0..self.number_of_hashers {
            if !self.bitset.get(Self::index(i, self.bits_per_hasher, &data)) {
                return false;
            }
        }

        return true;
    }
}

