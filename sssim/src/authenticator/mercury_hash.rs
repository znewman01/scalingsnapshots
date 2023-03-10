use std::collections::HashMap;

use crate::util::DataSized;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use serde::Serialize;

use crate::{
    authenticator::{Hash, Revision},
    log::PackageId,
    util::DataSizeFromSerialize,
    util::Information,
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Clone, Serialize)]
pub struct Metadata {
    revision: Revision,
    hash: Hash,
}

/// The mercury-hash client snapshot contains *all* the snapshot state.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    packages: HashMap<PackageId, Metadata>,
    id: u64,
}

impl DataSizeFromSerialize for Snapshot {}

/// An authenticator as-in mercury-hash.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, Default, Debug, Serialize)]
pub struct Authenticator {
    snapshot: Snapshot,
}

impl DataSizeFromSerialize for Authenticator {}

#[allow(unused_variables)]
impl super::Authenticator for Authenticator {
    type ClientSnapshot = Snapshot;
    type Id = u64;
    type Diff = Snapshot;
    type Proof = ();
    fn name() -> &'static str {
        "mercury_hash"
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut snapshot = Snapshot::default();
        for p in packages {
            snapshot.packages.insert(p, Metadata::default());
        }
        snapshot.id += 1;
        Self { snapshot }
    }

    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<Self::Diff> {
        if snapshot_id == Self::id(&self.snapshot) {
            // already up to date
            return None;
        }
        Some(self.snapshot.clone())
    }

    fn publish(&mut self, package: PackageId) {
        self.snapshot.id += 1;
        self.snapshot
            .packages
            .entry(package)
            .and_modify(|m| m.revision.0 = m.revision.0.checked_add(1).unwrap())
            .or_insert_with(Metadata::default);
    }

    fn request_file(
        &mut self,
        snapshot_id: Self::Id,
        package: &PackageId,
    ) -> (Revision, Self::Proof) {
        let metadata = self
            .snapshot
            .packages
            .get(package)
            .expect("Should never get a request for a package that's missing.");
        (metadata.revision, ())
    }

    fn get_metadata(&self) -> Snapshot {
        self.snapshot.clone()
    }

    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id {
        snapshot.id
    }

    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff) {
        snapshot.packages = diff.packages;
        snapshot.id = diff.id;
    }

    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool {
        for (package_id, metadata) in &snapshot.packages {
            let new_metadata = match diff.packages.get(package_id) {
                None => {
                    return false;
                }
                Some(m) => m,
            };
            if new_metadata.revision < metadata.revision {
                return false;
            }
        }
        true
    }

    // Could validate the hash here
    fn verify_membership(
        snapshot: &Self::ClientSnapshot,
        package_id: &PackageId,
        revision: Revision,
        _: Self::Proof,
    ) -> bool {
        if let Some(metadata) = snapshot.packages.get(package_id) {
            metadata.revision == revision
        } else {
            false
        }
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
