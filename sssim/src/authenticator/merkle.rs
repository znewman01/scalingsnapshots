//! Merkle binary prefix tree authenticator.
use std::fmt::Debug;

use derivative::Derivative;
use digest::Output;
use digest_hash::EndianUpdate;
use serde::Serialize;

use crate::util::DataSized;

use crate::primitives::merkle::{Digest, Hasher, ObjectHasher, Proof, Tree};
use crate::util::FixedDataSized;
use crate::{authenticator::Revision, log::PackageId, util::byte, util::Information};

#[derive(Clone, Debug, Serialize)]
#[serde(bound = "Output<H>: Serialize")]
pub struct Snapshot<H: Hasher> {
    digest: Digest<PackageId, H>,
}

impl<H: Hasher> FixedDataSized for Snapshot<H> {
    fn fixed_size() -> Information {
        Information::new::<byte>(<H as Hasher>::output_size())
    }
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Default(bound = "Tree<PackageId, Revision, H>: Default"))]
pub struct Authenticator<H: Hasher> {
    tree: Tree<PackageId, Revision, H>,
}

impl<H: Hasher> DataSized for Authenticator<H>
where
    Tree<PackageId, Revision, H>: DataSized,
{
    fn size(&self) -> Information {
        self.tree.size()
    }
}

#[allow(unused_variables)]
impl<H: Hasher> super::Authenticator for Authenticator<H>
where
    ObjectHasher<H>: Hasher<OutputSize = H::OutputSize> + EndianUpdate,
    Output<H>: Copy,
    H: Debug,
    Snapshot<H>: Clone + Serialize,
    Proof<Revision, H>: Serialize + Clone + DataSized,
{
    type ClientSnapshot = Snapshot<H>;
    type Id = ();
    type Diff = Snapshot<H>;
    type Proof = Proof<Revision, H>;

    fn name() -> &'static str {
        "merkle_bpt"
    }

    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<Self::Diff> {
        Some(self.get_metadata())
    }

    fn get_metadata(&self) -> Self::ClientSnapshot {
        let digest = self.tree.digest();
        Snapshot { digest }
    }

    fn publish(&mut self, package: PackageId) {
        let revision = self
            .tree
            .values()
            .get(&package)
            .map(Revision::incremented)
            .unwrap_or_else(Default::default);
        self.tree.insert(package, revision);
    }

    fn request_file(
        &mut self,
        snapshot_id: Self::Id,
        package: &PackageId,
    ) -> (Revision, Self::Proof) {
        let proof = self.tree.lookup(package).cloned();
        let revision = proof
            .get_unverified()
            .expect("should never get a file request for a missing package");
        (*revision, proof)
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut tree: Tree<_, _, _> = Default::default();
        for p in packages {
            tree.insert(p, Revision::default());
        }
        Self { tree }
    }

    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id {}

    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff) {
        *snapshot = diff
    }

    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool {
        true
    }

    fn verify_membership(
        snapshot: &Self::ClientSnapshot,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        snapshot.digest.verify(package_id, proof) == Ok(Some(revision))
    }

    fn cdn_size(&self) -> Information {
        self.size()
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use crate::authenticator::tests;

    proptest! {
        #[ignore] // TODO(test): fix tests::update
        #[test]
        fn update((authenticator, snapshot) in (any::<Authenticator>(), any::<Snapshot>())) {
            tests::update(snapshot, &authenticator)?;
        }
    }
}
*/
