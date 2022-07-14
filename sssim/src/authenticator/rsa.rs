use std::{collections::HashMap, num::NonZeroU64};

use crate::{
    accumulator::{Accumulator, Digest},
    hash_to_prime::hash_to_prime,
};
use rug::Integer;
use serde::Serialize;

use authenticator::{ClientSnapshot, Revision};

use crate::{authenticator, log::PackageId};

#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot<D: Digest> {
    digest: Option<D>,
}

impl<D: Digest> Snapshot<D> {
    pub fn new(digest: D) -> Self {
        Self { digest: Some(digest) }
    }
}

impl<D> ClientSnapshot for Snapshot<D>
where
    D: Digest + Clone + Serialize,
    <D as Digest>::AppendOnlyWitness: Clone + Serialize,
    <D as Digest>::Witness: Clone + Serialize,
{
    type Id = Option<D>;
    type Diff = (D, Option<D::AppendOnlyWitness>);
    type Proof = D::Witness;

    fn id(&self) -> Self::Id {
        self.digest.clone()
    }

    fn update(&mut self, diff: Self::Diff) {
        let (new_digest, _) = diff;
        self.digest = Some(new_digest);
    }

    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        let (new_digest, proof) = diff;
        match (proof, self.digest.as_ref()) {
            (Some(p), Some(s)) => s.verify_append_only(p, &new_digest),
            (Some(_), None) => panic!("Weird combination of proof and no state"),
            (None, None) => true,
            (None, Some(_)) => false
        }
    }

    fn verify_membership(
        &self,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        let encoded = bincode::serialize(package_id).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();
        match &self.digest {
            None => false,
            Some(d) => d.verify(&prime, revision.0.get().try_into().unwrap(), proof)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Authenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest: std::fmt::Debug + Serialize + Eq + PartialEq + std::hash::Hash,
{
    rsa_acc: A,
    log: Vec<Integer>,
    old_acc_idxs: HashMap<<A as Accumulator>::Digest, usize>, // TODO: consider giving this usize to the client in this snapshot
}

impl<A> Authenticator<A>
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
    A: Accumulator + Serialize + Default,
    <A as Accumulator>::Digest:
        Clone + Serialize + PartialEq + Eq + std::hash::Hash + std::fmt::Debug,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + Serialize,
{
    fn batch_import(packages: Vec<PackageId>) -> Self {
        // TODO: implement batch import in the accumulator
        let mut auth = Self::default();
        for p in packages {
            let encoded = bincode::serialize(&p).unwrap();
            let prime = hash_to_prime(&encoded).unwrap();
            auth.rsa_acc.increment(prime.clone());
        }
        auth.rsa_acc.precompute_proofs();
        auth
    }

    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot<A::Digest> as ClientSnapshot>::Id,
    ) -> Option<<Snapshot<A::Digest> as ClientSnapshot>::Diff> {
        let snap = match snapshot_id {
            // client had no state, they don't need a proof
            None => {return Some((self.rsa_acc.digest().clone(), None));}
            Some(s) => s
        };
        if &snap == self.rsa_acc.digest() {
            return None;
        }
        let new_digest = self.rsa_acc.digest().clone();
        let old_rsa_acc_idx = match self.old_acc_idxs.get(&snap) {
            Some(o) => o,
            None => {
                panic!("missing accumulator index");
            }
        };
        let proof = self
            .rsa_acc
            .prove_append_only_from_vec(&self.log[*old_rsa_acc_idx..]);
        Some((new_digest, Some(proof)))
    }

    fn publish(&mut self, package: PackageId) -> () {
        let encoded = bincode::serialize(&package).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();
        self.rsa_acc.increment(prime.clone());
        self.log.push(prime);
        self.old_acc_idxs
            .insert(self.rsa_acc.digest().clone(), self.log.len());
    }

    fn request_file(
        &mut self,
        snapshot_id: <Snapshot<A::Digest> as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot<A::Digest> as ClientSnapshot>::Proof) {
        let encoded = bincode::serialize(package).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();

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
