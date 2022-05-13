use crate::{
    hash_to_prime::division_intractable_hash,
    rsa_accumulator::{RsaAccumulator, RsaAccumulatorDigest, Witness, MODULUS},
};
use serde::Serialize;

use authenticator::{ClientSnapshot, Revision};

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

impl ClientSnapshot for Snapshot {
    type Id = RsaAccumulatorDigest;
    type Diff = Self;
    type Proof = Witness;

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
        let encoded = bincode::serialize(package_id).unwrap();
        let prime = division_intractable_hash(&encoded, &MODULUS);
        if !self
            .rsa_state
            .verify(&prime, revision.0.try_into().unwrap(), proof)
        {
            return false;
        }
        true
    }
}

#[derive(Default, Debug, Serialize)]
pub struct Authenticator {
    rsa_acc: RsaAccumulator,
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
        let encoded = bincode::serialize(package).unwrap();
        let prime = division_intractable_hash(&encoded, &MODULUS);
        self.rsa_acc.increment(prime);
    }

    fn request_file(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        package: &PackageId,
    ) -> (Revision, <Snapshot as ClientSnapshot>::Proof) {
        let encoded = bincode::serialize(package).unwrap();
        let prime = division_intractable_hash(&encoded, &MODULUS);

        let revision = self.rsa_acc.get(&prime);

        let proof = self.rsa_acc.prove(&prime, revision).expect("proof failed");
        (Revision::from(u64::from(revision)), proof)
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}
