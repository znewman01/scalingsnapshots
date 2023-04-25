//! Merkp_le binary prefix tree (trie) for representing a dictionary.
//!
//! Follows CONIKS.
use crate::bit_twiddling::*;
use derivative::Derivative;
use digest::Output;
use digest_hash::{EndianUpdate, Hash};
use std::{collections::HashMap, marker::PhantomData};
use thiserror::Error;

pub trait Hasher = digest::Digest;
type ObjectHasher<H> = digest_hash::LittleEndian<H>;

const NONCE: [u8; 4] = [0, 0, 0, 0];
const NODE_TYPE_EMPTY: [u8; 4] = [0, 0, 0, 1];
const NODE_TYPE_LEAF: [u8; 4] = [0, 0, 0, 2];

fn hash<D: Hash, H: Hasher>(data: &D) -> Output<ObjectHasher<H>>
where
    ObjectHasher<H>: Hasher + EndianUpdate,
{
    let mut hasher = ObjectHasher::<H>::new();
    data.hash(&mut hasher);
    hasher.finalize()
}

/// A direction in the tree.
#[derive(Debug, Clone, Copy)]
enum Direction {
    Left,
    Right,
}

use Direction::*;

impl From<bool> for Direction {
    fn from(value: bool) -> Self {
        match value {
            false => Left,
            true => Right,
        }
    }
}

#[derive(Debug, Clone)]
struct LeafData<H: Hasher> {
    /// H(key)
    key_index: Output<H>,
    /// How deep in the prefix tree? (0-indexed)
    depth: usize,
    /// H(value).
    value_hash: Output<H>,
}

impl<H: Hasher> LeafData<H> {
    fn new(index: Output<H>, depth: usize, value: Output<H>) -> Self {
        Self {
            key_index: index,
            depth,
            value_hash: value,
        }
    }

    fn hash(&self) -> Output<ObjectHasher<H>>
    where
        ObjectHasher<H>: Hasher + EndianUpdate,
    {
        let mut hasher = ObjectHasher::<H>::new();
        hasher.update(NODE_TYPE_LEAF);
        hasher.update(NONCE);
        self.key_index.hash(&mut hasher);
        u64::try_from(self.depth)
            .expect("64 bit platform")
            .hash(&mut hasher);
        self.value_hash.hash(&mut hasher);
        hasher.finalize()
    }

    fn from_key_value<K: Hash, V: Hash>(depth: usize, key: &K, value: &V) -> Self
    where
        V: Hash,
        ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
    {
        Self::new(hash::<_, H>(key), depth, hash::<_, H>(value))
    }
}

#[derive(Debug, Clone)]
struct EmptyData<H: Hasher> {
    /// How deep in the prefix tree?
    depth: usize,
    /// The unique prefix.
    prefix: Output<H>,
}

impl<H: Hasher> EmptyData<H> {
    fn new(depth: usize, prefix: Output<H>) -> Self {
        debug_assert_eq!(mask(&prefix, depth), prefix);
        Self { depth, prefix }
    }

    fn hash(&self) -> Output<ObjectHasher<H>>
    where
        ObjectHasher<H>: Hasher + EndianUpdate,
    {
        let mut hasher = ObjectHasher::<H>::new();
        hasher.update(NODE_TYPE_EMPTY);
        hasher.update(NONCE);
        self.prefix.hash(&mut hasher);
        u64::try_from(self.depth)
            .expect("64 bit platform")
            .hash(&mut hasher);
        hasher.finalize()
    }
}

fn hash_interior<H: Hasher>(left: &Output<H>, right: &Output<H>) -> Output<ObjectHasher<H>>
where
    ObjectHasher<H>: Hasher + EndianUpdate,
{
    let mut hasher = ObjectHasher::<H>::new();
    left.hash(&mut hasher);
    right.hash(&mut hasher);
    hasher.finalize()
}

#[derive(Debug)]
struct InteriorData<H: Hasher> {
    left: Box<Node<H>>,
    right: Box<Node<H>>,
}

impl<H: Hasher> InteriorData<H> {
    fn new(left: Box<Node<H>>, right: Box<Node<H>>) -> Self {
        Self { left, right }
    }

