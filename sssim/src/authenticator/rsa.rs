use core::fmt::Debug;
use derivative::Derivative;
use std::{collections::HashMap, fmt, hash, marker::PhantomData, num::NonZeroU64};

use crate::{
    accumulator::{Accumulator, BatchAccumulator},
    hash_to_prime::hash_to_prime,
    multiset::MultiSet,
    primitives::Prime,
    util::{byte, DataSized, Information, STRING_BYTES},
};

use authenticator::Revision;
use serde::Serialize;

use crate::{authenticator, log::PackageId};

use super::BatchAuthenticator;

#[derive(Clone, Default, Debug, Serialize)]
pub struct Snapshot<A>
where
    A: Accumulator,
{
    #[serde(bound(serialize = "A::Digest: Serialize"))]
    digest: Option<A::Digest>,
    #[serde(skip)]
    _accumulator: PhantomData<A>,
}

impl<A: Accumulator> Snapshot<A> {
    fn new(inner: A::Digest) -> Self {
        Snapshot {
            digest: Some(inner),
            _accumulator: Default::default(),
        }
    }
}

fn hash_package(package: &PackageId) -> Prime {
    let encoded = bincode::serialize(package).unwrap();
    hash_to_prime(&encoded).unwrap()
}

fn convert_package_counts(package_counts: &HashMap<PackageId, u32>) -> HashMap<Prime, u32> {
    let mut hashed_package_counts: HashMap<Prime, u32> = Default::default();
    for (key, revision) in package_counts.iter() {
        hashed_package_counts.insert(hash_package(key), *revision);
    }
    hashed_package_counts
}

