//! TODO(maybe): this is Mercury, not vanilla TUF.
//! Vanilla TUF downloads *all* metadata:
//! - snapshot: map from filename to HASH of targets metadata
//! - targets metadata, which includes version number
//!   - only new targets, because duh
use std::collections::HashMap;

#[cfg(test)]
use proptest_derive::Arbitrary;

use serde::Serialize;

use crate::util::DataSized;

use crate::{authenticator::Revision, log::PackageId, util::byte, util::Information};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    packages: HashMap<PackageId, Revision>,
    id: u64,
}

impl DataSized for Snapshot {
    fn size(&self) -> Information {
        let mut size = Information::new::<byte>(8); // id
        let len: u64 = self.packages.len().try_into().unwrap();
        size += match self.packages.iter().next() {
            Some((k, v)) => (k.size() + v.size()) * len,
            None => Information::new::<byte>(0),
        };
        size
    }
}

/// The vanilla TUF client snapshot contains *all* the snapshot state.

/// An authenticator as-in vanilla TUF.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug, Serialize)]
pub struct Authenticator {
    snapshot: Snapshot,
}

impl DataSized for Authenticator {
    fn size(&self) -> Information {
        self.snapshot.size()
    }
}

#[allow(unused_variables)]
impl super::Authenticator for Authenticator {
    type ClientSnapshot = Snapshot;
    type Id = u64;
    type Diff = Snapshot;
    type Proof = ();

    fn name() -> &'static str {
        "vanilla_tuf"
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut snapshot = Snapshot::default();
        for p in packages {
            snapshot.packages.insert(p, Revision::default());
        }
        snapshot.id = 1;
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
            .and_modify(|r| r.0 = r.0.checked_add(1).unwrap())
            .or_insert_with(Revision::default);
    }

    fn request_file(
        &mut self,
        snapshot_id: Self::Id,
        package: &PackageId,
    ) -> (Revision, Self::Proof) {
        let revision = self
            .snapshot
            .packages
            .get(package)
            .expect("Should never get a request for a package that's missing.");
        (*revision, ())
    }

    fn get_metadata(&self) -> Snapshot {
        self.snapshot.clone()
    }

    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id {
        snapshot.id
    }

    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff) {
        snapshot.packages = diff.packages;
        snapshot.id = diff.id
    }

    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool {
        for (package_id, old_revision) in &snapshot.packages {
            let new_revision = match diff.packages.get(package_id) {
                None => {
                    return false;
                }
                Some(r) => r,
            };
            if new_revision < old_revision {
                return false;
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
        if let Some(old_revision) = snapshot.packages.get(package_id) {
            &revision == old_revision
        } else {
            false
        }
    }

    fn cdn_size(&self) -> Information {
        let mut size = self.snapshot.id.size();
        for (key, value) in &self.snapshot.packages {
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