    fn into_child(self, direction: Direction) -> Box<Node<H>> {
        match direction {
            Left => self.left,
            Right => self.right,
        }
    }

    fn child(&self, direction: Direction) -> &Box<Node<H>> {
        match direction {
            Left => &self.left,
            Right => &self.right,
        }
    }

    fn child_mut(&mut self, direction: Direction) -> &mut Node<H> {
        match direction {
            Left => self.left.as_mut(),
            Right => self.right.as_mut(),
        }
    }

    fn sibling(&self, direction: Direction) -> &Node<H> {
        match direction {
            Left => &self.right,
            Right => &self.left,
        }
    }
}

impl<H: Hasher> InteriorData<H> {
    fn hash(&self) -> Output<H>
    where
        ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
    {
        hash_interior::<H>(&self.left.hash, &self.right.hash)
    }
}

#[derive(Debug)]
enum NodeData<H: Hasher> {
    Leaf(LeafData<H>),
    Empty(EmptyData<H>),
    Interior(InteriorData<H>),
}

#[derive(Debug)]
struct Node<H: Hasher> {
    inner: NodeData<H>,
    hash: Output<H>,
}

impl<H: Hasher> From<LeafData<H>> for Node<H>
where
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
{
    fn from(data: LeafData<H>) -> Self {
        let hash = data.hash();
        let inner = NodeData::Leaf(data);
        Self { inner, hash }
    }
}

impl<H: Hasher> From<EmptyData<H>> for Node<H>
where
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
{
    fn from(data: EmptyData<H>) -> Self {
        let hash = data.hash();
        let inner = NodeData::Empty(data);
        Self { inner, hash }
    }
}

impl<H: Hasher> From<InteriorData<H>> for Node<H>
where
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
{
    fn from(inner: InteriorData<H>) -> Self {
        let hash = hash_interior::<H>(&inner.left.hash, &inner.right.hash);
        let inner = NodeData::Interior(inner);
        Self { inner, hash }
    }
}

impl<H: Hasher> Node<H>
where
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
{
    fn leaf(index: Output<H>, depth: usize, value: Output<H>) -> Self {
        LeafData::new(index, depth, value).into()
    }

    fn empty(depth: usize, prefix: Output<H>) -> Self {
        EmptyData::new(depth, prefix).into()
    }

    fn interior(left: Box<Node<H>>, right: Box<Node<H>>) -> Self {
        InteriorData::new(left, right).into()
    }

    fn interior_for_direction(
        child: Box<Node<H>>,
        sibling: Box<Node<H>>,
        direction: Direction,
    ) -> Self {
        match direction {
            Left => Self::interior(child, sibling),
            Right => Self::interior(sibling, child),
        }
    }

    fn rehash(&mut self) {
        self.hash = match &self.inner {
            NodeData::Leaf(data) => data.hash(),
            NodeData::Empty(data) => data.hash(),
            NodeData::Interior(inner) => inner.hash(),
        };
    }
}

/// Binary Merkle Prefix Tree.
#[derive(Debug)]
struct Tree<K: Hash, V: Hash, H: Hasher> {
    /// The root node of a Merkle prefix tree for the given keys/values.
    root: Box<Node<H>>,
    /// This is where the actual keys and values are stored.
    values: HashMap<K, V>,
}

