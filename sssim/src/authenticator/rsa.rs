use core::fmt::Debug;
use derivative::Derivative;
use std::{collections::HashMap, num::NonZeroU64};

use crate::{
    accumulator::{Accumulator, BatchAccumulator, BatchDigest, Digest},
    hash_to_prime::hash_to_prime,
    multiset::MultiSet,
    util::{byte, DataSized, Information},
};
use rug::Integer;

use authenticator::{Authenticator as _, ClientSnapshot, Revision};
use serde::Serialize;

use crate::{authenticator, log::PackageId};

use super::{BatchAuthenticator, BatchClientSnapshot};

#[derive(Default, Clone, Debug)]
pub struct Snapshot<D: Digest> {
    digest: Option<D>,
}

impl<D: Digest> From<D> for Snapshot<D> {
    fn from(inner: D) -> Self {
        Snapshot {
            digest: Some(inner),
        }
    }
}

fn hash_package(package: &PackageId) -> Integer {
    let encoded = bincode::serialize(package).unwrap();
    hash_to_prime(&encoded).unwrap()
}

fn convert_package_counts(package_counts: &HashMap<PackageId, Revision>) -> HashMap<Integer, u32> {
    let mut hashed_package_counts: HashMap<Integer, u32> = Default::default();
    for (key, value) in package_counts.iter() {
        let revision: u32 = u64::from(value.0).try_into().unwrap();
        hashed_package_counts.insert(hash_package(key), revision);
    }
    hashed_package_counts
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

#[derive(Derivative)]
#[derivative(Clone(bound = "A: Clone, <A as Accumulator>::Digest: Clone"))]
#[derivative(Debug(bound = "A: std::fmt::Debug, <A as Accumulator>::Digest: std::fmt::Debug"))]
pub struct Authenticator<A>
where
    A: Accumulator,
    A: Debug,
    A: Serialize,
{
    acc: A,
    log: Vec<Integer>,
    old_acc_idxs: HashMap<<A as Accumulator>::Digest, usize>, // TODO: consider giving this usize to the client in this snapshot
}

impl<A> Authenticator<A>
where
    A: Accumulator + Default + Serialize + std::fmt::Debug,
    <A as Accumulator>::Digest: Clone + std::fmt::Debug + std::hash::Hash + Eq,
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
    A: Accumulator + Default + Serialize + std::fmt::Debug,
    <A as Accumulator>::Digest: Clone + std::fmt::Debug + std::hash::Hash + Eq,
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
        let prime = hash_package(&package);
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
        let prime = hash_package(&package);

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
    A: Accumulator + Default + Serialize + std::fmt::Debug,
    <A as Accumulator>::Digest:
        Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash + Serialize,
{
    fn size(&self) -> Information {
        Information::new::<byte>(0)
    }
}

impl<D> BatchClientSnapshot for Snapshot<D>
where
    D: BatchDigest + Digest + std::fmt::Debug + Clone + Serialize,
    D::BatchWitness: Serialize + Clone + std::fmt::Debug,
    D::AppendOnlyWitness: std::fmt::Debug + Clone + Serialize,
    D::Witness: std::fmt::Debug + Clone + Serialize,
{
    type BatchProof = D::BatchWitness;

    fn batch_verify(
        &self,
        packages: HashMap<PackageId, Revision>,
        proof: Self::BatchProof,
    ) -> bool {
        let members = convert_package_counts(&packages);
        self.digest.as_ref().unwrap().verify_batch(&members, proof)
    }
}

impl<A> BatchAuthenticator<Snapshot<A::Digest>> for Authenticator<A>
where
    A: Accumulator + BatchAccumulator + Default + Serialize + std::fmt::Debug,
    <A as Accumulator>::Digest:
        Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash + Serialize + BatchDigest,
    <A as Accumulator>::Digest: Digest + std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as BatchDigest>::BatchWitness: std::fmt::Debug + Clone + Serialize,
{
    fn batch_prove(
        &mut self,
        packages: Vec<PackageId>,
    ) -> (
        HashMap<PackageId, Revision>,
        <Snapshot<A::Digest> as BatchClientSnapshot>::BatchProof,
    ) {
        let mut members: HashMap<Integer, u32> = Default::default();
        let mut package_revisions: HashMap<PackageId, Revision> = Default::default();
        for package in packages {
            let prime = hash_package(&package);
            let revision: u32 = self.acc.get(&prime);
            members.insert(prime, revision);
            let revision: Revision = usize::try_from(revision).unwrap().try_into().unwrap();
            package_revisions.insert(package, revision);
        }
        (package_revisions, self.acc.prove_batch(&members))
    }
}

impl<A> authenticator::Authenticator<Snapshot<A::Digest>> for PoolAuthenticator<A>
where
    A: Accumulator + Serialize + std::fmt::Debug,
    <A as Accumulator>::Digest: BatchDigest + Debug + Serialize + Clone,
    <<A as Accumulator>::Digest as Digest>::Witness: Serialize + Debug + Clone,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: Serialize + Debug + Clone + DataSized,
    authenticator::rsa::PoolAuthenticator<A>: DataSized
{
    fn name() -> &'static str {
        "rsa_batched"
    }

    fn refresh_metadata(&self, snapshot_id: S::Id) -> Option<S::Diff> {

    }
}

