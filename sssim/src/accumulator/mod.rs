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

    // TODO: rationalize prove_append_only's
    #[must_use]
    fn prove_append_only_from_vec(
        &self,
        other: &[Integer],
    ) -> <Self::Digest as Digest>::AppendOnlyWitness;

    #[must_use]
    fn prove_append_only(&self, other: &Self) -> <Self::Digest as Digest>::AppendOnlyWitness;

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
