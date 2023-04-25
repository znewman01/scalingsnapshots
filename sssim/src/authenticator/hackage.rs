use std::collections::HashMap;

use crate::util::DataSized;
use serde::Serialize;

#[cfg(test)]
use proptest_derive::Arbitrary;

use crate::{authenticator::Revision, log::PackageId, util::byte, util::Information};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, Default, Debug, Serialize)]
pub struct Snapshot {
    package_revisions: HashMap<PackageId, Revision>,
    /// How far into the log has this client read?
    high_water_mark: usize,
}

impl DataSized for Snapshot {
    fn size(&self) -> Information {
        let mut size = Information::new::<byte>(8); // high_water_mark
        let len: u64 = self.package_revisions.len().try_into().unwrap();
        size += match self.package_revisions.iter().next() {
            Some((k, v)) => (k.size() + v.size()) * len,
            None => Information::new::<byte>(0),
        };
        size
    }
}

impl DataSized for Vec<(PackageId, Revision)> {
    fn size(&self) -> Information {
        let len: u64 = self.len().try_into().unwrap();
        match self.iter().next() {
            Some((k, v)) => (k.size() + v.size()) * len,
            None => Information::new::<byte>(0),
        }
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

impl DataSized for Authenticator {
    fn size(&self) -> Information {
        let mut size = self.log.size();
        let len: u64 = self.package_revisions.len().try_into().unwrap();
        size += match self.package_revisions.iter().next() {
            Some((k, v)) => (k.size() + v.size()) * len,
            None => Information::new::<byte>(0),
        };
        size
    }
}

#[allow(unused_variables)]
impl super::Authenticator for Authenticator {
    type ClientSnapshot = Snapshot;
    type Id = usize;
    type Diff = Vec<(PackageId, Revision)>;
    type Proof = ();

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

    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<Self::Diff> {
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
        snapshot_id: Self::Id,
        package: &PackageId,
    ) -> (Revision, Self::Proof) {
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

    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id {
        snapshot.high_water_mark
    }

    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff) {
        snapshot.high_water_mark += diff.len();
        for (package_id, new_revision) in diff.into_iter() {
            snapshot.package_revisions.insert(package_id, new_revision);
        }
    }

    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool {
        // TODO(maybe): combine with update
        for (package_id, new_revision) in diff.iter() {
            let result = snapshot.package_revisions.get(package_id);
            if matches!(result, Some(old_revision) if old_revision > new_revision) {
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
        matches!(snapshot.package_revisions.get(package_id), Some(r) if r == &revision)
    }

    fn cdn_size(&self) -> Information {
        self.log.size()
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
