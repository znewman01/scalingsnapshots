//! The Mercury paper suggests using delta compression [RFC 3229] to transmit
//! the (package->revision) map to clients.
//!
//! This is an implementation simulating delta compression.
//!
//! It works by storing each version of the (package->revision) map (by their
//! index).
//!
//! This approach has a few problems not mentioned in Mercury:
//!
//! 1. O(p^2) storage for p packages: we store the current (package->revision)
//!    map, and a previous map which had all-but-the-lastest package in it,
//!    and...
//!
//! 2. It's not CDN-friendly: the server must compute diffs on-the-fly. (In
//!    principle a CDN *could* do this, but in practice none that I'm familiar
//!    with does.)
//!
//! There are a few alternatives worth considering:
//!
//! 1. Store a log of each update. When a client requests a catch-up from index
//!    i, serve them all the updates from i up until the present.
//!
//!    This *is* CDN-friendly now. It's inefficient in cases where the same
//!    package receieves many publication events, as each one appears in the
//!    diff.
//!
//!    (This is the "hackage" method in hackage.rs.)
//!
//! 2. Store a log of each update. When the client requests a catch-up from
//!    index i, the server computes the diff (basically, cutting out packages
//!    that appear multiple times).
//!
//!    This is much more storage-efficient than the approach in this file, but
//!    is still not CDN-friendly (even in principle).
//!
//! 3. Store the *deltas* for each prior index. Now, when a package is
//!    published, we must update each prior delta.
//!
//!    This is almost as expensive as storing each version of the map, but *is*
//!    CDN-friendly.
//!
//! 4. Store the deltas between *carefully selected indexes*.
//!
//!    That is, instead of a delta between each prior index and the current index,
//!    you can store a delta from a->b and another from b->c. Then, the client
//!    will combine a small number of deltas to get their update.
//!
//!    This is once again CDN-friendly, but has a big performance advantage over
//!    (3). The optimal way to do this is a skiplist, so that there are O(log u)
//!    deltas between any two indexes.
use crate::util::{DataSized, FixedDataSized};
use std::collections::HashMap;

#[cfg(test)]
use proptest_derive::Arbitrary;

use serde::Serialize;

use crate::{authenticator::Revision, log::PackageId, util::byte, util::Information};

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

impl FixedDataSized for Metadata {
    fn fixed_size() -> Information {
        Revision::fixed_size()
    }
}

/// The mercury TUF client snapshot contains *all* the snapshot state.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    packages: HashMap<PackageId, Metadata>,
    id: u64,
}

impl DataSized for Snapshot {
    fn size(&self) -> Information {
        self.id.size() + self.packages.size()
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, Default, Debug, Serialize)]
pub struct Authenticator {
    // TODO(meh): replace with a skiplist
    snapshots: HashMap<u64, Snapshot>,
    snapshot: Snapshot,
}

impl DataSized for Authenticator {
    fn size(&self) -> Information {
        let mut size = self.snapshot.size();
        for snapshot in self.snapshots.values() {
            size += Information::new::<byte>(8); // key
            size += snapshot.size();
        }
        size
    }
}

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
        // TODO(maybe): this is slow, consider using log data structure
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
            if let Some(old_metadata) = snapshot.packages.get(package_id) {
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
