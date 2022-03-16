use std::collections::HashMap;

use authenticator::ClientSnapshot;

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use crate::{
    authenticator,
    log::FileName,
    util::{DataSize, DataSized},
};

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug, Clone)]
pub struct Metadata {
    revision: u64,
}

impl DataSized for Metadata {
    fn size(&self) -> DataSize {
        DataSize::from_bytes(std::mem::size_of::<Self>().try_into().unwrap())
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Clone, Debug)]
pub struct Snapshot {
    files: HashMap<FileName, Metadata>,
    id: u64,
}

impl DataSized for Snapshot {
    fn size(&self) -> DataSize {
        // TODO: better to serialize then figure out the size?
        // also gzip?
        let mut size: u64 = TryInto::try_into(std::mem::size_of::<Self>()).expect("Not that big");
        for (filename, metadata) in self.files.iter() {
            size += filename.size().bytes();
            size += metadata.size().bytes();
        }
        DataSize::from_bytes(size)
    }
}

/// The vanilla TUF client snapshot contains *all* the snapshot state.
impl ClientSnapshot for Snapshot {
    type Id = u64;
    type Diff = Snapshot;
    type Proof = ();

    fn id(&self) -> Self::Id {
        self.id
    }

    fn update(&mut self, diff: Self::Diff) {
        self.files = diff.files;
        self.id = diff.id
    }

    fn check_no_rollback(&self, diff: &Self::Diff) -> bool {
        for (file, metadata) in self.files.iter() {
            let new_metadata = match diff.files.get(&file) {
                None => {
                    return false;
                }
                Some(m) => m,
            };
            if !(new_metadata.revision >= metadata.revision) {
                return false;
            }
        }
        true
    }

    fn verify_membership(&self, file: FileName, _: Self::Proof) -> bool {
        self.files.get(&file).is_some()
    }
}

/// An authenticator as-in vanilla TUF.
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Default, Debug)]
pub struct Authenticator {
    snapshot: Snapshot,
}

#[allow(unused_variables)]
impl authenticator::Authenticator<Snapshot> for Authenticator {
    fn refresh_metadata(
        &self,
        snapshot_id: &<Snapshot as ClientSnapshot>::Id,
    ) -> Option<<Snapshot as ClientSnapshot>::Diff> {
        if snapshot_id == &self.snapshot.id() {
            // already up to date
            return None;
        }
        Some(self.snapshot.clone())
    }

    fn publish(&mut self, file: &FileName) -> () {
        self.snapshot.id += 1;
        let entry = self.snapshot.files.entry(file.clone());
        let mut metadata = entry.or_insert_with(Metadata::default);
        metadata.revision += 1;
    }

    fn request_file(
        &self,
        snapshot_id: <Snapshot as ClientSnapshot>::Id,
        file: FileName,
    ) -> <Snapshot as ClientSnapshot>::Proof {
        ()
    }
}

impl DataSized for Authenticator {
    fn size(&self) -> DataSize {
        self.snapshot.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authenticator::tests;

    proptest! {
        #[ignore] // TODO: fix tests::update
        #[test]
        fn update((authenticator, snapshot) in (any::<Authenticator>(), any::<Snapshot>())) {
            tests::update(snapshot, authenticator)?;
        }
    }
}
