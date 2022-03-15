mod insecure;
pub use insecure::Authenticator as Insecure;

use crate::{
    log::{File, PackageRelease},
    util::DataSized,
};

// Client-side state
pub trait ClientSnapshot {
    /// Everything the client needs to verify proofs about metadata.
    type Digest;
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

    fn digest(&self) -> Self::Digest;

    /// Verify that applying `diff` doesn't roll back any targets.
    fn check_no_rollback(&self, diff: &Self::Diff) -> bool;

    /// Verify that `file` *is* in this snapshot.
    fn verify_membership(&self, file: File, proof: Self::Proof) -> bool;
}

// Server-side state
pub trait Authenticator<S: ClientSnapshot>: DataSized {
    fn refresh_metadata(&self, snapshot_id: &S::Id) -> S::Diff;

    fn publish(&mut self, release: &PackageRelease) -> ();

    fn request_file(&self, snapshot_id: S::Id, file: File) -> S::Proof;
}

#[cfg(test)]
pub(in crate) mod tests {
    use super::*;
    use proptest::prelude::*;

    pub fn update<S, A>(mut client_state: S, server_state: A) -> Result<(), TestCaseError>
    where
        S: ClientSnapshot,
        A: Authenticator<S>,
    {
        let id = client_state.id();
        let diff = server_state.refresh_metadata(&id);
        prop_assert!(
            client_state.check_no_rollback(&diff),
            "Server should never cause rollback."
        );
        client_state.update(diff);
        Ok(())
    }
    // test refresh metadata
}
