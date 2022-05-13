use crate::{
    hash_to_prime::division_intractable_hash,
    rsa_accumulator::{RsaAccumulator, RsaAccumulatorDigest, MODULUS},
};
use rug::Integer;
use serde::Serialize;
use std::collections::HashMap;

use authenticator::{ClientSnapshot, Hash, Revision};

use crate::{authenticator, log::PackageId};

#[derive(Default, Clone, Debug, Serialize)]
pub struct Snapshot {
    rsa_state: RsaAccumulatorDigest,
}

impl Snapshot {
    pub fn new(rsa_state: RsaAccumulatorDigest) -> Self {
        Self { rsa_state }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Proof(Hash, Integer);

impl ClientSnapshot for Snapshot {
    type Id = RsaAccumulatorDigest;
    type Diff = Self;
    type Proof = Proof;

    fn id(&self) -> Self::Id {
        self.rsa_state.clone()
    }

    fn update(&mut self, diff: Self::Diff) {
        self.rsa_state = diff.rsa_state;
    }

    fn check_no_rollback(&self, _diff: &Self::Diff) -> bool {
        //done by auditors
        true
    }

    fn verify_membership(
        &self,
        package_id: &PackageId,
        revision: Revision,
        proof: Self::Proof,
    ) -> bool {
        let (h, p) = (proof.0, proof.1);
        let encoded = bincode::serialize(&(h, revision, package_id)).unwrap();
        let prime = division_intractable_hash(&encoded, &MODULUS);
        if !self.rsa_state.verify(&prime, p) {
            return false;
        }
        true
    }
}

#[derive(Default, Debug, Serialize)]
pub struct Authenticator {
    rsa_acc: RsaAccumulator,
    revisions: HashMap<PackageId, Revision>,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        if &snapshot_id == self.rsa_acc.digest() {
            return None;
        }
        Some(Snapshot::new(self.rsa_acc.digest().clone()))
    }

    fn publish(&mut self, package: &PackageId) -> () {
        let entry = self.revisions.entry(package.clone());
        let mut revision = entry.or_insert_with(Revision::default);
        revision.0 += 1;

        if revision.0 != 1 {
            let h = Hash::default();
            let encoded = bincode::serialize(&(h, revision.0 - 1, package)).unwrap();
            let prime = division_intractable_hash(&encoded, &MODULUS);
            self.rsa_acc.remove(&prime);
        }
        let h = Hash::default();
        let encoded = bincode::serialize(&(h, revision.0, package)).unwrap();
        let prime = division_intractable_hash(&encoded, &MODULUS);
        self.rsa_acc.add(prime);
    }

    fn request_file(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot as ClientSnapshot>::Proof) {
        let h = Hash::default();
        let revision = self
            .revisions
            .get(package)
            .expect("Should never get a request for a package that's missing.");
        let encoded = bincode::serialize(&(h, revision, package)).unwrap();
        let prime = division_intractable_hash(&encoded, &MODULUS);

        let proof = self.rsa_acc.prove(&prime).expect("proof failed");
        (*revision, Proof(h, proof))
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}
