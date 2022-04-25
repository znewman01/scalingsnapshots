//TODO: add hash
use smtree::index::TreeIndex;
use smtree::node_template::HashNodeSmt;
use smtree::pad_secret::ALL_ZEROS_SECRET;
use smtree::proof::MerkleProof;
use smtree::traits::{InclusionProvable, ProofExtractable, Serializable};
use smtree::tree::SparseMerkleTree;
use std::collections::HashMap;

use authenticator::{ClientSnapshot, Revision};

use crate::{
    authenticator,
    log::PackageId,
    util::{DataSize, DataSized},
};

static TREE_HEIGHT: usize = 256;
type Node = HashNodeSmt<blake3::Hasher>;
type Root = <Node as ProofExtractable>::ProofNode;

#[derive(Default, Clone, Debug)]
pub struct Snapshot {
    root: Root,
}

impl Snapshot {
    pub fn new(root: Root) -> Self {
        Self { root }
    }
}

impl DataSized for Snapshot {
    fn size(&self) -> DataSize {
        DataSize::from_bytes(32) // blake3 output size
    }
}

#[derive(Debug, Clone)]
pub struct Proof(MerkleProof<Node>);

impl DataSized for Proof {
    fn size(&self) -> DataSize {
        let len: usize = self.0.serialize().len();
        DataSize::from_bytes(len.try_into().expect("it really shouldn't be that big"))
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
        let leaf = Node::new(hash(&revision.0.to_be_bytes()).to_vec());
        let idxs = proof.0.get_indexes();
        if idxs.len() != 1 {
            return false;
        }
        if idxs[0] != expected_index {
            return false;
        }
        if !proof.0.verify(&leaf, &self.root) {
            return false;
        }
        true
    }
}

/// An authenticator as-in vanilla TUF.
#[derive(Default, Debug)]
pub struct Authenticator {
    tree: SparseMerkleTree<Node>,
    revisions: HashMap<PackageId, Revision>,
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

    fn publish(&mut self, package: &PackageId) -> () {
        let entry = self.revisions.entry(package.clone());
        let mut revision = entry.or_insert_with(Revision::default);
        revision.0 += 1;

        let idx = TreeIndex::new(TREE_HEIGHT, hash(package.0.as_bytes()));
        let node = Node::new(hash(&revision.0.to_be_bytes()).to_vec());
        self.tree.update(&idx, node, &ALL_ZEROS_SECRET);
    }

    fn request_file(
        &self,
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

        (*revision, Proof(proof))
    }
}

impl DataSized for Authenticator {
    fn size(&self) -> DataSize {
        // TODO: better to serialize then figure out the size?
        // also gzip?
        let mut snapshot_size: u64 =
            TryInto::try_into(std::mem::size_of::<Self>()).expect("Not that big");
        for (package_id, revision) in self.revisions.iter() {
            snapshot_size += package_id.size().bytes();
            snapshot_size += revision.size().bytes();
        }

        let mut tree_size = 0;
        for _ in itertools::chain!(
            self.tree.get_leaves().into_iter(),
            self.tree.get_internals().into_iter(),
            self.tree.get_paddings().into_iter(),
        ) {
            tree_size += 32; // blake3 output fixed
        }

        DataSize::from_bytes(snapshot_size + tree_size)
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}
