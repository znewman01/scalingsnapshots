use std::collections::HashMap;

use authenticator::ClientSnapshot;
use serde::Serialize;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use crate::{
    authenticator::{self, Revision},
    log::PackageId,
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug)]
pub struct Snapshot {
    package_revisions: HashMap<PackageId, Revision>,
    hwm: usize,
}

impl ClientSnapshot for Snapshot {
    type Id = usize;
    type Diff = Vec<(PackageId, Revision)>;
    type Proof = ();

    fn id(&self) -> Self::Id {
        self.hwm
    }

    fn update(&mut self, diff: Self::Diff) {
        self.hwm += diff.len();
        for (package_id, new_revision) in diff.into_iter() {
            self.package_revisions.insert(package_id, new_revision);
        }
    }

    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        for (package_id, new_revision) in diff.into_iter() {
            let result = self.package_revisions.get(package_id);
            if matches!(result, Some(old_revision) if old_revision > new_revision) {
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
        matches!(self.package_revisions.get(package_id), Some(r) if r == &revision)
    }
}

/// A Hackage-style authenticator.
///
/// That is, an authenticator with a
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Serialize)]
pub struct Authenticator {
    log: Vec<(PackageId, Revision)>,
    package_revisions: HashMap<PackageId, Revision>,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        let diff_len = match snapshot_id.checked_sub(self.log.len()) {
            Some(len) => len,
            None => return None, // snapshot_id is in the future!
        };

        let mut diff = Vec::with_capacity(diff_len);
        diff.clone_from_slice(&self.log[snapshot_id..]);
        Some(diff)
    }

    fn publish(&mut self, package_id: &PackageId) {
        let mut revision = self
            .package_revisions
            .entry(package_id.clone())
            .or_insert(Revision::from(0));
        revision.0 += 1;
        self.log
            .push((package_id.clone(), self.package_revisions[package_id]))
    }

    fn request_file(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot as ClientSnapshot>::Proof) {
        let revision = self
            .package_revisions
            .get(package)
            .expect("Should never get a request for a package that's missing");
        (*revision, ())
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
