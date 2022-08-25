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
pub struct Snapshot {}

impl ClientSnapshot for Snapshot {
    type Id = ();
    type Diff = ();
    type Proof = ();

    fn id(&self) -> Self::Id {}

    fn update(&mut self, _diff: Self::Diff) {}

    fn check_no_rollback(&self, _: &Self::Diff) -> bool {
        true
    }

    fn verify_membership(&self, _: &PackageId, _: Revision, _: Self::Proof) -> bool {
        true
    }
}

/// An insecure authenticator.
///
/// Useful for testing.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, Default, Debug, Serialize)]
pub struct Authenticator {}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn name() -> &'static str {
        "insecure"
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        Self {}
    }

    fn refresh_metadata(
        &self,
        _snapshot_id: <Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        None
    }

    fn publish(&mut self, release: PackageId) {}

    fn request_file(
        &mut self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        file: &PackageId,
    ) -> (Revision, <Snapshot as ClientSnapshot>::Proof) {
        (Revision::default(), ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authenticator::tests;

    proptest! {
        #[test]
        fn update((authenticator, snapshot) in (any::<Authenticator>(), any::<Snapshot>())) {
            tests::update(snapshot, &authenticator)?;
        }
    }
}
