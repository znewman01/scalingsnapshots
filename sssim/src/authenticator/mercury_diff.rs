use std::collections::HashMap;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use crate::{
    authenticator::{self, ClientSnapshot, Revision},
    log::PackageId,
    util::{DataSize, DataSized},
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Clone, Copy)]
pub struct Metadata {
    revision: Revision,
}

impl DataSized for Metadata {
    fn size(&self) -> DataSize {
        DataSize::from_bytes(std::mem::size_of::<Self>().try_into().unwrap())
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug)]
pub struct Snapshot {
    packages: HashMap<PackageId, Metadata>,
    id: u64,
}

impl DataSized for Snapshot {
    fn size(&self) -> DataSize {
        // TODO: better to serialize then figure out the size?
        // also gzip?
        let mut size: u64 = TryInto::try_into(std::mem::size_of::<Self>()).expect("Not that big");
        for (package_id, metadata) in self.packages.iter() {
            // TODO: only add if new since last update
            size += package_id.size().bytes();
            size += metadata.size().bytes();
        }
        DataSize::from_bytes(size)
    }
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
        for (package_id, metadata) in diff.packages.iter() {
            if let Some(mut old_metadata) = self.packages.get_mut(&package_id) {
                old_metadata.revision.0 = metadata.revision.0;
            } else {
                self.packages.insert(package_id.clone(), metadata.clone());
            }
        }
        self.id = diff.id
    }

    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        for (package_id, metadata) in self.packages.iter() {
            let new_metadata = match diff.packages.get(&package_id) {
                None => {
                    return false;
                }
                Some(m) => m,
            };
            if !(new_metadata.revision >= metadata.revision) {
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
        if let Some(metadata) = self.packages.get(&package_id) {
            if metadata.revision != revision {
                false
            } else {
                true
            }
        } else {
            false
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug)]
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
        for (package_id, metadata) in self.snapshot.packages.iter() {
            if prev_snapshot.packages[package_id].revision != metadata.revision {
                if let Some(mut diff_metadata) = diff.packages.get_mut(&package_id) {
                    diff_metadata.revision.0 = metadata.revision.0;
                } else {
                    diff.packages.insert(package_id.clone(), metadata.clone());
                }
            }
        }

        Some(diff)
    }

    fn publish(&mut self, package: &PackageId) -> () {
        self.snapshots
            .insert(self.snapshot.id.clone(), self.snapshot.clone());
        let new_snapshot = self.snapshots.get_mut(&self.snapshot.id);
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

impl DataSized for Authenticator {
    fn size(&self) -> DataSize {
        self.snapshot.size()
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
            tests::update(snapshot, authenticator)?;
        }
    }
}