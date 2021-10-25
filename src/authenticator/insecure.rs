use authenticator::ClientSnapshot;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use crate::authenticator;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug)]
pub struct Snapshot {}

impl ClientSnapshot for Snapshot {
    type Digest = ();
    type Id = ();
    type Diff = ();

    fn id(&self) -> Self::Id {}

    fn update(&mut self, _diff: Self::Diff) {}

    fn digest(&self) -> Self::Digest {}

    fn check_no_rollback(&self, _: &Self::Diff) -> bool {
        true
    }
}

/// An insecure authenticator.
///
/// Useful for testing.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug)]
pub struct Authenticator {}

impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        _snapshot_id: &<Snapshot as ClientSnapshot>::Id,
    ) -> <Snapshot as ClientSnapshot>::Diff {
    }

    // TODO: storage usage
    // (when implementing for vanilla TUF, use spreadsheet to estimate this)
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
