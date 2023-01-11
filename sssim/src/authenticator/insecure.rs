use serde::Serialize;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use crate::{authenticator::Revision, log::PackageId, util::DataSizeFromSerialize};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, Default, Debug, Serialize)]
pub struct Snapshot {}

impl DataSizeFromSerialize for Snapshot {}

/// An insecure authenticator.
///
/// Useful for testing.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, Default, Debug, Serialize)]
pub struct Authenticator {}

impl DataSizeFromSerialize for Authenticator {}

#[allow(unused_variables)]
impl super::Authenticator for Authenticator {
    type ClientSnapshot = Snapshot;
    type Id = ();
    type Diff = ();
    type Proof = ();

    fn name() -> &'static str {
        "insecure"
    }

    fn refresh_metadata(&self, _: Self::Id) -> Option<Self::Diff> {
        None
    }

    fn get_metadata(&self) -> Snapshot {
        Snapshot::default()
    }

    fn publish(&mut self, _: PackageId) {}

    fn request_file(&mut self, _: Self::Id, _: &PackageId) -> (Revision, Self::Proof) {
        (Revision::default(), ())
    }

    fn batch_import(packages: Vec<PackageId>) -> Self {
        Self {}
    }
    fn id(_: &Self::ClientSnapshot) -> Self::Id {}

    fn update(_: &mut Self::ClientSnapshot, _: Self::Diff) {}

    fn check_no_rollback(_: &Self::ClientSnapshot, _: &Self::Diff) -> bool {
        true
    }

    fn verify_membership(
        _: &Self::ClientSnapshot,
        _: &PackageId,
        _: Revision,
        _: Self::Proof,
    ) -> bool {
        true
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
