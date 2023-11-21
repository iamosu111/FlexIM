use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use bloom_filter::BloomFilter;
use ring::digest::Algorithm;

use crate::tree::{Tree, LeavesIterator, LeavesIntoIterator};
use crate::{Positioned, bloom_filter};
use crate::proof::{Lemma, Proof};
use super::utils::*;

#[derive(Clone, Debug)]
pub struct BloomMerkleTree<T> {
    /// The hashing algorithm used by this Merkle tree
    pub algorithm: &'static Algorithm,

    /// The root of the inner binary tree
    root: Tree<T>,

    /// The height of the tree
    height: usize,

    /// The number of leaf nodes in the tree
    count: usize,
}

impl<T: PartialEq> PartialEq for BloomMerkleTree<T> {
    #[allow(trivial_casts)]
    fn eq(&self, other: &BloomMerkleTree<T>) -> bool {
        self.root == other.root
            && self.height == other.height
            && self.count == other.count
            && (self.algorithm as *const Algorithm) == (other.algorithm as *const Algorithm)
    }
}

impl<T: Eq> Eq for BloomMerkleTree<T> {}

impl<T: Ord> PartialOrd for BloomMerkleTree<T> {
    fn partial_cmp(&self, other: &BloomMerkleTree<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord> Ord for BloomMerkleTree<T> {
    #[allow(trivial_casts)]
    fn cmp(&self, other: &BloomMerkleTree<T>) -> Ordering {
        self.height
            .cmp(&other.height)
            .then(self.count.cmp(&other.count))
            .then((self.algorithm as *const Algorithm).cmp(&(other.algorithm as *const Algorithm)))
            .then_with(|| self.root.cmp(&other.root))
    }
}

impl<T: Hash> Hash for BloomMerkleTree<T> {
    #[allow(trivial_casts)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        <Tree<T> as Hash>::hash(&self.root, state);
        self.height.hash(state);
        self.count.hash(state);
        (self.algorithm as *const Algorithm).hash(state);
    }
}


// TODO: Construct from vec
impl<T> BloomMerkleTree<T> {
    /// Constructs a Merkle Tree from a vector of data blocks.
    /// Returns `None` if `values` is empty.
    pub fn from_vec(algorithm: &'static Algorithm, values: Vec<T>) -> Self
    where
        T: Hashable,
    {
        if values.is_empty() {
            return BloomMerkleTree {
                algorithm,
                root: Tree::empty(algorithm.hash_empty()),
                height: 0,
                count: 0,
            };
        }

        let count = values.len();
        let mut height = 0;
        let mut cur = Vec::with_capacity(count);

        for v in values {
            let leaf = Tree::new_leaf(algorithm, v);
            cur.push(leaf);
        }

        while cur.len() > 1 {
            let mut next = Vec::new();
            while !cur.is_empty() {
                if cur.len() == 1 {
                    next.push(cur.remove(0));
                } else {
                    let left = cur.remove(0);
                    let right = cur.remove(0);

                    let bloom_filter = Tree::union_bloom_filter(&left, &right);
                    let combined_hash = algorithm.hash_nodes(left.hash(), right.hash(), bloom_filter.to_bytes());

                    let node = Tree::Node {
                        hash: combined_hash.as_ref().into(),
                        left: Box::new(left),
                        right: Box::new(right),
                        bloom_filter,
                    };

                    next.push(node);
                }
            }

            height += 1;

            cur = next;
        }

        debug_assert!(cur.len() == 1);

        let root = cur.remove(0);

        BloomMerkleTree {
            algorithm,
            root,
            height,
            count,
        }
    }


    /// Returns the root hash of Merkle tree
    pub fn root_hash(&self) -> &Vec<u8> {
        self.root.hash()
    }

    /// Returns the height of Merkle tree
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the number of leaves in the Merkle tree
    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns whether the Merkle tree is empty or not
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Generate an inclusion proof for the given value.
    /// Returns `None` if the given value is not found in the tree.
    pub fn gen_proof(&self, value: T) -> Option<Proof<T>>
    where
        T: Hashable,
    {
        let root_hash = self.root_hash().clone();
        let leaf_hash = self.algorithm.hash_leaf(&value);

        Lemma::new(&self.root, leaf_hash.as_ref())
            .map(|position_lemma| match position_lemma {
                Positioned::Leaf(ref lemma) => lemma.clone(),
                Positioned::Node(ref lemma) => lemma.clone(),})
            .map(|lemma|Proof::new(self.algorithm, root_hash, lemma, value))
    }

    /// Creates an `Iterator` over the values contained in this Merkle tree.
    pub fn iter(&self) -> LeavesIterator<T> {
        self.root.iter()
    }

    pub fn contains(&self, value: T) -> bool 
    where
        T: Hash,
    {
        match &self.root {
            Tree::Empty { .. } => false,
            Tree::Leaf { .. } => false,
            Tree::Node { bloom_filter, .. } => bloom_filter.contains(&value)
        }
    }
}


impl<T> IntoIterator for BloomMerkleTree<T> {
    type Item = T;
    type IntoIter = LeavesIntoIterator<T>;

    /// Creates a consuming iterator, that is, one that moves each value out of the Merkle tree.
    /// The tree cannot be used after calling this.
    fn into_iter(self) -> Self::IntoIter {
        self.root.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a BloomMerkleTree<T> {
    type Item = &'a T;
    type IntoIter = LeavesIterator<'a, T>;

    /// Creates a borrowing `Iterator` over the values contained in this Merkle tree.
    fn into_iter(self) -> Self::IntoIter {
        self.root.iter()
    }
}