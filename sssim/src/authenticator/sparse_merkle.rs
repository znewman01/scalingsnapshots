use crate::util::{byte, Information};
use digest::Digest;
use serde::{Serialize, Serializer};
use sha3::Sha3_256;
use smtree::index::TreeIndex;
use smtree::node_template::HashNodeSmt;
use smtree::pad_secret::ALL_ZEROS_SECRET;
use smtree::proof::MerkleProof;
use smtree::traits::{InclusionProvable, ProofExtractable};
use smtree::tree::SparseMerkleTree;
use std::collections::HashMap;
use uom::ConstZero;

use authenticator::Revision;

use crate::util::FixedDataSized;
use crate::{authenticator, log::PackageId, util::DataSized};

static TREE_HEIGHT: usize = 256;
type Node = HashNodeSmt<Sha3_256>;
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

impl DataSized for Snapshot {
    fn size(&self) -> crate::util::Information {
        Information::new::<byte>(Sha3_256::output_size())
    }
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

impl DataSized for Proof {
    fn size(&self) -> Information {
        let siblings_size = self.inner.get_path_siblings().len()
            * Information::new::<byte>(Sha3_256::output_size());
        let indexes_size = self.inner.get_indexes().len() * Information::new::<byte>(40); // each index has a usize and 32 u8s
        siblings_size + indexes_size
    }
}

impl From<MerkleProof<Node>> for Proof {
    fn from(inner: MerkleProof<Node>) -> Self {
        Proof { inner }
    }
}

fn hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[derive(Debug, Clone)]
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
        "sparse_merkle"
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
        std::thread::sleep(std::time::Duration::from_secs(30));
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

    fn cdn_size(&self) -> Information {
        let hash_size = Information::new::<byte>(32);
        let leaf_size = PackageId::fixed_size() + usize::fixed_size() + hash_size;
        let internal_size = 3 * usize::fixed_size() + hash_size;
        let num_leaves = self.tree.get_leaves().len();

        // assume worst case: all possible internal nodes, no padding
        leaf_size * num_leaves + internal_size * self.tree.get_nodes_num()
    }
}

impl DataSized for Authenticator {
    fn size(&self) -> Information {
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
            tree_size += Information::new::<byte>(Sha3_256::output_size());
        }

        snapshot_size + tree_size
    }
}

#[cfg(test)]
mod tests {
    // TODO(test): fix tests
}
