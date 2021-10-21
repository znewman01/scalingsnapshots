use crate::authenticator;

#[derive(Default, Debug)]
pub struct Snapshot {}

impl authenticator::Snapshot for Snapshot {
    type Digest = ();
    type Id = ();

    fn id(&self) -> Self::Id {}

    fn set_digest(&mut self, _state: Self::Digest) {}

    fn digest(&self) -> Self::Digest {}

    fn check_no_rollback(&self, _: &Self) -> bool {
        true
    }
}

/// An insecure authenticator.
///
/// Useful for testing.
#[derive(Default, Debug)]
pub struct Authenticator {}

impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        _snapshot_id: &<Snapshot as authenticator::Snapshot>::Id,
    ) -> Snapshot {
        Snapshot::default()
    }
}
