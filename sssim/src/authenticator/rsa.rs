use std::{collections::HashMap, num::NonZeroU64};

use crate::{
    accumulator::{Accumulator, BatchAccumulator, BatchDigest, Digest},
    hash_to_prime::hash_to_prime,
    multiset::MultiSet,
    util::{byte, DataSized, Information},
};
use rug::Integer;

use authenticator::{ClientSnapshot, Revision};
use serde::Serialize;

use crate::{authenticator, log::PackageId};

#[derive(Default, Clone, Debug)]
pub struct Snapshot<D: Digest> {
    digest: Option<D>,
}

impl<D: Digest> Snapshot<D> {
    pub fn new(digest: D) -> Self {
        Self {
            digest: Some(digest),
        }
    }
}

impl<D: Digest> DataSized for Snapshot<D> {
    fn size(&self) -> Information {
        Information::new::<byte>(0)
    }
}

impl<D> ClientSnapshot for Snapshot<D>
where
    D: Digest + Clone + Serialize + std::fmt::Debug,
    <D as Digest>::AppendOnlyWitness: Clone + Serialize,
    <D as Digest>::Witness: Clone + Serialize + std::fmt::Debug,
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
            (Some(p), Some(s)) => s.verify_append_only(p, new_digest),
            (Some(_), None) => panic!("Weird combination of proof and no state"),
            (None, None) => true,
            (None, Some(_)) => false,
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
            Some(d) => d.verify(&prime, revision.0.get().try_into().unwrap(), proof),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Authenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest:
        Clone + std::fmt::Debug + Serialize + Eq + PartialEq + std::hash::Hash,
{
    acc: A,
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
    fn new(acc: A) -> Self {
        let mut old_acc_idxs: HashMap<<A as Accumulator>::Digest, usize> = Default::default();
        old_acc_idxs.insert(acc.digest().clone(), 0);
        Authenticator {
            acc: acc,
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
    A: Accumulator + Serialize + Default + std::fmt::Debug,
    <A as Accumulator>::Digest:
        Clone + Serialize + PartialEq + Eq + std::hash::Hash + std::fmt::Debug,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + Serialize + std::fmt::Debug,
{
    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut multiset = MultiSet::<Integer>::default();
        for p in packages {
            let encoded = bincode::serialize(&p).unwrap();
            let prime = hash_to_prime(&encoded).unwrap();
            multiset.insert(prime);
        }
        let mut acc = A::import(multiset.clone());
        let digest = acc.digest().clone();
        for (value, rev) in multiset.iter() {
            let witness = acc.prove(value, *rev).unwrap();
            assert!(digest.verify(value, *rev, witness));
        }
        Self::new(acc)
    }

    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot<A::Digest> as ClientSnapshot>::Id,
    ) -> Option<<Snapshot<A::Digest> as ClientSnapshot>::Diff> {
        let snap = match snapshot_id {
            // client had no state, they don't need a proof
            None => {
                return Some((self.acc.digest().clone(), None));
            }
            Some(s) => s,
        };
        if &snap == self.acc.digest() {
            return None;
        }
        let new_digest = self.acc.digest().clone();
        let proof = self.acc.prove_append_only(&snap);
        Some((new_digest, Some(proof)))
    }

    fn publish(&mut self, package: PackageId) {
        let encoded = bincode::serialize(&package).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();
        self.acc.increment(prime.clone());
        self.log.push(prime);
        self.old_acc_idxs
            .insert(self.acc.digest().clone(), self.log.len());
    }

    fn request_file(
        &mut self,
        snapshot_id: <Snapshot<A::Digest> as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot<A::Digest> as ClientSnapshot>::Proof) {
        let encoded = bincode::serialize(package).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();

        let revision = self.acc.get(&prime);
        let proof = self.acc.prove(&prime, revision).expect("proof failed");

        let revision: NonZeroU64 = u64::from(revision).try_into().unwrap();
        (Revision::from(revision), proof)
    }

    fn name() -> &'static str {
        "rsa"
    }

    fn get_metadata(&self) -> Snapshot<A::Digest> {
        Snapshot::new(self.acc.digest().clone())
    }
}

