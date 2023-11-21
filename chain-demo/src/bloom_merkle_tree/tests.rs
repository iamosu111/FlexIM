#![cfg(test)]

use crate::bloom_filter::{SeededBloomFilter, BloomFilter};
use ring::digest::{Algorithm, SHA512};

use super::utils::*;
use crate::bloom_merkle_tree::BloomMerkleTree;
static DIGEST: &Algorithm = &SHA512;


#[test]
fn test_from_str_vec() {
    let values = vec!["one", "two", "three", "four"];

    let hashes :Vec<Vec<u8>> = vec![
        DIGEST.hash_leaf(&values[0].as_bytes()).as_ref().into(),
        DIGEST.hash_leaf(&values[1].as_bytes()).as_ref().into(),
        DIGEST.hash_leaf(&values[2].as_bytes()).as_ref().into(),
        DIGEST.hash_leaf(&values[3].as_bytes()).as_ref().into(),
    ];

    let count = values.len();
    let tree = BloomMerkleTree::from_vec(DIGEST, values);

    let mut bf01 = SeededBloomFilter::new(BLOOMFILTER_CAPASITY,BLOOMFILTER_FP);
    bf01.insert(&hashes[0]);
    bf01.insert(&hashes[1]);

    let mut bf23 = SeededBloomFilter::new(BLOOMFILTER_CAPASITY,BLOOMFILTER_FP);
    bf23.insert(&hashes[2]);
    bf23.insert(&hashes[3]);

    let h01: Vec<u8> = DIGEST.hash_nodes(&hashes[0], &hashes[1], bf01.to_bytes()).as_ref().into();
    let h23: Vec<u8> = DIGEST.hash_nodes(&hashes[2], &hashes[3], bf23.to_bytes()).as_ref().into();

    let bf_root = bf01.union(&bf23);
    let root_hash = DIGEST.hash_nodes(&h01, &h23, bf_root.to_bytes());

    assert_eq!(tree.count(), count);
    assert_eq!(tree.height(), 2);
    assert_eq!(tree.root_hash().as_slice(), root_hash.as_ref());
    
}

#[test]
fn test_bloom_contains(){
    let values: Vec<Vec<u8>> = vec!["one".into(), "two".into(), "three".into(), "four".into()];

    let hashes :Vec<Vec<u8>> = vec![
        DIGEST.hash_leaf(&values[0]).as_ref().into(),
        DIGEST.hash_leaf(&values[1]).as_ref().into(),
        DIGEST.hash_leaf(&values[2]).as_ref().into(),
        DIGEST.hash_leaf(&values[3]).as_ref().into(),
    ];

    let tree = BloomMerkleTree::from_vec(DIGEST, values);
    // element contains in tree
    assert_eq!(tree.contains(hashes[0].clone()), true);
}

#[test]
fn test_valid_proof() {
    let values = (1..10).map(|x| vec![x]).collect::<Vec<_>>();
    let tree = BloomMerkleTree::from_vec(DIGEST, values.clone());
    let root_hash = tree.root_hash();

    for value in values {
        let proof = tree.gen_proof(value);
        let is_valid = proof.map(|p| p.validate(&root_hash)).unwrap_or(false);

        assert!(is_valid);
    }
}


#[test]
fn test_wrong_proof() {
    let values1 = vec![vec![1], vec![2], vec![3], vec![4]];
    let tree1 = BloomMerkleTree::from_vec(DIGEST, values1.clone());

    let values2 = vec![vec![4], vec![5], vec![6], vec![7]];
    let tree2 = BloomMerkleTree::from_vec(DIGEST, values2);

    let root_hash = tree2.root_hash();

    for value in values1 {
        let proof = tree1.gen_proof(value);
        let is_valid = proof.map(|p| p.validate(root_hash)).unwrap_or(false);

        assert_eq!(is_valid, false);
    }
}

#[test]
fn test_tree_iter() {
    let values = (1..10).map(|x| vec![x]).collect::<Vec<_>>();
    let tree = BloomMerkleTree::from_vec(DIGEST, values.clone());
    let iter = tree.iter().cloned().collect::<Vec<_>>();

    assert_eq!(values, iter);
}