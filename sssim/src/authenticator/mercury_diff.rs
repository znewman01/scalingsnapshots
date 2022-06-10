use std::collections::HashMap;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use serde::Serialize;

use crate::{
    authenticator::{self, ClientSnapshot, Revision},
    log::PackageId,
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Clone, Copy, Serialize)]
pub struct Metadata {
    revision: Revision,
}

impl From<Revision> for Metadata {
    fn from(revision: Revision) -> Self {
        Self { revision }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    packages: HashMap<PackageId, Metadata>,
    id: u64,
}

/// The mercury TUF client snapshot contains *all* the snapshot state.
impl ClientSnapshot for Snapshot {
    type Id = u64;
    type Diff = Snapshot;
    type Proof = ();

    fn id(&self) -> Self::Id {
        self.id
    }

    // only update changed packages
    fn update(&mut self, diff: Self::Diff) {
        for (package_id, metadata) in &diff.packages {
            if let Some(mut old_metadata) = self.packages.get_mut(package_id) {
                old_metadata.revision.0 = metadata.revision.0;
            } else {
                self.packages.insert(package_id.clone(), *metadata);
            }
        }
        self.id = diff.id;
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

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Serialize)]
pub struct Authenticator {
    snapshots: HashMap<u64, Snapshot>,
    snapshot: Snapshot,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    // find the packages that have changed
    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        if snapshot_id == self.snapshot.id() {
            // already up to date
            return None;
        }
        let prev_snapshot = &self.snapshots[&snapshot_id];
        let mut diff = Snapshot {
            id: self.snapshot.id(),
            packages: HashMap::new(),
        };
        for (package_id, metadata) in &self.snapshot.packages {
            if prev_snapshot.packages[package_id].revision != metadata.revision {
                if let Some(mut diff_metadata) = diff.packages.get_mut(package_id) {
                    diff_metadata.revision.0 = metadata.revision.0;
                } else {
                    diff.packages.insert(package_id.clone(), *metadata);
                }
            }
        }

        Some(diff)
    }

    fn publish(&mut self, package: PackageId) {
        self.snapshots
            .insert(self.snapshot.id, self.snapshot.clone());
        let new_snapshot = self.snapshots.get_mut(&self.snapshot.id);
        self.snapshot.id += 1;
        let entry = self
            .snapshot
            .packages
            .entry(package)
            .and_modify(|m| m.revision.0 = m.revision.0.checked_add(1).unwrap())
            .or_insert_with(Metadata::default);
    }

    fn request_file(
        &mut self,
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
