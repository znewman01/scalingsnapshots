use std::collections::HashMap;

use rug::Integer;

pub mod rsa;
// pub mod rsa_optimized; // todo: rename to caching

//pub use rsa_optimized::CachingAccumulator;

pub use rsa::{RsaAccumulator, RsaAccumulatorDigest};

use crate::multiset::MultiSet;

pub trait Digest {
    type Witness;
    type AppendOnlyWitness;

    #[must_use]
    fn verify(&self, member: &Integer, revision: u32, witness: Self::Witness) -> bool;

    #[must_use]
    fn verify_append_only(&self, proof: &Self::AppendOnlyWitness, new_state: &Self) -> bool;
}

pub trait Accumulator {
    type Digest: Digest;

    fn digest(&self) -> &Self::Digest;

    fn increment(&mut self, member: Integer);

    #[must_use]
    fn prove_append_only(
        &self,
        other: &Self::Digest,
    ) -> <Self::Digest as Digest>::AppendOnlyWitness;

    #[must_use]
    fn prove(
        &mut self,
        member: &Integer,
        revision: u32,
    ) -> Option<<Self::Digest as Digest>::Witness>;

    #[must_use]
    fn get(&self, member: &Integer) -> u32;

    fn import(multiset: MultiSet<Integer>) -> Self;
}

pub trait BatchDigest: Digest {
    type BatchWitness;

    fn verify_batch(&self, members: &HashMap<Integer, u32>, witness: Self::BatchWitness) -> bool;
}

pub trait BatchAccumulator: Accumulator
where
    <Self as Accumulator>::Digest: BatchDigest,
{
    fn increment_batch(&mut self, members: Vec<Integer>) {
        members.into_iter().for_each(|m| self.increment(m));
    }

    fn prove_batch(
        &mut self,
        entries: &HashMap<Integer, u32>,
    ) -> <<Self as Accumulator>::Digest as BatchDigest>::BatchWitness;
}