impl<A: Accumulator> DataSized for Snapshot<A>
where
    A::Digest: DataSized,
{
    fn size(&self) -> Information {
        self.digest.size()
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Diff<A: Accumulator> {
    digest: A::Digest,
    #[serde(bound(serialize = "A::AppendOnlyWitness: Serialize"))]
    update: Option<A::AppendOnlyWitness>,
}

impl<A: Accumulator> Diff<A> {
    pub fn new(digest: A::Digest, update: Option<A::AppendOnlyWitness>) -> Self {
        Self { digest, update }
    }
}

impl<A: Accumulator> DataSized for Diff<A>
where
    A::Digest: DataSized,
    A::AppendOnlyWitness: DataSized,
{
    fn size(&self) -> Information {
        self.digest.size() + self.update.size()
    }
}

#[derive(Derivative)]
#[derivative(Clone(bound = "A: Clone, <A as Accumulator>::Digest: Clone"))]
#[derivative(Debug(bound = "A: std::fmt::Debug, <A as Accumulator>::Digest: std::fmt::Debug"))]
pub struct Authenticator<A: Accumulator> {
    acc: A,
    log: Vec<Prime>,
    old_acc_idxs: HashMap<<A as Accumulator>::Digest, usize>, // TODO(maybe): consider giving this usize to the client in this snapshot
}

impl<A> Authenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest: Clone + fmt::Debug + hash::Hash + Eq,
{
    fn new(acc: A) -> Self {
        let mut old_acc_idxs: HashMap<<A as Accumulator>::Digest, usize> = Default::default();
        old_acc_idxs.insert(acc.digest().clone(), 0);
        Authenticator {
            acc,
            log: vec![],
            old_acc_idxs,
        }
    }
}

impl<A> Default for Authenticator<A>
where
    A: Accumulator + Default,
    <A as Accumulator>::Digest: Clone + fmt::Debug + hash::Hash + Eq,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[allow(unused_variables)]
impl<A: Accumulator> super::Authenticator for Authenticator<A>
where
    A: Default + fmt::Debug + DataSized,
    A::Digest: Clone + PartialEq + Eq + hash::Hash + fmt::Debug,
    A::AppendOnlyWitness: Clone + fmt::Debug,
    A::Witness: Clone + DataSized + Serialize,
    Diff<A>: Clone + DataSized + Serialize,
    Snapshot<A>: Clone + DataSized,
    Authenticator<A>: DataSized,
{
    type ClientSnapshot = Snapshot<A>;
    type Id = Option<A::Digest>;
    type Diff = Diff<A>;
    type Proof = A::Witness;

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut multiset = MultiSet::<Prime>::default();
        for p in packages {
            let encoded = bincode::serialize(&p).unwrap();
            let prime = hash_to_prime(&encoded).unwrap();
            multiset.insert(prime);
        }
        let mut acc = A::import(multiset.clone());
        let digest = acc.digest().clone();
        for (value, rev) in multiset.iter() {
            let witness = acc.prove(value, *rev).unwrap();
            assert!(A::verify(&digest, value, *rev, witness));
        }
        Self::new(acc)
    }

    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<Self::Diff> {
        let snap = match snapshot_id {
            // client had no state, they don't need a proof
            None => {
                return Some(Diff::new(self.acc.digest().clone(), None));
            }
            Some(s) => s,
        };
        if &snap == self.acc.digest() {
            return None;
        }
        let new_digest = self.acc.digest().clone();
        let proof = self.acc.prove_append_only(&snap);
        Some(Diff::new(new_digest, Some(proof)))
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
        snapshot_id: Self::Id,
        package: &PackageId,
    ) -> (Revision, Self::Proof) {
        let prime = hash_package(&package);

        let revision = self.acc.get(&prime);
        let proof = self.acc.prove(&prime, revision).expect("proof failed");

        let revision: NonZeroU64 = u64::from(revision).try_into().unwrap();
        (Revision::from(revision), proof)
    }

    fn name() -> &'static str {
        "rsa"
    }

    fn get_metadata(&self) -> Self::ClientSnapshot {
        Snapshot::new(self.acc.digest().clone())
    }
    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id {
        snapshot.digest.clone()
    }

    fn update(snapshot: &mut Self::ClientSnapshot, diff: Self::Diff) {
        snapshot.digest = Some(diff.digest);
    }

    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool {
        let (new_digest, proof) = (&diff.digest, &diff.update);
        match (proof, snapshot.digest.as_ref()) {
            (Some(p), Some(s)) => A::verify_append_only(s, &p, &new_digest),
            (Some(_), None) => panic!("Weird combination of proof and no state"),
            (None, None) => true,
            (None, Some(_)) => false,
        }
    }

    fn verify_membership(
        snapshot: &Self::ClientSnapshot,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        let encoded = bincode::serialize(package_id).unwrap();
        let prime = hash_to_prime(&encoded).unwrap();
        match &snapshot.digest {
            None => false,
            Some(d) => A::verify(&d, &prime, revision.0.get().try_into().unwrap(), proof),
        }
    }

    fn cdn_size(&self) -> Information {
        self.acc.cdn_size()
    }
}

impl<A: Accumulator> DataSized for Authenticator<A>
where
    A: DataSized,
    A::Digest: DataSized,
{
    fn size(&self) -> Information {
        let mut size = self.acc.size();
        let len: u64 = self.log.len().try_into().unwrap();
        size += len * Information::new::<byte>(32);

        if self.old_acc_idxs.len() > 0 {
            let item = self.old_acc_idxs.keys().next();
            let len: u64 = self.old_acc_idxs.len().try_into().unwrap();
            //val is usize
            let val = Information::new::<byte>(8);
            size += (item.expect(" ").size() + val) * len;
        }
        size
    }
}

