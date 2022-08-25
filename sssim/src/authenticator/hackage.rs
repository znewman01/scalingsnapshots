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
#[derive(Clone, Default, Debug, Serialize)]
pub struct Snapshot {
    package_revisions: HashMap<PackageId, Revision>,
    /// How far into the log has this client read?
    high_water_mark: usize,
}

impl ClientSnapshot for Snapshot {
    type Id = usize;
    type Diff = Vec<(PackageId, Revision)>;
    type Proof = ();

    fn id(&self) -> Self::Id {
        self.high_water_mark
    }

    fn update(&mut self, diff: Self::Diff) {
        self.high_water_mark += diff.len();
        for (package_id, new_revision) in diff.into_iter() {
            self.package_revisions.insert(package_id, new_revision);
        }
    }

    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        // TODO: combine with update
        for (package_id, new_revision) in diff.iter() {
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
#[derive(Clone, Default, Debug, Serialize)]
pub struct Authenticator {
    log: Vec<(PackageId, Revision)>,
    package_revisions: HashMap<PackageId, Revision>,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn name() -> &'static str {
        "hackage"
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut auth = Self::default();
        for p in packages {
            auth.publish(p);
        }
        auth
    }

    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        let diff_len = match self.log.len().checked_sub(snapshot_id) {
            Some(len) => len,
            None => return None, // snapshot_id is in the future!
        };

        let mut diff = Vec::new();
        diff.extend_from_slice(&self.log[snapshot_id..]);
        Some(diff)
    }

    fn publish(&mut self, package: PackageId) {
        let revision = self
            .package_revisions
            .entry(package.clone())
            .and_modify(|r| r.0 = r.0.checked_add(1).unwrap())
            .or_insert_with(Revision::default);
        self.log.push((package, *revision));
    }

    fn request_file(
        &mut self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot as ClientSnapshot>::Proof) {
        let revision = self
            .package_revisions
            .get(package)
            .expect("Should never get a request for a package that's missing");
        (*revision, ())
    }

    fn get_metadata(&self) -> Snapshot {
        Snapshot {
            package_revisions: self.package_revisions.clone(),
            high_water_mark: self.log.len(),
        }
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