impl<K: Hash, V: Hash, H: Hasher> Default for Tree<K, V, H>
where
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
{
    fn default() -> Self {
        let root = Box::new(Node::empty(0, Default::default()));
        Self {
            root,
            values: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
enum ProofInner<V, H: Hasher> {
    Member(V),
    NonMemberEmpty(Output<H>),
    NonMemberLeaf {
        leaf_index: Output<H>,
        value_hash: Output<H>,
    },
}

use ProofInner::*;

#[derive(Debug, Clone)]
struct Proof<V, H: Hasher> {
    /// Root to leaf.
    sibling_hashes: Vec<Output<H>>,
    /// H(key)
    key_index: Output<H>,
    inner: ProofInner<V, H>,
}

impl<V: Clone, H: Hasher> Proof<&V, H> {
    pub fn cloned(self) -> Proof<V, H> {
        use ProofInner::*;

        let inner = match self.inner {
            Member(value) => Member(value.clone()),
            NonMemberEmpty(x) => NonMemberEmpty(x),
            NonMemberLeaf {
                leaf_index,
                value_hash,
            } => NonMemberLeaf {
                leaf_index,
                value_hash,
            },
        };
        Proof { inner, ..self }
    }
}

#[derive(Debug)]
struct Digest<K, H: Hasher> {
    value: Output<H>,
    _key: PhantomData<K>,
}

fn insert_recursive<H: Hasher>(
    current_node: &mut Node<H>,
    depth: usize,
    index: Output<H>,
    value_hash: Output<H>,
) where
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
    Output<H>: Copy,
{
    let new_node = match &mut current_node.inner {
        NodeData::Leaf(data) => {
            debug_assert_eq!(data.depth, depth);
            if data.key_index == index {
                // We already had this key; just update its value (by replacing the leaf node).
                Some(Node::leaf(index, depth, value_hash))
            } else {
                // There's already a leaf there. Need to prepare interior nodes.
                debug_assert_eq!(mask(&data.key_index, depth), mask(&index, depth));

                // The existing leaf and the new leaf share a prefix (possibly
                // beyond `depth`). Find it.
                //
                // existing:  0 1 0 1 0 1 0 1
                // new_leaf:  0 1 0 1 0 0 0 1
                //                      ^shared_prefix_len
                let shared_prefix_len = shared_prefix_length(data.key_index, index);

                // There will be interior nodes from `depth` until
                // `shared_prefix_len`, at which point there will be the two
                // leaf nodes.
                let mut child = Box::new(Node::leaf(index, shared_prefix_len + 1, value_hash));
                let mut sibling = Box::new(Node::leaf(
                    data.key_index,
                    shared_prefix_len + 1,
                    data.value_hash,
                ));
                for i in ((depth + 1)..=shared_prefix_len).rev() {
                    let direction = Direction::from(get_bit_i(&index, i));
                    child = Box::new(Node::interior_for_direction(child, sibling, direction));

                    // Make the empty leaf for the next level up. It should
                    // differ from `index` at bit `i - 1`.
                    let mut other_index = mask(&index, i);
                    flip_bit_i(&mut other_index, i - 1);
                    sibling = Box::new(Node::empty(i, other_index));
                }

                // Create the interior node that will replace the existing leaf.
                let direction = Direction::from(get_bit_i(&index, depth));
                Some(Node::interior_for_direction(child, sibling, direction))
            }
        }
        NodeData::Empty(data) => {
            // Replace the empty node with a leaf node.
            debug_assert_eq!(data.depth, depth);
            debug_assert_eq!(mask(&data.prefix, depth), mask(&index, depth));
            Some(Node::leaf(index, depth, value_hash))
        }
        NodeData::Interior(inner) => {
            // Recurse down the tree. This node is unchanged (but will need to be rehashed).
            let direction = Direction::from(get_bit_i(&index, depth));
            insert_recursive(inner.child_mut(direction), depth + 1, index, value_hash);
            None
        }
    };

    // Replace the current node with `new_node`, if applicable.
    if let Some(new_node) = new_node {
        *current_node = new_node;
    }
    current_node.rehash();
}

impl<K: Hash, V: Hash, H: Hasher> Tree<K, V, H>
where
    K: Eq + std::hash::Hash + std::fmt::Debug,
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
    Output<H>: Copy,
{
    fn digest(&self) -> Digest<K, H> {
        Digest {
            value: self.root.hash,
            _key: PhantomData,
        }
    }

    /// Look up the given key in the dictionary, along with a proof of correctness.
    fn lookup(&self, key: &K) -> Proof<&V, H> {
        let key_index = hash::<_, H>(key);
        let mut sibling_hashes = Vec::<Output<H>>::new();
        let mut depth = 0usize;
        let mut current_node = &self.root;

        loop {
            match &current_node.inner {
                NodeData::Leaf(data) => {
                    let inner = if data.key_index == key_index {
                        Member(self.values.get(key).expect("found!"))
                    } else {
                        NonMemberLeaf {
                            leaf_index: data.key_index.clone(),
                            value_hash: data.value_hash.clone(),
                        }
                    };
                    return Proof {
                        sibling_hashes,
                        key_index,
                        inner,
                    };
                }
                NodeData::Empty(data) => {
                    // Terminate: a membership proof for the Empty node convinces
                    debug_assert_eq!(mask(&data.prefix, depth), mask(&key_index, depth));
                    let inner = NonMemberEmpty(data.prefix);
                    return Proof {
                        sibling_hashes,
                        key_index,
                        inner,
                    };
                }
                NodeData::Interior(inner) => {
                    // Push a new sibling hash and go depeer.
                    let direction = Direction::from(get_bit_i(&key_index, depth));
                    sibling_hashes.push(inner.sibling(direction).hash);
                    current_node = inner.child(direction);
                }
            }
            depth += 1;
        }
    }

    fn insert(&mut self, key: K, value: V) {
        let index = hash::<_, H>(&key);
        let value_hash = hash::<_, H>(&value);

        insert_recursive(&mut self.root, 0usize, index, value_hash);

        self.values.insert(key, value);
    }
}

/// Verification of a Merkle BPT proof failed.
#[derive(Error, Debug, Derivative)]
#[derivative(PartialEq(bound = "Output<H>: PartialEq"))]
pub enum VerificationError<H: Hasher> {
    #[error(
        "index of leaf {leaf_index:?} did not match given key index {key_index:?} (depth {depth})"
    )]
    IndexMismatch {
        leaf_index: Output<H>,
        key_index: Output<H>,
        depth: usize,
    },
    #[error("non-member proof provided, but indexes match completely: {0}")]
    UnexpectedIndexMatch(Output<H>),
    #[error("computed hash {computed:?} doesn't match expected hash {expected:?}")]
    HashMismatch {
        computed: Output<H>,
        expected: Output<H>,
    },
}

/// Checks that `leaf_index` is a valid leaf-node nonmembership proof for
/// `key_index` at `depth`.
fn check_valid_non_member_leaf<H: Hasher>(
    leaf_index: Output<H>,
    key_index: Output<H>,
    depth: usize,
) -> Result<(), VerificationError<H>> {
    // A leaf node with a matching prefix (up to `depth`) but *not* a
    // matching key convinces us that the key is missing.
    if mask(&leaf_index, depth) != mask(&key_index, depth) {
        return Err(VerificationError::IndexMismatch {
            leaf_index,
            key_index,
            depth,
        });
    }
    if leaf_index == key_index {
        return Err(VerificationError::UnexpectedIndexMatch(key_index));
    }
    Ok(())
}

/// Check that `leaf_index` is the index of a valid empty-node nonmembership
/// proof for `key_index` at `depth`.
fn check_valid_non_member_empty<H: Hasher>(
    leaf_index: Output<H>,
    key_index: Output<H>,
    depth: usize,
) -> Result<(), VerificationError<H>> {
    // An empty node with a matching prefix (up to depth) convinces
    // us that the key is missing.
    if mask(&leaf_index, depth) != mask(&key_index, depth) {
        return Err(VerificationError::IndexMismatch {
            leaf_index,
            key_index,
            depth,
        });
    }
    Ok(())
}

impl<K, H: Hasher> Digest<K, H> {
    /// Verify a lookup proof for key `key` (whether `key` is present or not), returning the result.
    fn verify<V>(&self, key: &K, result: Proof<V, H>) -> Result<Option<V>, VerificationError<H>>
    where
        K: Hash,
        V: Hash,
        ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
        Output<H>: Copy,
        H: std::fmt::Debug,
    {
        let mut depth = result.sibling_hashes.len();
        let key_index = hash(key);

        // Compute the hash of the "leaf" node, and check that the purported result makes sense.
        let (mut current_hash, value) = match result.inner {
            ProofInner::Member(value) => {
                let data = LeafData::new(key_index, depth, hash::<_, H>(&value));
                (data.hash(), Some(value))
            }
            ProofInner::NonMemberLeaf {
                leaf_index,
                value_hash,
            } => {
                check_valid_non_member_leaf(leaf_index, key_index, depth)?;
                let data = LeafData::new(leaf_index, depth, value_hash);
                (data.hash(), None)
            }
            ProofInner::NonMemberEmpty(leaf_index) => {
                check_valid_non_member_empty(leaf_index, key_index, depth)?;
                let data = EmptyData::new(depth, leaf_index);
                (data.hash(), None)
            }
        };

        // Recompute the hash from leaf to root.
        for sibling_hash in result.sibling_hashes.iter().rev() {
            depth -= 1;
            let direction = Direction::from(get_bit_i(&result.key_index, depth));
            current_hash = match direction {
                Left => hash_interior(&current_hash, sibling_hash),
                Right => hash_interior(sibling_hash, &current_hash),
            };
        }
        debug_assert_eq!(depth, 0);

        if current_hash != self.value {
            return Err(VerificationError::HashMismatch {
                computed: current_hash,
                expected: self.value,
            });
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest::sample::Index;

    type Key = u8;
    type Value = u8;
    type CRHF = sha3::Sha3_256;

    fn insertions() -> impl Strategy<Value = Vec<(Key, Value)>> {
        prop::collection::vec((any::<Key>(), any::<Value>()), 0..20)
    }

    proptest! {
        /// Tests that, after a sequence of insertions, the proof is valid and the result is correct.
        #[test]
        fn test_tree(insertions in insertions(), key: Key) {
            let mut tree = Tree::<Key, Value, CRHF>::default();
            // Use a hash map as a reference for the expected result after the given insertions.
            let mut map = HashMap::<Key, Value>::default();

            for (key, value) in insertions {
                tree.insert(key, value);
                map.insert(key, value);
            }

            let digest = tree.digest();
            let proof = tree.lookup(&key);
            assert_eq!(digest.verify(&key, proof), Ok(map.get(&key)));
        }

        /// Tests that verification fails if any bit in the tree digest is perturbed.
        #[test]
        fn test_tree_bad_digest(insertions in insertions(), key: Key, index: Index) {
            let mut tree = Tree::<Key, Value, CRHF>::default();
            let mut map = HashMap::<Key, Value>::default();

            for (key, value) in insertions {
                tree.insert(key, value);
                map.insert(key, value);
            }

            let mut digest = tree.digest();
            let bit = index.index(<CRHF as digest::Digest>::output_size() * 8);
            flip_bit_i(&mut digest.value, bit);

            let proof = tree.lookup(&key);
            assert!(digest.verify(&key, proof).is_err());
        }

        /// Tests that verification fails if a correct proof is given for an incorrect key.
        #[test]
        fn test_tree_wrong_key(insertions in insertions(), index: Index, other_key: Key) {
            prop_assume!(!insertions.is_empty());
            let key = insertions[index.index(insertions.len())].0;
            prop_assume!(key != other_key);

            let mut tree = Tree::<Key, Value, CRHF>::default();
            let mut map = HashMap::<Key, Value>::default();

            for (key, value) in insertions {
                tree.insert(key, value);
                map.insert(key, value);
            }

            let digest = tree.digest();
            let proof = tree.lookup(&key);
            assert!(digest.verify(&other_key, proof).is_err());
        }

        /// Tests that verification fails if the wrong value is included in the proof.
        #[test]
        fn test_tree_wrong_value(insertions in insertions(), index: Index, value_offset: Value) {
            prop_assume!(value_offset != 0);
            prop_assume!(!insertions.is_empty());
            let key = insertions[index.index(insertions.len())].0;

            let mut tree = Tree::<Key, Value, CRHF>::default();
            let mut map = HashMap::<Key, Value>::default();

            for (key, value) in insertions {
                tree.insert(key, value);
                map.insert(key, value);
            }

            let digest = tree.digest();
            let mut proof = tree.lookup(&key);
            let mut new_value = value_offset;
            if let ProofInner::Member(value) = proof.inner {
                new_value = u8::wrapping_add(new_value, *value);
                proof.inner = ProofInner::Member(&new_value);
            }
            assert!(digest.verify(&key, proof).is_err());
        }
    }
}
