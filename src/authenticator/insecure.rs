use authenticator::ClientSnapshot;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use crate::{
    authenticator,
    log::{File, PackageRelease},
    util::{DataSize, DataSized},
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug)]
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

    fn verify_membership(&self, _: File, _: Self::Proof) -> bool {
        true
    }
}

/// An insecure authenticator.
///
/// Useful for testing.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug)]
pub struct Authenticator {}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        _snapshot_id: &<Snapshot as ClientSnapshot>::Id,
    ) -> <Snapshot as ClientSnapshot>::Diff {
    }

    fn publish(&mut self, release: &PackageRelease) -> () {}

    fn request_file(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        file: File,
    ) -> <Snapshot as ClientSnapshot>::Proof {
        ()
    }
}

impl DataSized for Authenticator {
    fn size(&self) -> DataSize {
        DataSize::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authenticator::tests;

    proptest! {
        #[test]
        fn update((authenticator, snapshot) in (any::<Authenticator>(), any::<Snapshot>())) {
            tests::update(snapshot, authenticator)?;
        }
    }
}
