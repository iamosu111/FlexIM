use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use crate::bloom_filter::{SeededBloomFilter, BloomFilter};
use ring::digest::Algorithm;

use super::utils::HashUtils;
use crate::tree::Tree;
// TODO: ADD to support multiple values exsit proof, value = Vec<T>
/// An inclusion proof represent the fact that a `value` is a member
/// of a `MerkleTree` with root hash `root_hash`, and hash function `algorithm`.
#[cfg_attr(feature = "serialization-serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Proof<T> {
    /// The hashing algorithm used in the original `MerkleTree`
    #[cfg_attr(feature = "serialization-serde", serde(with = "algorithm_serde"))]
    pub algorithm: &'static Algorithm,

    /// The hash of the root of the original `MerkleTree`
    pub root_hash: Vec<u8>,

    /// The first `Lemma` of the `Proof`
    pub lemma: Lemma,

    /// The value concerned by this `Proof`
    pub value: T,
}


impl<T: PartialEq> PartialEq for Proof<T> {
    fn eq(&self, other: &Proof<T>) -> bool {
        self.root_hash == other.root_hash && self.lemma == other.lemma && self.value == other.value
    }
}

impl<T: Eq> Eq for Proof<T> {}

impl<T: Ord> PartialOrd for Proof<T> {
    fn partial_cmp(&self, other: &Proof<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord> Ord for Proof<T> {
    fn cmp(&self, other: &Proof<T>) -> Ordering {
        self.root_hash
            .cmp(&other.root_hash)
            .then(self.value.cmp(&other.value))
            .then_with(|| self.lemma.cmp(&other.lemma))
    }
}

impl<T: Hash> Hash for Proof<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root_hash.hash(state);
        self.lemma.hash(state);
        self.value.hash(state);
    }
}

impl<T> Proof<T> {
    /// Constructs a new `Proof`
    pub fn new(algorithm: &'static Algorithm, root_hash: Vec<u8>, lemma: Lemma, value: T) -> Self {
        Proof {
            algorithm,
            root_hash,
            lemma,
            value,
        }
    }

    /// Checks whether this inclusion proof is well-formed,
    /// and whether its root hash matches the given `root_hash`.
    pub fn validate(&self, root_hash: &[u8]) -> bool {
        if self.root_hash != root_hash || self.lemma.node_hash != root_hash {
            return false;
        }

        self.lemma.validate(self.algorithm)
    }

    // /// Returns the index of this proof's value, given the total number of items in the tree.
    // ///
    // /// # Panics
    // ///
    // /// Panics if the proof is malformed. Call `validate` first.
    // pub fn index(&self, count: usize) -> usize {
    //     self.lemma.index(count)
    // }
}

/// A `Lemma` holds the hash of a node, a left_lemma of node, and a right_lemma
/// `node_hash ==  left_lemma.node_hash|right_lemma.node_hash|hash(bloom_filter)` 
/// 
#[cfg_attr(feature = "serialization-serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lemma {
    pub node_hash: Vec<u8>,
    // pub sub_lemmas: Option<Vec<Box<Lemma>>>,
    pub left_lemma: Option<Box<Positioned<Lemma>>>,
    pub right_lemma: Option<Box<Positioned<Lemma>>>,
    pub bloom_filter: Option<SeededBloomFilter>,
}
// Contains, node_hash & sub_lemmas & bloom_filter
// Not_cantains, node_hash & bloom_filter

impl Lemma {

    /// Attempts to generate a proof that the a value with hash `needle` is a
    /// member of the given `tree`.
    pub fn new<T>(tree: &Tree<T>, needle: &[u8]) -> Option<Positioned<Lemma>> {
        match *tree {
            Tree::Empty { .. } => None,

            Tree::Leaf { ref hash, .. } 
            => { Some(Positioned::Leaf(Lemma {
                node_hash: hash.clone(),
                left_lemma: None,
                right_lemma: None,
                bloom_filter: None,
            }))},

            Tree::Node {
                ref hash,
                ref left,
                ref right,
                ref bloom_filter,
            } => Lemma::new_tree_proof(hash, needle, left, right, bloom_filter),
        }
    }


    /// ## Contains
    /// 
    /// Call Lemma:new(left) & Lemma:new(right) to constuct Lemma
    /// 
    /// ## Not Cantains
    /// 
    /// Lemma {node_hash, bloom_filter}
    /// 
    fn new_tree_proof<T>(
        hash: &[u8],
        needle: &[u8],
        left: &Tree<T>,
        right: &Tree<T>,
        bloom_filter: &SeededBloomFilter,
    ) -> Option<Positioned<Lemma>> {
        if bloom_filter.contains(&needle.to_vec()) {
            let left_lemma = Lemma::new(left,needle);
            let right_lemma = Lemma::new(right, needle);
            Some( Positioned::Node(
                Lemma { 
                node_hash: hash.into(),
                left_lemma: Box::new(left_lemma.unwrap()).into(),
                right_lemma: Box::new(right_lemma.unwrap()).into(),
                bloom_filter: Some(bloom_filter.clone())
            }))
        } else {
            Some( Positioned::Node(
                Lemma { 
                node_hash: hash.into(),
                left_lemma: None,
                right_lemma: None,
                bloom_filter: Some(bloom_filter.clone())
            }))
        }

    }

    fn validate(&self, algorithm: &'static Algorithm) -> bool {
        let mut result = true;
        let left_result = match self.left_lemma {
            None => {return true},
            Some(ref temp) => {
                match *temp.to_owned() {
                    Positioned::Leaf(ref lemma) => {lemma.node_hash.clone()},
                    Positioned::Node(ref lemma) => {
                        result = result && lemma.validate(algorithm);
                        lemma.node_hash.clone()},
                }
            }
        };
        let right_result = match self.right_lemma {
            None => {return true},
            Some(ref temp) => {
                match *temp.to_owned() {
                    Positioned::Leaf(ref lemma) => {lemma.node_hash.clone()},
                    Positioned::Node(ref lemma) => {
                        result = result && lemma.validate(algorithm);
                        lemma.node_hash.clone()},
                }
            }
        };
        let combined_hash = algorithm.hash_nodes(&left_result, &right_result, self.bloom_filter.as_ref().unwrap().to_bytes());
        result = result && (combined_hash.as_ref() == self.node_hash.as_slice());


        result
    }

}


/// Tags a value so that we know from which Types of a `Node` (if any) it was found.
#[cfg_attr(feature = "serialization-serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Positioned<T> {
    /// leaf node
    Leaf(T),

    /// inner node
    Node(T),
}
