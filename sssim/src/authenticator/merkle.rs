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

use authenticator::{ClientSnapshot, Revision};

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

#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    #[serde(serialize_with = "smtree_serialize")]
    root: Root,
}

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

/// The vanilla TUF client snapshot contains *all* the snapshot state.
impl ClientSnapshot for Snapshot {
    type Id = Root;
    type Diff = Self;
    type Proof = Proof;

    fn id(&self) -> Self::Id {
        self.root.clone()
    }

    fn update(&mut self, diff: Self::Diff) {
        self.root = diff.root
    }

    fn check_no_rollback(&self, _diff: &Self::Diff) -> bool {
        true
    }

    fn verify_membership(
        &self,
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
        if !proof.inner.verify(&leaf, &self.root) {
            return false;
        }
        true
    }
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
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
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
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot as ClientSnapshot>::Proof) {
        let revision = self
            .revisions
            .get(package)
            .expect("Should never get a request for a package that's missing.");
        let idx = TreeIndex::new(TREE_HEIGHT, hash(package.0.as_bytes()));
        let proof = MerkleProof::<Node>::generate_inclusion_proof(&self.tree, &[idx])
            .expect("Proof generation failed.");

        (*revision, proof.into())
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