impl<A> authenticator::PoolAuthenticator<Snapshot<A::Digest>> for PoolAuthenticator<A>
where
    A: Accumulator + BatchAccumulator + Default + Serialize + std::fmt::Debug,
    <A as Accumulator>::Digest:
        Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash + Serialize + BatchDigest,
    <A as Accumulator>::Digest: Digest + std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as BatchDigest>::BatchWitness: std::fmt::Debug + Clone + Serialize,
    PoolAuthenticator<A>: DataSized,
{
    fn batch_process(&mut self) {
        let bod_digest = self.inner.acc.digest().clone();
        let mut bod_package_counts: HashMap<PackageId, Revision> = Default::default();
        let mut bod_membership_witnesses: HashMap<
            PackageId,
            <<A as Accumulator>::Digest as Digest>::Witness,
        > = Default::default();
        for package in &self.current_pool {
            let (revision, proof) = self.inner.request_file(Default::default(), &package);
            bod_package_counts.insert(package.clone(), revision);
            bod_membership_witnesses.insert(package.clone(), proof);
        }
        let bod_batch_membership_witness = todo!(); // self.inner.aggegrate(bod_membership_witnesses);

        //self.inner.increment_batch(self.current_pool);

        let eod_digest = self.inner.acc.digest().clone();
        let mut eod_package_counts: HashMap<PackageId, Revision> = Default::default();
        let mut eod_membership_witnesses: HashMap<
            PackageId,
            <<A as Accumulator>::Digest as Digest>::Witness,
        > = Default::default();
        for package in self.current_pool {
            let (revision, proof) = self.inner.request_file(Default::default(), &package);
            eod_package_counts.insert(package, revision);
            eod_membership_witnesses.insert(package, proof);
        }
        let eod_batch_membership_witness: <A::Digest as BatchDigest>::BatchWitness = todo!(); // self.inner.aggegrate(eod_membership_witnesses);

        let epoch: Epoch<A::Digest> = Epoch {
            packages: self.current_pool,
            eod_digest,
            bod_package_counts,
            bod_package_membership_witness: bod_batch_membership_witness,
            eod_package_counts,
            eod_package_membership_witness: eod_batch_membership_witness,
        };
        self.epoch_idxs_by_digest
            .insert(bod_digest, self.past_epochs.len().into());
        self.past_epochs.push(epoch);
        self.current_pool = vec![];
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}

#[derive(Default, Clone, Debug)]
pub struct PoolSnapshot<D: BatchDigest> {
    inner: Snapshot<D>,
    pool: Vec<PackageId>,
}

#[derive(Serialize, Derivative)]
#[derivative(Clone(bound = "D: Clone, D::BatchWitness: Clone"))]
struct CatchUpToEODProof<D: BatchDigest>
where
    D::BatchWitness: Serialize,
{
    eod_digest: D,
    bod_package_counts: HashMap<PackageId, Revision>,
    bod_package_membership_witness: D::BatchWitness,
    eod_package_membership_witness: D::BatchWitness, // TODO: missing? bod->eod append only witness
}

impl<D: BatchDigest> CatchUpToEODProof<D>
where
    D::BatchWitness: Serialize,
{
    fn from_epoch(epoch: Epoch<D>, next_digest: D) -> Self {
        Self {
            eod_digest: next_digest,
            bod_package_counts: epoch.bod_package_counts,
            bod_package_membership_witness: epoch.bod_package_membership_witness,
            eod_package_membership_witness: epoch.eod_package_membership_witness,
        }
    }
}

#[derive(Derivative)]
#[derivative(Clone(bound = "D: Clone, D::AppendOnlyWitness: Clone, CatchUpToEODProof<D>: Clone"))]
#[derivative(Default)]
#[derive(Serialize)]
pub struct PoolDiff<D: BatchDigest>
where
    D::AppendOnlyWitness: Serialize,
    D::BatchWitness: Serialize,
{
    rest_of_current_day: Vec<PackageId>,
    current_day_final_digest: Option<CatchUpToEODProof<D>>,
    latest_digest: Option<(D, D::AppendOnlyWitness)>,
    latest_pool: Vec<PackageId>,
}

impl<D: BatchDigest> PoolDiff<D>
where
    D::AppendOnlyWitness: Serialize,
    D::BatchWitness: Serialize,
    D: Default,
{
    fn for_current_day(rest_of_current_day: Vec<PackageId>) -> Self {
        Self {
            rest_of_current_day,
            ..Default::default()
        }
    }
}

impl<D: Clone> PoolSnapshot<D>
where
    D: BatchDigest,
    D::BatchWitness: Serialize + Clone,
{
    fn validate_catch_up_proof(
        &self,
        catch_up_proof: &CatchUpToEODProof<D>,
        rest_of_current_day: &Vec<PackageId>,
    ) -> Result<D, ()> {
        let hashed_package_counts = convert_package_counts(&catch_up_proof.bod_package_counts);
        if !self.inner.digest.as_ref().unwrap().verify_batch(
            &hashed_package_counts,
            catch_up_proof.bod_package_membership_witness.clone(),
        ) {
            return Err(());
        }
        let mut package_counts = catch_up_proof.bod_package_counts.clone();

        for package in self.pool.iter().chain(rest_of_current_day) {
            match package_counts.get_mut(&package) {
                Some(r) => {
                    r.increment();
                }
                None => {
                    return Err(()); // missing from current_revisions
                }
            }
        }
        let hashed_package_counts = convert_package_counts(&package_counts);
        if !catch_up_proof.eod_digest.verify_batch(
            &hashed_package_counts,
            catch_up_proof.eod_package_membership_witness.clone(),
        ) {
            return Err(());
        }
        // TODO: check bod->eod append only
        Ok(catch_up_proof.eod_digest.clone())
    }
}

impl<D> ClientSnapshot for PoolSnapshot<D>
where
    D: Digest + Clone + Serialize + std::fmt::Debug + BatchDigest,
    <D as Digest>::AppendOnlyWitness: Clone + Serialize,
    <D as BatchDigest>::BatchWitness: Clone + Serialize,
    <D as Digest>::Witness: Clone + Serialize + std::fmt::Debug,
{
    type Id = Option<(D, usize)>;
    type Diff = PoolDiff<D>;
    type Proof = D::Witness;

    fn id(&self) -> Self::Id {
        Some((self.inner.digest.as_ref().unwrap().clone(), self.pool.len()))
    }

    fn update(&mut self, mut diff: Self::Diff) {
        let eod_digest: D = match diff.current_day_final_digest {
            Some(catch_up_proof) => catch_up_proof.eod_digest, // The next digest is ready; we may want to update to that.
            None => {
                // Still in the same day. No new digest.
                self.pool.append(&mut diff.rest_of_current_day);
                return;
            }
        };
        self.inner = Snapshot::from(match diff.latest_digest {
            Some((d, _)) => d,  // Use the latest digest.
            None => eod_digest, // No "latest" digest; use the one from end-of-current-day.
        });
        // The latest pool can apply against either the current day's final
        // digest or the latest digest.
        self.pool = diff.latest_pool;
    }

    // TODO: special-case for RSA accumulators
    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        match (
            diff.current_day_final_digest.as_ref(),
            diff.latest_digest.as_ref(),
        ) {
            (Some(catch_up_proof), Some(latest_digest)) => {
                if let Ok(eod_digest) =
                    self.validate_catch_up_proof(catch_up_proof, &diff.rest_of_current_day)
                {
                    let eod_snapshot = Snapshot::from(eod_digest);
                    let (d, w) = latest_digest;
                    if !eod_snapshot.check_no_rollback(&(d.clone(), Some(w.clone()))) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            (Some(catch_up_proof), None) => {
                if self
                    .validate_catch_up_proof(catch_up_proof, &diff.rest_of_current_day)
                    .is_err()
                {
                    return false;
                }
            }
            (None, None) => {}
            (None, Some(_)) => {
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
        let bod_revision = revision - self.pool.iter().filter(|p| p == &package_id).count();
        self.inner
            .verify_membership(package_id, bod_revision, proof)
    }
}

#[derive(Derivative)]
#[derivative(Debug(bound = "D: std::fmt::Debug, D::BatchWitness: std::fmt::Debug"))]
#[derivative(Clone(bound = "D: Clone, D::BatchWitness: Clone"))]
#[derivative(Default(bound = "D: Default, D::BatchWitness: Default"))]
struct Epoch<D: BatchDigest> {
    packages: Vec<PackageId>,
    eod_digest: D,
    // only for the things that got updated this epoch
    bod_package_counts: HashMap<PackageId, Revision>,
    // only for the things that got updated this epoch
    eod_package_counts: HashMap<PackageId, Revision>,
    bod_package_membership_witness: D::BatchWitness,
    eod_package_membership_witness: D::BatchWitness,
}

#[derive(Derivative)]
#[derivative(Clone(
    bound = "A: Clone, Epoch<<A as Accumulator>::Digest>: Clone, <A as Accumulator>::Digest: Clone"
))]
#[derivative(Debug(
    bound = "A: std::fmt::Debug, Epoch<<A as Accumulator>::Digest>: std::fmt::Debug,<A as Accumulator>::Digest: std::fmt::Debug"
))]
#[derivative(Default(
    bound = "A: Default, <A as Accumulator>::Digest: Clone + std::fmt::Debug + std::hash::Hash + Eq"
))]
pub struct PoolAuthenticator<A: Accumulator>
where
    <A as Accumulator>::Digest: BatchDigest,
    A: std::fmt::Debug,
    A: Serialize,
{
    inner: Authenticator<A>,
    past_epochs: Vec<Epoch<<A as Accumulator>::Digest>>,
    epoch_idxs_by_digest: HashMap<<A as Accumulator>::Digest, usize>,
    current_pool: Vec<PackageId>,
}

