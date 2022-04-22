use std::collections::HashMap;
use crate::rsa_accumulator::RsaAccumulatorDigest;
use bincode::Encode;

use authenticator::{ClientSnapshot, Revision, Hash};

use crate::{
    authenticator,
    log::PackageId,
    util::{DataSize, DataSized},
};


#[derive(Default, Clone, Debug)]
pub struct Snapshot {
    rsa_state: RsaAccumulatorDigest
}

impl Snapshot {
    pub fn new(state: RsaAccumulatorDigest) -> Self {
        Self { state }
    }
}

impl DataSized for Snapshot {
    fn size(&self) -> DataSize {
        self.rsa_state.size()
    }
}

#[derive(Debug, Clone)]
pub struct Proof(Hash, Integer);

impl DataSized for Proof {
    fn size(&self) -> DataSize {
        DataSized::from_bytes(self.0.size().to_bytes() + self.1.size().to_bytes())
    }
}

impl ClientSnapshot for Snapshot {
    type Id = RsaAccumulatorDigest;
    type Diff = (Self);
    type Proof = Proof;

    fn id(&self) -> Self::Id {
        self.rsa_state.clone()
    }

    fn update(&mut self, diff: Self::Diff) {
        self.rsa_state = diff;
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
        let (h, p) = proof;
        let encoded = bincode::serialize(&(h, revision, package_id)).unwrap();
        let prime = division_intractable_hash(encoded, Modulus);
        if !self.rsa_state.verify(prime, p) {
            return false;
        }
        true
    }
}

#[derive(Default, Debug)]
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
        if snapshot_id == self.rsa_acc.digest() {
            return None;
        }
        return Some(self.rsa_acc.digest())
    }

    fn publish(&mut self, package: &PackageId) -> () {
        let entry = self.revisions.entry(package.clone());
        let mut revision = entry.or_insert_with(Revision::default);
        revision.0 += 1;

        if revision.0 != 1 {
            let h = Hash::default();
            let encoded = bincode::serialize(&(h, revision.0 - 1, package_id)).unwrap();
            let prime = division_intractable_hash(encoded, Modulus);
            self.rsa_acc.remove(prime);
        }
        let h = Hash::default();
        let encoded = bincode::serialize(&(h, revision.0, package_id)).unwrap();
        let prime = division_intractable_hash(encoded, Modulus);
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
        let encoded = bincode::serialize(&(h, revision, package_id)).unwrap();
        let prime = division_intractable_hash(encoded, Modulus);

        let proof = rsa_acc.prove(prime);
        (*revision, Proof(proof))
    }
}

impl DataSized for Authenticator {
    fn size(&self) -> DataSize {
        // TODO: better to serialize then figure out the size?
        // also gzip?
        let mut snapshot_size: u64 =
            TryInto::try_into(std::mem::size_of::<Self>()).expect("Not that big");
        for (package_id, revision) in self.revisions.iter() {
            snapshot_size += package_id.size().bytes();
            snapshot_size += revision.size().bytes();
        }

        let rsa_size = rsa_acc.size().to_bytes();

        DataSize::from_bytes(snapshot_size + rsa_size)
    }
}

#[cfg(test)]
mod tests {
    // TODO: fix tests
}