impl<A> BatchAuthenticator for Authenticator<A>
where
    A: BatchAccumulator<BatchDigest = <A as Accumulator>::Digest>
        + Default
        + DataSized
        + fmt::Debug,
    A::Digest: Clone + fmt::Debug + Eq + PartialEq + hash::Hash,
    A::AppendOnlyWitness: fmt::Debug + Clone + DataSized,
    A::Witness: fmt::Debug + Clone + DataSized,
    A::Witness: Clone + DataSized + Serialize,
    Authenticator<A>: super::Authenticator<ClientSnapshot = Snapshot<A>>,
    A::BatchWitness: Clone + DataSized + Serialize,
{
    type BatchProof = A::BatchWitness;

    fn batch_prove(
        &mut self,
        packages: Vec<PackageId>,
    ) -> (HashMap<PackageId, u32>, Self::BatchProof) {
        let package_keys: HashMap<PackageId, Prime> = packages
            .into_iter()
            .map(|p| {
                let h = hash_package(&p);
                (p, h)
            })
            .collect();
        let (counts, batch_proof): (HashMap<Prime, u32>, _) =
            self.acc.prove_batch(package_keys.values().cloned());
        let mut package_revisions: HashMap<PackageId, u32> = Default::default();
        for (package, package_key) in package_keys {
            let count: u32 = *counts.get(&package_key).unwrap();
            package_revisions.insert(package, count);
        }
        (package_revisions, batch_proof)
    }

    fn batch_verify(
        snapshot: &Self::ClientSnapshot,
        packages: HashMap<PackageId, u32>,
        proof: Self::BatchProof,
    ) -> bool {
        let members = convert_package_counts(&packages);
        A::verify_batch(snapshot.digest.as_ref().unwrap(), &members, proof)
    }
}

impl<A> super::PoolAuthenticator for PoolAuthenticator<A>
where
    A: BatchAccumulator + Default + DataSized,
    PoolDiff<A>: Serialize + Clone + DataSized,
    A::Witness: Serialize + Clone + DataSized,
    A::Digest: Clone + Eq + hash::Hash + Default,
    PoolAuthenticator<A>: super::Authenticator,
    A::AppendOnlyWitness: Clone + Default,
    Authenticator<A>: BatchAuthenticator<BatchProof = <A as BatchAccumulator>::BatchWitness>,
{
    fn batch_process(&mut self) {
        let mut pool_counts: HashMap<PackageId, usize> = Default::default();
        for package in self.current_pool.clone() {
            *pool_counts.entry(package).or_default() += 1;
        }
        let pool_packages: Vec<_> = pool_counts.keys().cloned().collect();

        let bod_digest = self.inner.acc.digest().clone();
        let (bod_package_counts, bod_batch_witness) = self.inner.batch_prove(pool_packages.clone());

        let bod_to_eod: A::AppendOnlyWitness = match self
            .inner
            .acc
            .increment_batch(self.current_pool.iter().map(hash_package))
        {
            Some(proof) => proof,
            None => self.inner.acc.prove_append_only(&bod_digest),
        };

        let eod_digest = self.inner.acc.digest().clone();
        let (eod_package_counts, eod_batch_witness) = self.inner.batch_prove(pool_packages);

        let packages: Vec<_> = self.current_pool.drain(..).collect();

        let epoch: Epoch<A> = Epoch {
            packages,
            eod_digest,
            bod_package_counts,
            bod_package_membership_witness: bod_batch_witness,
            eod_package_counts,
            eod_package_membership_witness: eod_batch_witness,
            bod_to_eod,
        };
        self.epoch_idxs_by_digest
            .insert(bod_digest, self.past_epochs.len().into());
        self.past_epochs.push(epoch);
    }
}

#[cfg(test)]
mod tests {
    // TODO(test): fix tests
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Default(bound = "Snapshot<A>: Default"))]
pub struct PoolSnapshot<A: BatchAccumulator> {
    inner: Snapshot<A>,
    pool: Vec<PackageId>,
}

impl<A: BatchAccumulator> DataSized for PoolSnapshot<A>
where
    Snapshot<A>: DataSized,
{
    fn size(&self) -> Information {
        let size = self.inner.size();
        match self.pool.get(0) {
            None => size,
            Some(a) => {
                let len: u64 = self.pool.len().try_into().unwrap();
                a.size() * len + size
            }
        }
    }
}

