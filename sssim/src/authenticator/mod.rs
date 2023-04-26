mod hackage;
mod insecure;
mod mercury_diff;
//. mod merkle;
mod rsa;
mod sparse_merkle;
mod vanilla_tuf;

use std::{
    collections::HashMap,
    num::{NonZeroU64, TryFromIntError},
};

use serde::Serialize;

use crate::{
    accumulator::rsa::RsaAccumulator,
    util::{FixedDataSized, Information},
};

use crate::primitives::RsaGroup;
pub use hackage::Authenticator as Hackage;
pub use insecure::Authenticator as Insecure;
pub use mercury_diff::Authenticator as MercuryDiff;
// pub use mercury_hash::Authenticator as MercuryHash;
// pub use mercury_hash_diff::Authenticator as MercuryHashDiff;
pub use sparse_merkle::Authenticator as SparseMerkle;
pub type Rsa = rsa::Authenticator<RsaAccumulator<RsaGroup>>;
pub type RsaPool = rsa::PoolAuthenticator<RsaAccumulator<RsaGroup>>;
pub use vanilla_tuf::Authenticator as VanillaTuf;

use crate::{log::PackageId, util::byte, util::DataSized};

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Revision(pub NonZeroU64);

impl FixedDataSized for Revision {
    fn fixed_size() -> Information {
        Information::new::<byte>(8)
    }
}

impl From<usize> for Revision {
    fn from(revision: usize) -> Self {
        Self(u64::try_from(revision).unwrap().try_into().unwrap())
    }
}

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

impl Revision {
    fn increment(&mut self) {
        self.0 = NonZeroU64::try_from(self.0.get() + 1).unwrap();
    }

    fn decrement(&mut self) -> Result<(), TryFromIntError> {
        self.0 = NonZeroU64::try_from(self.0.get() + 1)?;
        Ok(())
    }
}

impl std::ops::Add<usize> for Revision {
    type Output = Revision;

    fn add(self, rhs: usize) -> Self::Output {
        Self::from(self.0.checked_add(rhs.try_into().unwrap()).unwrap())
    }
}

impl std::ops::Sub<usize> for Revision {
    type Output = Revision;

    fn sub(self, rhs: usize) -> Self::Output {
        let num = self.0.get().checked_sub(rhs.try_into().unwrap()).unwrap();
        Self::from(NonZeroU64::try_from(num).unwrap())
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

// Server-side state
pub trait Authenticator: DataSized {
    /// Client-side state
    type ClientSnapshot: DataSized + Clone;
    /// Identifies what digest we have, so
    ///   (1) the server can give us proofs against it
    ///   (2) the server can update us appropriately
    type Id;
    /// Information needed to update our client snapshot.
    type Diff: Serialize + DataSized + Clone;
    /// Information neeeded to verify file membership in the snapshot.
    type Proof: Serialize + DataSized + Clone;

    fn name() -> &'static str;

    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<Self::Diff>;

    fn get_metadata(&self) -> Self::ClientSnapshot;

    fn publish(&mut self, package: PackageId);

    // TODO(maybe): we can always assume that snapshot_id is latest
    fn request_file(
        &mut self,
        snapshot_id: Self::Id,
        package: &PackageId,
    ) -> (Revision, Self::Proof);

    fn batch_import(packages: Vec<PackageId>) -> Self;

    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id;

    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff);

    /// Verify that applying `diff` doesn't roll back any targets.
    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool;

    /// Verify that `file` *is* in this snapshot.
    fn verify_membership(
        snapshot: &Self::ClientSnapshot,
        package: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool;

    fn cdn_size(&self) -> Information;
}

pub trait BatchAuthenticator: Authenticator {
    type BatchProof: Serialize + DataSized + Clone;

    fn batch_prove(
        &mut self,
        packages: Vec<PackageId>,
    ) -> (HashMap<PackageId, u32>, Self::BatchProof);

    fn batch_verify(
        snapshot: &Self::ClientSnapshot,
        packages: HashMap<PackageId, u32>,
        proof: Self::BatchProof,
    ) -> bool;
}

pub trait PoolAuthenticator: Authenticator {
    fn batch_process(&mut self);
}

/*
#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn update<S, A>(mut client_state: S, server_state: &A) -> Result<(), TestCaseError>
    where
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
*/
