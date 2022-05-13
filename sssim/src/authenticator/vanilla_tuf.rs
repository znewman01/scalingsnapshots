//! TODO: this is Mercury, not vanilla TUF.
//! Vanilla TUF downloads *all* metadata:
//! - snapshot: map from filename to HASH of targets metadata
//! - targets metadata, which includes version number
//!   - only new targets, because duh
//!
//! TODO: delta variant of Mercury
use std::collections::HashMap;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use serde::Serialize;

use crate::{
    authenticator::{self, ClientSnapshot, Revision},
    log::PackageId,
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Clone, Serialize)]
pub struct Metadata {
    revision: Revision,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    packages: HashMap<PackageId, Metadata>,
    id: u64,
}

/// The vanilla TUF client snapshot contains *all* the snapshot state.
impl ClientSnapshot for Snapshot {
    type Id = u64;
    type Diff = Snapshot;
    type Proof = ();

    fn id(&self) -> Self::Id {
        self.id
    }

    fn update(&mut self, diff: Self::Diff) {
        self.packages = diff.packages;
        self.id = diff.id
    }

    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        for (package_id, metadata) in &self.packages {
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

    fn verify_membership(
        &self,
        package_id: &PackageId,
        revision: Revision,
        _: Self::Proof,
    ) -> bool {
        if let Some(metadata) = self.packages.get(package_id) {
            metadata.revision == revision
        } else {
            false
        }
    }
}

/// An authenticator as-in vanilla TUF.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Serialize)]
pub struct Authenticator {
    snapshot: Snapshot,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        if snapshot_id == self.snapshot.id() {
            // already up to date
            return None;
        }
        Some(self.snapshot.clone())
    }

    fn publish(&mut self, package: &PackageId) {
        self.snapshot.id += 1;
        let entry = self.snapshot.packages.entry(package.clone());
        let mut metadata = entry.or_insert_with(Metadata::default);
        metadata.revision.0 += 1;
    }

    fn request_file(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot as ClientSnapshot>::Proof) {
        let metadata = self
            .snapshot
            .packages
            .get(package)
            .expect("Should never get a request for a package that's missing.");
        (metadata.revision, ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authenticator::tests;

    proptest! {
        #[ignore] // TODO: fix tests::update
        #[test]
        fn update((authenticator, snapshot) in (any::<Authenticator>(), any::<Snapshot>())) {
            tests::update(snapshot, &authenticator)?;
        }
    }
}