#[derive(Derivative, Serialize)]
#[derivative(Clone(
    bound = "A::Digest: Clone, A::BatchWitness: Clone, A::AppendOnlyWitness: Clone"
))]
struct CatchUpToEODProof<A: BatchAccumulator> {
    eod_digest: A::Digest,
    bod_package_counts: HashMap<PackageId, u32>,
    bod_package_membership_witness: A::BatchWitness,
    eod_package_membership_witness: A::BatchWitness,
    bod_to_eod: A::AppendOnlyWitness,
}

impl<A: BatchAccumulator> CatchUpToEODProof<A>
where
    A::BatchWitness: Serialize,
{
    fn from_epoch(epoch: Epoch<A>, next_digest: A::Digest) -> Self {
        Self {
            eod_digest: next_digest,
            bod_package_counts: epoch.bod_package_counts,
            bod_package_membership_witness: epoch.bod_package_membership_witness,
            eod_package_membership_witness: epoch.eod_package_membership_witness,
            bod_to_eod: epoch.bod_to_eod,
        }
    }
}

impl<A: BatchAccumulator> DataSized for CatchUpToEODProof<A>
where
    A::AppendOnlyWitness: DataSized,
    A::Digest: DataSized,
    A::BatchWitness: DataSized,
{
    fn size(&self) -> Information {
        let mut size = Information::new::<byte>(0);
        if self.bod_package_counts.len() > 0 {
            let item = self.bod_package_counts.keys().next();
            let len: u64 = self.bod_package_counts.len().try_into().unwrap();
            size += (item.expect("map not empty").size() + Information::new::<byte>(4)) * len;
        }

        size += self.bod_package_membership_witness.size()
            + self.eod_package_membership_witness.size()
            + self.bod_to_eod.size()
            + self.eod_digest.size();
        size
    }
}

#[derive(Derivative, Serialize)]
#[derivative(Clone(bound = "A: Clone, A::AppendOnlyWitness: Clone, CatchUpToEODProof<A>: Clone"))]
#[derivative(Default)]
pub struct PoolDiff<A: BatchAccumulator> {
    rest_of_current_day: Vec<PackageId>,
    #[serde(bound(serialize = "CatchUpToEODProof<A>: Serialize"))]
    current_day_final_digest: Option<CatchUpToEODProof<A>>,
    #[serde(bound(serialize = "A::Digest: Serialize, A::AppendOnlyWitness: Serialize"))]
    latest_digest: Option<(A::Digest, A::AppendOnlyWitness)>,
    latest_pool: Vec<PackageId>,
    initial_digest: Option<A::Digest>,
}

impl<A: BatchAccumulator> DataSized for PoolDiff<A>
where
    A::Digest: DataSized,
    A::AppendOnlyWitness: DataSized,
    CatchUpToEODProof<A>: DataSized,
{
    fn size(&self) -> Information {
        let mut size = Information::new::<byte>(0);
        if self.rest_of_current_day.len() > 0 {
            let len: u64 = self.rest_of_current_day.len().try_into().unwrap();
            size += len * self.rest_of_current_day[0].size();
        }
        if self.latest_pool.len() > 0 {
            let len: u64 = self.latest_pool.len().try_into().unwrap();
            size += len * self.latest_pool[0].size();
        }
        match &self.latest_digest {
            None => {}
            Some((d, a)) => size += d.size() + a.size(),
        }
        size += self.current_day_final_digest.size() + self.initial_digest.size();
        size
    }
}

impl<A: BatchAccumulator> PoolDiff<A> {
    fn initial(digest: A::Digest, latest_pool: Vec<PackageId>) -> Self {
        Self {
            initial_digest: Some(digest),
            latest_pool,
            rest_of_current_day: vec![],
            current_day_final_digest: None,
            latest_digest: None,
        }
    }