#[allow(unused_variables)]
impl<A> authenticator::Authenticator<PoolSnapshot<A::Digest>> for PoolAuthenticator<A>
where
    A: Accumulator + Serialize + Default + std::fmt::Debug + BatchAccumulator,
    <A as Accumulator>::Digest: Clone
        + Serialize
        + PartialEq
        + Eq
        + std::hash::Hash
        + std::fmt::Debug
        + BatchDigest
        + Default,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: std::fmt::Debug + Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + Serialize + std::fmt::Debug,
    <<A as Accumulator>::Digest as BatchDigest>::BatchWitness: Serialize + Clone + std::fmt::Debug,
{
    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut inner = Authenticator::<A>::batch_import(packages.clone());
        let (eod_package_counts, eod_package_membership_witness) =
            inner.batch_prove(packages.clone());
        let epoch: Epoch<<A as Accumulator>::Digest> = Epoch {
            packages,
            eod_digest: <A as Accumulator>::Digest::default(),
            bod_package_counts: Default::default(),
            eod_package_counts,
            bod_package_membership_witness: eod_package_membership_witness.clone(), // total lie but it typechecks
            eod_package_membership_witness,
        };
        let past_epochs = vec![epoch.clone()];
        let mut epoch_idxs_by_digest = HashMap::default();
        epoch_idxs_by_digest.insert(epoch.eod_digest.clone(), 0);
        Self {
            inner,
            past_epochs,
            epoch_idxs_by_digest,
            current_pool: vec![],
        }
    }

    fn refresh_metadata(
        &self,
        snapshot_id: <PoolSnapshot<A::Digest> as ClientSnapshot>::Id,
    ) -> Option<PoolDiff<<A as Accumulator>::Digest>> {
        if snapshot_id.is_none() {
            todo!(); // not pooldiff
        }
        let (digest, id_idx) = snapshot_id.unwrap();

        if self.inner.acc.digest() == &digest {
            if id_idx == self.current_pool.len() {
                return None;
            }
            return Some(PoolDiff::for_current_day(self.current_pool.clone()));
        }

        let epoch_idx = *self.epoch_idxs_by_digest.get(&digest).unwrap();
        let epoch = &self.past_epochs[epoch_idx];
        let rest_of_current_day = epoch.packages[id_idx..].to_vec().clone();

        if (epoch_idx + 1) == self.past_epochs.len() {
            panic!("uh oh");
        } else if (epoch_idx + 2) == self.past_epochs.len() {
            // one day behind
            let next_digest = self.inner.acc.digest();
            Some(PoolDiff {
                rest_of_current_day,
                current_day_final_digest: Some(CatchUpToEODProof::from_epoch(
                    epoch.clone(),
                    next_digest.clone(),
                )),
                latest_digest: None,
                latest_pool: self.current_pool.clone(),
            })
        } else {
            // >one day behind
            // get *append only* from eod_digest to latest_digest
            let next_digest = &self.past_epochs[epoch_idx + 1].eod_digest;
            let append_only_witness = self.inner.acc.prove_append_only(next_digest);
            let latest_digest = Some((self.inner.acc.digest().clone(), append_only_witness));
            Some(PoolDiff {
                rest_of_current_day,
                current_day_final_digest: Some(CatchUpToEODProof::from_epoch(
                    epoch.clone(),
                    next_digest.clone(),
                )),
                latest_digest,
                latest_pool: self.current_pool.clone(),
            })
        }
    }

    fn publish(&mut self, package: PackageId) {
        self.current_pool.push(package);
    }

    fn request_file(
        &mut self,
        snapshot_id: Option<(<A as Accumulator>::Digest, usize)>,
        package: &PackageId,
    ) -> (Revision, <Snapshot<A::Digest> as ClientSnapshot>::Proof) {
        let (inner_snapshot, pool_size) = snapshot_id.unwrap();
        let _ = pool_size;
        // assert_eq!(pool_size, self.current_pool.size());
        let (bod_revision, bod_membership_proof) =
            self.inner.request_file(Some(inner_snapshot), package);
        let revision = bod_revision + self.current_pool.iter().filter(|p| p == &package).count();
        (revision, bod_membership_proof)
    }

    fn name() -> &'static str {
        "rsa_pool"
    }

    fn get_metadata(&self) -> PoolSnapshot<A::Digest> {
        let snapshot: Snapshot<A::Digest> = Snapshot::new(self.inner.acc.digest().clone());
        PoolSnapshot {
            inner: snapshot,
            pool: self.current_pool.clone(),
        }
    }
}

impl<A> PoolAuthenticator<A>
where
    A: Accumulator + Default + std::fmt::Debug + Clone + Serialize,
    <A as Accumulator>::Digest:
        std::fmt::Debug + Clone + BatchDigest + Eq + PartialEq + std::hash::Hash + Serialize,
    A: Accumulator + Serialize + Default + std::fmt::Debug,
    <A as Accumulator>::Digest:
        Clone + Serialize + PartialEq + Eq + std::hash::Hash + std::fmt::Debug,
    <<A as Accumulator>::Digest as Digest>::AppendOnlyWitness: Clone + Serialize,
    <<A as Accumulator>::Digest as Digest>::Witness: Clone + Serialize + std::fmt::Debug,
{
}

impl<A> DataSized for PoolAuthenticator<A>
where
    A: Accumulator + Default + std::fmt::Debug + Serialize,
    <A as Accumulator>::Digest: Clone + std::fmt::Debug + BatchDigest,
{
    fn size(&self) -> Information {
        Information::new::<byte>(0)
    }
}
