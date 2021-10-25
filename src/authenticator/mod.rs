// Client-side state
pub trait ClientSnapshot {
    /// Everything the client needs to verify proofs about metadata.
    type Digest;
    /// Identifies what digest we have, so
    ///   (1) the server can give us proofs against it
    ///   (2) the server can update us appropriately
    type Id;

    fn id(&self) -> Self::Id;

    fn set_digest(&mut self, state: Self::Digest);

    fn digest(&self) -> Self::Digest;

    fn check_no_rollback(&self, new: &Self) -> bool;
}

// Server-side state
pub trait Authenticator<S: ClientSnapshot> {
    fn refresh_metadata(&self, snapshot_id: &S::Id) -> S;
}

mod insecure;
pub use insecure::Authenticator as Insecure;
