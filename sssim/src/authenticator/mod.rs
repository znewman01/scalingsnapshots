mod hackage;
mod insecure;
mod mercury_diff;
mod mercury_hash;
mod mercury_hash_diff;
mod merkle;
mod rsa;
mod vanilla_tuf;

use std::num::NonZeroU64;

use serde::Serialize;

use crate::accumulator::rsa::RsaAccumulator;

pub use hackage::Authenticator as Hackage;
pub use insecure::Authenticator as Insecure;
pub use mercury_diff::Authenticator as MercuryDiff;
pub use mercury_hash::Authenticator as MercuryHash;
pub use mercury_hash_diff::Authenticator as MercuryHashDiff;
pub use merkle::Authenticator as Merkle;
pub use rsa::Authenticator as Accumulator;
pub type Rsa = Accumulator<RsaAccumulator>;
pub use vanilla_tuf::Authenticator as VanillaTuf;

use crate::{log::PackageId, util::DataSized};

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Revision(pub NonZeroU64);

impl From<NonZeroU64> for Revision {
    fn from(revision: NonZeroU64) -> Self {
        Self(revision)
    }
}

impl Default for Revision {
    fn default() -> Self {
        Self::from(NonZeroU64::try_from(1).expect("1 > 0"))
    }
}

#[cfg(test)]
impl Arbitrary for Revision {
    type Strategy = BoxedStrategy<Revision>;
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<u64>()
            .prop_filter_map("nonzero", |x| NonZeroU64::try_from(x).ok())
            .prop_map(Revision::from)
            .boxed()
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Hash(pub [u64; 4]);

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
    fn name() -> &'static str;

    fn refresh_metadata(&self, snapshot_id: S::Id) -> Option<S::Diff>;

    fn get_metadata(&self) -> S;

    fn publish(&mut self, package: PackageId);

    // TODO: we can always assume that snapshot_id is latest
    fn request_file(&mut self, snapshot_id: S::Id, package: &PackageId) -> (Revision, S::Proof);

    fn batch_import(packages: Vec<PackageId>) -> Self;
}

#[cfg(test)]
pub(in crate) mod tests {
    use super::*;

    // TODO: should take server state, publish operations.
    // 1. init client state
    // 2. publish
    // 3. refresh_metadata
    // 4. check_no_rollback
    pub fn update<S, A>(mut client_state: S, server_state: &A) -> Result<(), TestCaseError>
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