impl<A> DataSized for Authenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest:
        Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash + Serialize,
{
    fn size(&self) -> Information {
        Information::new::<byte>(0)
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}

#[derive(Default, Clone, Debug)]
pub struct PoolSnapshot<A: BatchAccumulator> {
    digest: Option<Snapshot<<A as Accumulator>::Digest>>,
    pool: Vec<PackageId>,
}

struct PoolDiff<A: BatchAccumulator> {
    rest_of_current_day: Vec<PackageId>,
    current_day_final_digest: Option<(
        <A as Accumulator>::Digest,
        &[(PackageId, u32)],
        <<A as Accumulator>::Digest as BatchDigest>::BatchWitness,
        <<A as Accumulator>::Digest as BatchDigest>::BatchWitness,
    )>,
    latest_digest: Option<(
        <A as Accumulator>::Digest,
        <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness,
    )>,
    latest_pool: Vec<PackageId>,
}

impl<A> ClientSnapshot for PoolSnapshot<A>
where
    <A as Accumulator>::Digest: Digest + Clone + Serialize + std::fmt::Debug,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + Serialize + std::fmt::Debug,
{
    type Id = (Option<<A as Accumulator>::Digest>, usize);
    type Diff = PoolDiff<<A as Accumulator>::Digest>;
    type Proof = <<A as Accumulator>::Digest as Digest>::Witness;

    fn id(&self) -> Self::Id {
        (self.digest.clone(), self.pool.len())
    }

    fn update(&mut self, diff: Self::Diff) {
        let current_day_final_digest = match diff.current_day_final_digest {
            Some((d, _)) => d, // The next digest is ready; we may want to update to that.
            None => {
                // Still in the same day. No new digest.
                self.pool.append(&diff.rest_of_current_day);
                return;
            }
        };
        self.digest = match diff.latest_digest {
            Some((d, _)) => d,                // Use the latest digest.
            None => current_day_final_digest, // No "latest" digest; use the one from end-of-current-day.
        };
        // The latest pool can apply against either the current day's final
        // digest or the latest digest.
        self.pool = diff.latest_pool;
    }

    // TODO: special-case for RSA accumulators
    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        let eod_digest =
            if let Some(eod_digest, current_revisions, batch_witness_old, batch_witness_new) =
                diff.current_day_final_digest
            {
                if !self
                    .digest
                    .verify_batch(current_revisions, batch_witness_old)
                {
                    return false;
                }

                let additions = self.pool + diff.rest_of_current_day;
                for package in additions {
                    match current_revisions.get_mut(package) {
                        Some(r) => {
                            r += 1;
                        }
                        None => {
                            return false; // missing from current_revisions
                        }
                    }
                }
                if !self
                    .eod_digest
                    .verify_batch(current_revisions, batch_witness_new)
                {
                    return false;
                }
                eod_digest
            } else if diff.latest_digest.is_some() {
                return false; // if latest_digest is set, current day must also be set
            };
        if let Some((d, w)) = diff.latest_digest {
            if !eod_digest.check_no_rollback((d, w)) {
                return false;
            }
        }
        true
    }

    fn verify_membership(
        &self,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        if !self.digest.verify(
            package_id,
            proof.last_digest_revision,
            proof.last_digest_witness,
        ) {
            return false;
        }
        let mut alleged_revision = proof.last_digest_revision;
        alleged_revision += self.pool.iter().filter(|p| p == package_id).count();
        return alleged_revision == revision;
    }
}

#[derive(Clone, Debug, Default)]
pub struct BatchAuthenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest:
        Clone + std::fmt::Debug + Serialize + Eq + PartialEq + std::hash::Hash,
{
    inner: Authenticator<A>,
}

impl<A> From<Authenticator<A>> for BatchAuthenticator<A>
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
    fn from(inner: Authenticator<A>) -> Self {
        Self { inner }
    }
}

#[allow(unused_variables)]
impl<A> authenticator::Authenticator<BatchSnapshot<A::Digest>> for BatchAuthenticator<A>
where
    A: Accumulator + Serialize + Default + std::fmt::Debug,
    <A as Accumulator>::Digest:
        Clone + Serialize + PartialEq + Eq + std::hash::Hash + std::fmt::Debug,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + Serialize + std::fmt::Debug,
{
    fn batch_import(packages: Vec<PackageId>) -> Self {
        Authenticator::<A>::batch_import(packages).into()
    }

    fn refresh_metadata(
        &self,
        snapshot_id: <BatchSnapshot<A::Digest> as ClientSnapshot>::Id,
    ) -> Option<<Snapshot<A::Digest> as ClientSnapshot>::Diff> {
    }

    fn publish(&mut self, package: PackageId) {
        let encoded = bincode::serialize(&package).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();
        self.acc.increment(prime.clone());
        self.log.push(prime);
        self.old_acc_idxs
            .insert(self.acc.digest().clone(), self.log.len());
    }

    fn request_file(
        &mut self,
        snapshot_id: <Snapshot<A::Digest> as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot<A::Digest> as ClientSnapshot>::Proof) {
        let encoded = bincode::serialize(package).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();

        let revision = self.acc.get(&prime);
        let proof = self.acc.prove(&prime, revision).expect("proof failed");

        let revision: NonZeroU64 = u64::from(revision).try_into().unwrap();
        (Revision::from(revision), proof)
    }

    fn name() -> &'static str {
        "rsa"
    }

    fn get_metadata(&self) -> Snapshot<A::Digest> {
        Snapshot::new(self.acc.digest().clone())
    }
}

impl<A> DataSized for Authenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest:
        Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash + Serialize,
{
    fn size(&self) -> Information {
        Information::new::<byte>(0)
    }
}