    fn for_current_day(rest_of_current_day: Vec<PackageId>) -> Self {
        Self {
            rest_of_current_day,
            current_day_final_digest: None,
            latest_digest: None,
            latest_pool: vec![],
            initial_digest: None,
        }
    }

    fn for_next_day(
        rest_of_current_day: Vec<PackageId>,
        current_day_final_digest: CatchUpToEODProof<A>,
        latest_pool: Vec<PackageId>,
    ) -> Self {
        Self {
            rest_of_current_day,
            current_day_final_digest: Some(current_day_final_digest),
            latest_pool,
            latest_digest: None,
            initial_digest: None,
        }
    }

    fn for_latter_day(
        rest_of_current_day: Vec<PackageId>,
        current_day_final_digest: CatchUpToEODProof<A>,
        latest_digest: (A::Digest, A::AppendOnlyWitness),
        latest_pool: Vec<PackageId>,
    ) -> Self {
        Self {
            rest_of_current_day,
            current_day_final_digest: Some(current_day_final_digest),
            latest_digest: Some(latest_digest),
            latest_pool,
            initial_digest: None,
        }
    }
}

impl<A> PoolSnapshot<A>
where
    A: BatchAccumulator<BatchDigest = <A as Accumulator>::Digest> + Clone,
    A::BatchWitness: Clone,
    A::Digest: Clone,
{
    fn validate_catch_up_proof(
        &self,
        catch_up_proof: &CatchUpToEODProof<A>,
        rest_of_current_day: &Vec<PackageId>,
    ) -> Result<A::Digest, ()> {
        let hashed_package_counts = convert_package_counts(&catch_up_proof.bod_package_counts);
        if !A::verify_batch(
            self.inner.digest.as_ref().unwrap(),
            &hashed_package_counts,
            catch_up_proof.bod_package_membership_witness.clone(),
        ) {
            return Err(());
        }
        let mut package_counts = catch_up_proof.bod_package_counts.clone();

        for package in self.pool.iter().chain(rest_of_current_day) {
            match package_counts.get_mut(&package) {
                Some(r) => {
                    *r += 1;
                }
                None => {
                    return Err(()); // missing from current_revisions
                }
            }
        }
        let hashed_package_counts = convert_package_counts(&package_counts);
        if !A::verify_batch(
            &catch_up_proof.eod_digest,
            &hashed_package_counts,
            catch_up_proof.eod_package_membership_witness.clone(),
        ) {
            return Err(());
        }
        if !A::verify_append_only(
            self.inner.digest.as_ref().unwrap(),
            &catch_up_proof.bod_to_eod,
            &catch_up_proof.eod_digest,
        ) {
            return Err(());
        }
        Ok(catch_up_proof.eod_digest.clone())
    }

    fn batch_verify(&self, mut packages: HashMap<PackageId, u32>, proof: A::BatchWitness) -> bool {
        // Subtract out the packages that appear in "self.pool" so that we can
        // check "packages" against "self.inner".
        for pool_package in &self.pool {
            if let Some(revision) = packages.get_mut(pool_package) {
                if *revision == 0 {
                    // The counts in "packages" CANNOT be correct becaus
                    // "pool_package" appears more times in "self.pool" than in
                    // "packages".
                    return false;
                }
                *revision -= 1;
            }
        }
        let members = convert_package_counts(&packages);
        A::verify_batch(self.inner.digest.as_ref().unwrap(), &members, proof) // members[foo] = 0; => check nonmembership of "foo"
    }
}

