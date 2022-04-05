mod insecure;
mod merkle;
mod vanilla_tuf;
pub use insecure::Authenticator as Insecure;
pub use vanilla_tuf::Authenticator as VanillaTuf;

use crate::{
    log::PackageId,
    util::{DataSize, DataSized},
};

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Revision(pub u64);

impl From<u64> for Revision {
    fn from(revision: u64) -> Self {
        Self(revision)
    }
}

impl DataSized for Revision {
    fn size(&self) -> DataSize {
        DataSize::from_bytes(
            std::mem::size_of::<Self>()
                .try_into()
                .expect("not that big"),
        )
    }
}

// Client-side state
pub trait ClientSnapshot {
    /// Identifies what digest we have, so
    ///   (1) the server can give us proofs against it
    ///   (2) the server can update us appropriately
    type Id;
    /// Information needed to update our client snapshot.
    type Diff: DataSized + Clone;
    /// Information neeeded to verify file membership in the snapshot.
    type Proof: DataSized + Clone;

    fn id(&self) -> Self::Id;

    fn update(&mut self, diff: Self::Diff);

    /// Verify that applying `diff` doesn't roll back any targets.
    fn check_no_rollback(&self, diff: &Self::Diff) -> bool;

    /// Verify that `file` *is* in this snapshot.
    fn verify_membership(
        &self,
        package: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool;
}

// Server-side state
pub trait Authenticator<S: ClientSnapshot>: DataSized {
    fn refresh_metadata(&self, snapshot_id: S::Id) -> Option<S::Diff>;

    fn publish(&mut self, package: &PackageId) -> ();

    fn request_file(&self, snapshot_id: S::Id, package: &PackageId) -> (Revision, S::Proof);
}

#[cfg(test)]
pub(in crate) mod tests {
    use super::*;

    // TODO: should take server state, publish operations.
    // 1. init client state
    // 2. publish
    // 3. refresh_metadata
    // 4. check_no_rollback
    pub fn update<S, A>(mut client_state: S, server_state: A) -> Result<(), TestCaseError>
    where
        S: ClientSnapshot,
        A: Authenticator<S>,
    {
        let id = client_state.id();
        let maybe_diff = server_state.refresh_metadata(id);
        if let Some(diff) = maybe_diff {
            prop_assert!(
                client_state.check_no_rollback(&diff),
                "Server should never cause rollback."
            );
            client_state.update(diff);
        }
        Ok(())
    }
    // test refresh metadata
}