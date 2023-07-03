use std::{collections::HashMap, fmt::Debug};

pub mod rsa;
// pub mod rsa_optimized; // todo: rename to caching

//pub use rsa_optimized::CachingAccumulator;

use crate::{multiset::MultiSet, primitives::Prime, util::Information};

pub trait Accumulator {
    type Digest: Clone + Debug;
    type Witness;
    type AppendOnlyWitness;
    type NonMembershipWitness;

    fn digest(&self) -> &Self::Digest;

    fn increment(&mut self, member: Prime);

    #[must_use]
    fn prove_append_only(&self, other: &Self::Digest) -> Self::AppendOnlyWitness;

    #[must_use]
    fn prove(&mut self, member: &Prime, revision: u32) -> Option<Self::Witness>;

    #[must_use]
    fn prove_nonmember(&mut self, value: &Prime) -> Option<Self::NonMembershipWitness>;

    #[must_use]
    fn get(&self, member: &Prime) -> u32;

    fn import(multiset: MultiSet<Prime>) -> Self;

    #[must_use]
    fn verify(digest: &Self::Digest, member: &Prime, revision: u32, witness: Self::Witness)
        -> bool;

    #[must_use]
    fn verify_append_only(
        digest: &Self::Digest,
        proof: &Self::AppendOnlyWitness,
        new_state: &Self::Digest,
    ) -> bool;

    fn cdn_size(&self) -> Information;
}

pub trait BatchAccumulator: Accumulator {
    type BatchDigest;
    type BatchWitness: Clone;

    fn increment_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        members: I,
    ) -> Option<<Self as Accumulator>::AppendOnlyWitness> {
        members.into_iter().for_each(|m| self.increment(m));
        None
    }

    fn prove_batch<I: IntoIterator<Item = Prime>>(
        &mut self,
        entries: I,
    ) -> (HashMap<Prime, u32>, Self::BatchWitness);

    fn verify_batch(
        digest: &Self::BatchDigest,
        members: &HashMap<Prime, u32>,
        witness: Self::BatchWitness,
    ) -> bool;
}
