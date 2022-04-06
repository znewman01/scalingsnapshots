use std::collections::HashMap;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use crate::{
    authenticator::{self, ClientSnapshot, Revision},
    log::PackageId,
    util::{DataSize, DataSized},
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Clone)]
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
            self.packages[package_id] = *metadata;
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
    snapshot: Snapshot,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    // find the packages that have changed
    fn refresh_metadata(
        &self,
        snapshot: Snapshot
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        if snapshot.id() == self.snapshot.id() {
            // already up to date
            return None;
        }
        let diff = Snapshot{id:self.snapshot.id(), packages:HashMap::new()};
        for (package_id, metadata) in self.snapshot.packages.iter() {
            if snapshot.packages[package_id].revision != metadata.revision {
                diff.packages[package_id] = *metadata;
            }
        }

        Some(diff)
    }

    fn publish(&mut self, package: &PackageId) -> () {
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
