use crate::util::DataSized;
use std::collections::HashMap;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use serde::Serialize;

use crate::{
    authenticator::Revision, log::PackageId, util::DataSizeFromSerialize, util::Information,
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

/// The mercury TUF client snapshot contains *all* the snapshot state.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    packages: HashMap<PackageId, Metadata>,
    id: u64,
}

impl DataSizeFromSerialize for Snapshot {}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, Default, Debug, Serialize)]
pub struct Authenticator {
    snapshots: HashMap<u64, Snapshot>,
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
        "mercury_diff"
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut snapshot = Snapshot::default();
        for p in packages {
            snapshot.packages.insert(p, Metadata::default());
        }
        let mut snapshots = HashMap::<u64, Snapshot>::new();
        snapshots.insert(0, Snapshot::default());
        //snapshots.insert(1, snapshot.clone());
        snapshot.id += 1;
        Self {
            snapshots,
            snapshot,
        }
    }

    // find the packages that have changed
    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<Self::Diff> {
        if snapshot_id == Self::id(&self.snapshot) {
            // already up to date
            return None;
        }
        let prev_snapshot = &self.snapshots[&snapshot_id];
        let mut diff = Snapshot {
            id: Self::id(&self.snapshot),
            packages: HashMap::new(),
        };
        for (package_id, metadata) in &self.snapshot.packages {
            match prev_snapshot.packages.get(package_id) {
                Some(m) if m.revision == metadata.revision => {
                    // do nothing; the package was up-to-date in the previous snapshot
                }
                _ => {
                    diff.packages.insert(package_id.clone(), *metadata);
                }
            }
        }

        Some(diff)
    }

    fn publish(&mut self, package: PackageId) {
        // TODO(meh): this is slow, consider using log data structure
        // also consider using immutable map
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

    // only update changed packages
    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff) {
        for (package_id, metadata) in &diff.packages {
            if let Some(mut old_metadata) = snapshot.packages.get_mut(package_id) {
                old_metadata.revision.0 = metadata.revision.0;
            } else {
                snapshot.packages.insert(package_id.clone(), *metadata);
            }
        }
        snapshot.id = diff.id;
    }

    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool {
        for (package_id, metadata) in &diff.packages {
            if let Some(old_metadata) = snapshot.packages.get(&package_id) {
                if metadata.revision < old_metadata.revision {
                    return false;
                }
            }
        }
        true
    }

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
        // TODO(meh): consider using log data structure or immutable map
        let mut size = self.snapshot.size();

        for (key, value) in &self.snapshots {
            size += key.size();
            size += value.size();
        }
        size
    }
}

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
