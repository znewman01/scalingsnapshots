use std::{collections::HashMap, num::NonZeroU64};

use crate::{
    accumulator::{Accumulator, Digest},
    hash_to_prime::division_intractable_hash,
};
use rug::Integer;
use serde::Serialize;

use authenticator::{ClientSnapshot, Revision};

use crate::{authenticator, log::PackageId};

#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot<D: Digest> {
    rsa_state: D, // TODO: s/rsa//
}

impl<D: Digest> Snapshot<D> {
    pub fn new(rsa_state: D) -> Self {
        Self { rsa_state }
    }
}

impl<D> ClientSnapshot for Snapshot<D>
where
    D: Digest + Clone + Serialize,
    <D as Digest>::AppendOnlyWitness: Clone + Serialize,
    <D as Digest>::Witness: Clone + Serialize,
{
    type Id = D;
    type Diff = (Self, D::AppendOnlyWitness);
    type Proof = D::Witness;

    fn id(&self) -> Self::Id {
        self.rsa_state.clone()
    }

    fn update(&mut self, diff: Self::Diff) {
        let (new_snapshot, _) = diff;
        self.rsa_state = new_snapshot.rsa_state;
    }

    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        let (new_snapshot, proof) = diff;
        self.rsa_state
            .verify_append_only(proof, &new_snapshot.rsa_state)
    }

    fn verify_membership(
        &self,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        let encoded = bincode::serialize(package_id).unwrap();
        let prime = division_intractable_hash(&encoded, &crate::accumulator::rsa::MODULUS); // TODO
        if !self
            .rsa_state
            .verify(&prime, revision.0.get().try_into().unwrap(), proof)
        {
            return false;
        }
        true
    }
}

#[derive(Debug, Serialize)]
pub struct Authenticator<A>
where
    A: Accumulator,
    <A as Accumulator>::Digest: std::fmt::Debug + Serialize + Eq + PartialEq + std::hash::Hash,
{
    rsa_acc: A,
    log: Vec<Integer>,
    old_acc_idxs: HashMap<<A as Accumulator>::Digest, usize>, // TODO: consider giving this usize to the client in this snapshot
}

impl<A> Authenticator<A>
where
    A: Accumulator,
    <A as Accumulator>::Digest: std::hash::Hash
        + Eq
        + Clone
        + std::fmt::Debug
        + Serialize
        + Eq
        + PartialEq
        + std::hash::Hash,
{
    fn new(rsa_acc: A) -> Self {
        let mut old_acc_idxs: HashMap<<A as Accumulator>::Digest, usize> = Default::default();
        old_acc_idxs.insert(rsa_acc.digest().clone(), 0);
        Authenticator {
            rsa_acc,
            log: vec![],
            old_acc_idxs,
        }
    }
}

impl<A> Default for Authenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest: std::hash::Hash
        + Eq
        + Clone
        + std::fmt::Debug
        + Serialize
        + Eq
        + PartialEq
        + std::hash::Hash,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[allow(unused_variables)]
impl<A> authenticator::Authenticator<Snapshot<A::Digest>> for Authenticator<A>
where
    A: Accumulator + Serialize,
    <A as Accumulator>::Digest:
        Clone + Serialize + PartialEq + Eq + std::hash::Hash + std::fmt::Debug,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + Serialize,
{
    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot<A::Digest> as ClientSnapshot>::Id,
    ) -> Option<<Snapshot<A::Digest> as ClientSnapshot>::Diff> {
        if &snapshot_id == self.rsa_acc.digest() {
            return None;
        }
        let new_snapshot = Snapshot::new(self.rsa_acc.digest().clone());
        let old_rsa_acc_idx = match self.old_acc_idxs.get(&snapshot_id) {
            Some(o) => o,
            None => {
                return None;
            }
        };
        let proof = self
            .rsa_acc
            .prove_append_only_from_vec(&self.log[(*old_rsa_acc_idx + 1)..]);
        Some((new_snapshot, proof))
    }

    fn publish(&mut self, package: PackageId) -> () {
        let encoded = bincode::serialize(&package).unwrap();
        let prime = division_intractable_hash(&encoded, &crate::accumulator::rsa::MODULUS);
        self.rsa_acc.increment(prime.clone());
        self.old_acc_idxs
            .insert(self.rsa_acc.digest().clone(), self.log.len());
        self.log.push(prime);
    }

    fn request_file(
        &self,
        snapshot_id: <Snapshot<A::Digest> as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot<A::Digest> as ClientSnapshot>::Proof) {
        let encoded = bincode::serialize(package).unwrap();
        let prime = division_intractable_hash(&encoded, &crate::accumulator::rsa::MODULUS);

        let revision = self.rsa_acc.get(&prime);
        let proof = self.rsa_acc.prove(&prime, revision).expect("proof failed");

        let revision: NonZeroU64 = u64::from(revision).try_into().unwrap();
        (Revision::from(revision), proof)
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}
