use std::collections::HashMap;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use serde::Serialize;

use crate::{
    authenticator::{self, ClientSnapshot, Hash, Revision},
    log::PackageId,
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Clone, Copy, Serialize)]
pub struct Metadata {
    revision: Revision,
    hash: Hash,
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
        for (package_id, metadata) in &diff.packages {
            if let Some(old_metadata) = self.packages.get(&package_id) {
                if metadata.revision < old_metadata.revision {
                    return false;
                }
            }
        }
        true
    }

    // Could validate the hash here
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
#[derive(Clone, Default, Debug, Serialize)]
pub struct Authenticator {
    snapshots: HashMap<u64, Snapshot>,
    snapshot: Snapshot,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn name() -> &'static str {
        "mercury_hash_diff"
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut snapshot = Snapshot::default();
        for p in packages {
            snapshot.packages.insert(p, Metadata::default());
        }
        let mut snapshots = HashMap::<u64, Snapshot>::new();
        snapshots.insert(0, Snapshot::default());
        snapshot.id += 1;
        Self {
            snapshots,
            snapshot,
        }
    }

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
            match prev_snapshot.packages.get(package_id) {
                Some(m) if m.revision == metadata.revision => {
                    // do nothing; package was already up-to-date in the previous snapshot
                }
                _ => {
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
        self.snapshot
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