#[derive(Derivative)]
#[derivative(Debug(
    bound = "A: std::fmt::Debug, A::BatchWitness: std::fmt::Debug, A::AppendOnlyWitness: std::fmt::Debug"
))]
#[derivative(Clone(bound = "A: Clone, A::BatchWitness: Clone, A::AppendOnlyWitness: Clone"))]
#[derivative(Default(
    bound = "A: Default, A::Digest: Default, A::BatchWitness: Default, A::AppendOnlyWitness: Default"
))]
struct Epoch<A: BatchAccumulator> {
    packages: Vec<PackageId>,
    eod_digest: A::Digest,
    // only for the things that got updated this epoch
    bod_package_counts: HashMap<PackageId, u32>,
    // only for the things that got updated this epoch
    eod_package_counts: HashMap<PackageId, u32>,
    bod_package_membership_witness: A::BatchWitness,
    eod_package_membership_witness: A::BatchWitness,
    bod_to_eod: A::AppendOnlyWitness,
}

impl<A: BatchAccumulator> DataSized for Epoch<A>
where
    A::Digest: DataSized,
    A::BatchWitness: DataSized,
    A::AppendOnlyWitness: DataSized,
{
    fn size(&self) -> Information {
        let mut size = self.eod_digest.size()
            + self.bod_package_membership_witness.size()
            + self.eod_package_membership_witness.size()
            + self.bod_to_eod.size();
        if self.packages.len() > 0 {
            let len: u64 = self.packages.len().try_into().unwrap();
            size += len * self.packages[0].size();
        }
        if self.bod_package_counts.len() > 0 {
            let len: u64 = self.bod_package_counts.len().try_into().unwrap();
            let item = self.bod_package_counts.keys().next();
            // val is usize
            let val = Information::new::<byte>(8);
            size += (item.expect(" ").size() + val) * len;
        }

        if self.eod_package_counts.len() > 0 {
            let len: u64 = self.eod_package_counts.len().try_into().unwrap();
            let item = self.eod_package_counts.keys().next();
            // val is usize
            let val = Information::new::<byte>(8);
            size += (item.expect(" ").size() + val) * len;
        }
        size
    }
}

#[derive(Derivative)]
#[derivative(Clone(bound = "A: Clone, Epoch<A>: Clone, A::Digest: Clone"))]
#[derivative(Debug(
    bound = "A: std::fmt::Debug, Epoch<A>: std::fmt::Debug, <A as Accumulator>::Digest: std::fmt::Debug"
))]
#[derivative(Default(
    bound = "A: Default, <A as Accumulator>::Digest: Clone + std::fmt::Debug + std::hash::Hash + Eq"
))]
pub struct PoolAuthenticator<A: BatchAccumulator> {
    inner: Authenticator<A>,
    past_epochs: Vec<Epoch<A>>,
    epoch_idxs_by_digest: HashMap<<A as Accumulator>::Digest, usize>,
    current_pool: Vec<PackageId>,
}

#[derive(Derivative, Serialize, Clone)]
// #[derivative(Clone(bound = "A::Witness: Clone, D::NonMembershipWitness: Clone"))]
pub enum PoolWitness<A: Accumulator> {
    Member(A::Witness),
    Nonmember(A::NonMembershipWitness),
}

impl<A: Accumulator> DataSized for PoolWitness<A>
where
    A::Witness: DataSized,
    A::NonMembershipWitness: DataSized,
{
    fn size(&self) -> Information {
        match self {
            Self::Member(inner) => inner.size(),
            Self::Nonmember(inner) => inner.size(),
        }
    }
}

