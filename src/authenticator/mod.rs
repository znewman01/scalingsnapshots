// Client-side state
pub trait Snapshot {
    type Digest;
    type Id;

    fn id(&self) -> Self::Id;

    fn set_digest(&mut self, state: Self::Digest);

    fn digest(&self) -> Self::Digest;

    fn check_no_rollback(&self, new: &Self) -> bool;
}

// Server-side state
pub trait Authenticator<S: Snapshot> {
    fn refresh_metadata(&self, snapshot_id: &S::Id) -> S;
}

mod insecure;
pub use insecure::Authenticator as Insecure;
