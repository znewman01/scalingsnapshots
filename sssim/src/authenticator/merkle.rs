use serde::{Serialize, Serializer};
//TODO: add hash
use smtree::index::TreeIndex;
use smtree::node_template::HashNodeSmt;
use smtree::pad_secret::ALL_ZEROS_SECRET;
use smtree::proof::MerkleProof;
use smtree::traits::{InclusionProvable, ProofExtractable};
use smtree::tree::SparseMerkleTree;
use std::collections::HashMap;
use uom::si::information::byte;
use uom::si::u64::Information;
use uom::ConstZero;

use authenticator::Revision;

use crate::util::DataSizeFromSerialize;
use crate::{authenticator, log::PackageId, util::DataSized};

static TREE_HEIGHT: usize = 256;
type Node = HashNodeSmt<blake3::Hasher>;
type Root = <Node as ProofExtractable>::ProofNode;

fn smtree_serialize<S, V>(value: &V, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: smtree::traits::Serializable,
{
    s.serialize_bytes(&smtree::traits::Serializable::serialize(value))
}

/// The vanilla TUF client snapshot contains *all* the snapshot state.
#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    #[serde(serialize_with = "smtree_serialize")]
    root: Root,
}

impl DataSizeFromSerialize for Snapshot {}

impl Snapshot {
    pub fn new(root: Root) -> Self {
        Self { root }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Proof {
    #[serde(serialize_with = "smtree_serialize")]
    inner: MerkleProof<Node>,
}

impl DataSizeFromSerialize for Proof {}

impl From<MerkleProof<Node>> for Proof {
    fn from(inner: MerkleProof<Node>) -> Self {
        Proof { inner }
    }
}

fn hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(data);
    *hasher.finalize().as_bytes()
}

/// An authenticator as-in vanilla TUF.
#[derive(Debug)]
pub struct Authenticator {
    tree: SparseMerkleTree<Node>,
    revisions: HashMap<PackageId, Revision>,
}

impl Default for Authenticator {
    fn default() -> Self {
        Self {
            // SparseMerkleTree::default gives it a height of 0!!
            tree: SparseMerkleTree::new(TREE_HEIGHT),
            revisions: Default::default(),
        }
    }
}

#[allow(unused_variables)]
impl super::Authenticator for Authenticator {
    type ClientSnapshot = Snapshot;
    type Id = Root;
    type Diff = Snapshot;
    type Proof = Proof;

    fn name() -> &'static str {
        "merkle"
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut nodes = Vec::<(TreeIndex, Node)>::new();
        let mut revisions = HashMap::<PackageId, Revision>::new();
        for p in packages {
            let idx = TreeIndex::new(TREE_HEIGHT, hash(p.0.as_bytes()));
            let revision = Revision::default();
            revisions.insert(p, revision);
            let node = Node::new(hash(&revision.0.get().to_be_bytes()).to_vec());
            nodes.push((idx, node));
        }
        let mut tree = SparseMerkleTree::new(TREE_HEIGHT);
        nodes.sort_by_key(|(x, _)| *x);
        tree.build(&nodes, &ALL_ZEROS_SECRET);
        Self { tree, revisions }
    }

    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<Self::Diff> {
        let my_root = self.tree.get_root();
        if snapshot_id == my_root {
            return None;
        }
        Some(Snapshot::new(my_root))
    }

    fn publish(&mut self, package: PackageId) {
        let idx = TreeIndex::new(TREE_HEIGHT, hash(package.0.as_bytes()));
        let revision = self
            .revisions
            .entry(package)
            .and_modify(|r| r.0 = r.0.checked_add(1).unwrap())
            .or_insert_with(Revision::default);

        let node = Node::new(hash(&revision.0.get().to_be_bytes()).to_vec());
        self.tree.update(&idx, node, &ALL_ZEROS_SECRET);
    }

    fn request_file(
        &mut self,
        snapshot_id: Self::Id,
        package: &PackageId,
    ) -> (Revision, Self::Proof) {
        let revision = self
            .revisions
            .get(package)
            .expect("Should never get a request for a package that's missing.");
        let idx = TreeIndex::new(TREE_HEIGHT, hash(package.0.as_bytes()));
        let proof = MerkleProof::<Node>::generate_inclusion_proof(&self.tree, &[idx])
            .expect("Proof generation failed.");

        (*revision, proof.into())
    }

    fn get_metadata(&self) -> Snapshot {
        Snapshot::new(self.tree.get_root())
    }

    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id {
        snapshot.root.clone()
    }

    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff) {
        snapshot.root = diff.root
    }

    fn check_no_rollback(snapshot: &Self::ClientSnapshot, _diff: &Self::Diff) -> bool {
        true
    }

    fn verify_membership(
        snapshot: &Self::ClientSnapshot,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        let expected_index = TreeIndex::new(TREE_HEIGHT, hash(package_id.0.as_bytes()));
        let leaf = Node::new(hash(&revision.0.get().to_be_bytes()).to_vec());
        let idxs = proof.inner.get_indexes();
        if idxs.len() != 1 {
            return false;
        }
        if idxs[0] != expected_index {
            return false;
        }
        if !proof.inner.verify(&leaf, &snapshot.root) {
            return false;
        }
        true
    }
}

impl Clone for Authenticator {
    fn clone(&self) -> Self {
        let packages = self.revisions.keys().cloned().collect();
        <Self as super::Authenticator>::batch_import(packages)
    }
}

impl DataSized for Authenticator {
    fn size(&self) -> Information {
        // TODO: better to serialize then figure out the size?
        // also gzip?
        let mut snapshot_size = Information::new::<byte>(
            TryInto::try_into(std::mem::size_of::<Self>()).expect("Not that big"),
        );
        for (package_id, revision) in &self.revisions {
            snapshot_size += package_id.size();
            snapshot_size += revision.size();
        }

        let mut tree_size = Information::ZERO;
        for _ in itertools::chain!(
            self.tree.get_leaves().into_iter(),
            self.tree.get_internals().into_iter(),
            self.tree.get_paddings().into_iter(),
        ) {
            tree_size += Information::new::<byte>(32); // blake3 output fixed
        }

        snapshot_size + tree_size
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}