#[allow(unused_variables)]
impl<A> super::Authenticator for PoolAuthenticator<A>
where
    A: BatchAccumulator<BatchDigest = <A as Accumulator>::Digest> + Clone,
    PoolDiff<A>: Serialize + Clone + DataSized,
    A::Witness: Serialize + Clone + DataSized,
    A::Digest: Default + Clone + Eq + hash::Hash,
    Authenticator<A>: BatchAuthenticator<BatchProof = <A as BatchAccumulator>::BatchWitness>
        + super::Authenticator<
            Id = Option<A::Digest>,
            Diff = Diff<A>,
            ClientSnapshot = Snapshot<A>,
            Proof = A::Witness,
        >,
    A::BatchWitness: Clone + Serialize,
    A::AppendOnlyWitness: Clone + Default,
    PoolWitness<A>: Clone + DataSized + Serialize,
    Epoch<A>: Clone,
    PoolAuthenticator<A>: DataSized,
    PoolSnapshot<A>: DataSized,
{
    type ClientSnapshot = PoolSnapshot<A>;
    type Id = Option<(A::Digest, usize)>;
    type Diff = PoolDiff<A>;
    type Proof = PoolWitness<A>;

    fn batch_import(packages: Vec<PackageId>) -> Self {
        let mut inner = Authenticator::<A>::batch_import(packages.clone());
        let (eod_package_counts, eod_package_membership_witness) =
            inner.batch_prove(packages.clone());
        let epoch: Epoch<A> = Epoch {
            packages,
            eod_digest: <A as Accumulator>::Digest::default(),
            bod_package_counts: Default::default(),
            eod_package_counts,
            bod_package_membership_witness: eod_package_membership_witness.clone(), // total lie but it typechecks
            eod_package_membership_witness,
            bod_to_eod: Default::default(), // total lie but it typechecks
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

    fn refresh_metadata(&self, snapshot_id: Self::Id) -> Option<PoolDiff<A>> {
        if snapshot_id.is_none() {
            let diff = self.inner.refresh_metadata(None).unwrap();
            return Some(PoolDiff::initial(diff.digest, self.current_pool.clone()));
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
            let current_day_final_digest =
                CatchUpToEODProof::from_epoch(epoch.clone(), next_digest.clone());
            Some(PoolDiff::for_next_day(
                rest_of_current_day,
                current_day_final_digest,
                self.current_pool.clone(),
            ))
        } else {
            // >one day behind
            // get *append only* from eod_digest to latest_digest
            let next_digest = &self.past_epochs[epoch_idx + 1].eod_digest;
            let append_only_witness = self.inner.acc.prove_append_only(next_digest);
            let latest_digest = (self.inner.acc.digest().clone(), append_only_witness);
            Some(PoolDiff::for_latter_day(
                rest_of_current_day,
                CatchUpToEODProof::from_epoch(epoch.clone(), next_digest.clone()),
                latest_digest,
                self.current_pool.clone(),
            ))
        }
    }

    fn publish(&mut self, package: PackageId) {
        // If package is new, then we need to precompute a nonmembership proof
        // for it against self.inner.
        let value = hash_package(&package);
        // We're precomputing the nonmembership proof *for the side effect* of
        // adding it to the cache. If value is already in the accumulator, this
        // does nothing.
        self.inner.acc.prove_nonmember(&value);
        self.current_pool.push(package);
    }

    fn request_file(
        &mut self,
        snapshot_id: Option<(<A as Accumulator>::Digest, usize)>,
        package: &PackageId,
    ) -> (Revision, Self::Proof) {
        let (inner_snapshot, pool_size) = snapshot_id.unwrap();
        let _ = pool_size;
        // assert_eq!(pool_size, self.current_pool.size());
        let value = hash_package(&package);
        let mut revision = self.inner.acc.get(&value);
        // let (bod_revision, bod_membership_proof) =
        //     self.inner.request_file(Some(inner_snapshot), package);
        // let revision = bod_revision + self.current_pool.iter().filter(|p| p == &package).count();
        // (revision, bod_membership_proof)
        let proof = if revision > 0 {
            PoolWitness::Member(
                self.inner
                    .acc
                    .prove(&value, revision)
                    .expect("proof failed"),
            )
        } else {
            PoolWitness::Nonmember(self.inner.acc.prove_nonmember(&value).unwrap())
        };
        let count: u32 = self
            .current_pool
            .iter()
            .filter(|p| p == &package)
            .count()
            .try_into()
            .unwrap();
        revision += count;
        let revision: NonZeroU64 = u64::from(revision).try_into().unwrap();
        let revision = Revision::from(revision);
        (revision, proof)
    }

    fn name() -> &'static str {
        "rsa_pool"
    }

    fn get_metadata(&self) -> Self::ClientSnapshot {
        let snapshot: Snapshot<A> = Snapshot::new(self.inner.acc.digest().clone());
        PoolSnapshot {
            inner: snapshot,
            pool: self.current_pool.clone(),
        }
    }
    fn id(snapshot: &Self::ClientSnapshot) -> Self::Id {
        Some((
            snapshot.inner.digest.as_ref().unwrap().clone(),
            snapshot.pool.len(),
        ))
    }

    fn update(snapshot: &mut Self::ClientSnapshot, mut diff: Self::Diff) {
        let eod_digest: A::Digest = match diff.current_day_final_digest {
            Some(catch_up_proof) => catch_up_proof.eod_digest, // The next digest is ready; we may want to update to that.
            None => {
                // Still in the same day. No new digest.
                snapshot.pool.append(&mut diff.rest_of_current_day);
                return;
            }
        };
        snapshot.inner = Snapshot::new(match diff.latest_digest {
            Some((d, _)) => d,  // Use the latest digest.
            None => eod_digest, // No "latest" digest; use the one from end-of-current-day.
        });
        // The latest pool can apply against either the current day's final
        // digest or the latest digest.
        snapshot.pool = diff.latest_pool;
    }

    // TODO(maybe): verify that we're doing special-case for RSA accumulators
    fn check_no_rollback(snapshot: &Self::ClientSnapshot, diff: &Self::Diff) -> bool {
        match (
            diff.current_day_final_digest.as_ref(),
            diff.latest_digest.as_ref(),
        ) {
            (Some(catch_up_proof), Some(latest_digest)) => {
                if let Ok(eod_digest) =
                    snapshot.validate_catch_up_proof(catch_up_proof, &diff.rest_of_current_day)
                {
                    let eod_snapshot = Snapshot::new(eod_digest);
                    let (d, w) = latest_digest;
                    let diff = Diff::new(d.clone(), Some(w.clone()));
                    if !Authenticator::<A>::check_no_rollback(&eod_snapshot, &diff) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            (Some(catch_up_proof), None) => {
                if snapshot
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
        snapshot: &Self::ClientSnapshot,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        let bod_revision = revision - snapshot.pool.iter().filter(|p| p == &package_id).count();
        match proof {
            PoolWitness::Member(proof) => Authenticator::<A>::verify_membership(
                &snapshot.inner,
                package_id,
                bod_revision,
                proof,
            ),
            PoolWitness::Nonmember(_) => false,
        }
    }

    fn cdn_size(&self) -> Information {
        self.inner.cdn_size()
            + Information::new::<byte>((self.current_pool.len() * STRING_BYTES).try_into().unwrap())
    }
}

impl<A: BatchAccumulator> DataSized for PoolAuthenticator<A>
where
    A::Digest: DataSized,
    Authenticator<A>: DataSized,
    Epoch<A>: DataSized,
{
    fn size(&self) -> Information {
        let mut size = self.inner.size();
        let len: u64 = self.past_epochs.len().try_into().unwrap();
        size += len * Information::new::<byte>(32);

        if self.epoch_idxs_by_digest.len() > 0 {
            let len: u64 = self.epoch_idxs_by_digest.len().try_into().unwrap();
            let item = self.epoch_idxs_by_digest.keys().next();
            // val is usize
            let val = Information::new::<byte>(8);
            size += (item.expect(" ").size() + val) * len;
        }

        if self.current_pool.len() > 0 {
            let len: u64 = self.current_pool.len().try_into().unwrap();
            size += len * self.current_pool[0].size();
        }
        size
    }
}
