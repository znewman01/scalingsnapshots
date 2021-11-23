//! This file contains definitions for the subset of TUF concepts relevant to
//! this project.
//!
//! In particular, we're not signing/verifying anything or worrying about
//! compatibility/formats, which lets us get rid of a lot of fields.
//!
//! # TUF concepts
//!
//! See: <https://theupdateframework.io/metadata/>.
//!
//! ## Offline role
//!
//! Fixed set of keys that should be stored offline.
//!
//! *Updated:* never.

//! In this project, we don't care about offline keys.
//!
//! ## Root metadata
//!
//! Lists keys associated with other roles (signed by the root offline keys)
//!
//! *Updated:* only after compromise or change in the top-level roles.
//!
//! In this project, we don't care about root metadata.
//!
//! ## (Delegated) targets metadata
//!
//! Lists files available for download.
//!
//! In practice, will consist of:
//!
//! 1. Offline top-level role that delegates to other keys for each package.
//! 2. A delegated target for each package containing all files for all versions
//!    of the package.
//!
//! We only use one level of delegations for this simulation.
//!
//! *Updated:* when a *new* package is added or package ownership changes.
//!
//! ## - Snapshot metadata
//!
//! Signs all the target metadata files (with associated version numbers). This
//! is what we're trying to compress in this project.
//!
//! *Updated:* with each package update (possibly batched).
//!
//! ## Timestamp metadata
//!
//! Signs the snapshots file with a date, indicating that it's current.
//!
//! In this project, we don't care about timestamp metadata.
//!
//! # Scaling TUF
//!
//! Normally, a user has to keep track of all of the above.
//!
//! We'd prefer instead to replace the snapshot metadata with one of many
//! possible more-efficient representations.
//!
//! Bonus: consider replacing top-level targets too?

#[cfg(test)]
use {proptest::prelude::*, proptest_derive::Arbitrary};

use std::collections::{hash_map::Entry, HashMap};

// TODO: implement using im <https://docs.rs/im/15.0.0/im/>?

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, Hash, Clone, Copy, Default, Eq, PartialEq)]
pub struct Revision(u32);

impl From<u32> for Revision {
    fn from(value: u32) -> Self {
        Revision(value)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct FileName(String);

impl From<String> for FileName {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<FileName> for String {
    fn from(name: FileName) -> Self {
        name.0
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct File {
    name: FileName,
    length: Option<u64>,
}

impl File {
    pub fn new(name: FileName, length: Option<u64>) -> Self {
        Self { name, length }
    }

    pub fn name(self) -> FileName {
        self.name
    }

    pub fn length(&self) -> Option<u64> {
        self.length
    }
}

#[derive(Debug, Hash, Default, Clone)]
pub struct TargetMetadata {
    /// List of paths in this target.
    files: Vec<File>,
    revision: Revision,
    // We omit: delegations, expires, spec_version, _type
}

#[cfg(test)]
impl Arbitrary for TargetMetadata {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (
            any::<Revision>(),
            prop::collection::vec(any::<File>(), 0..10),
        )
            .prop_map(|(revision, files)| TargetMetadata { files, revision })
            .boxed()
    }
}

impl TargetMetadata {
    pub fn initial(files: Vec<File>) -> Self {
        Self {
            files,
            revision: Revision(0),
        }
    }

    pub fn update(&self, files: Vec<File>) -> Self {
        Self {
            files,
            revision: Revision(
                self.revision
                    .0
                    .checked_add(1)
                    .expect("Targets revision number overflow!"),
            ),
        }
    }
}

#[cfg(test)]
mod targets_tests {
    use super::*;

    proptest! {
        #[test]
        fn initial_revision_zero(files: Vec<File>) {
            let targets = TargetMetadata::initial(files.clone());

            prop_assert_eq!(targets.revision, 0.into());
            prop_assert_eq!(targets.files, files);
        }

        #[test]
        fn later_revisions_increment(targets: TargetMetadata, files: Vec<File>) {
            let targets_new = targets.update(files.clone());

            prop_assert_eq!(targets_new.files, files);
            prop_assert_eq!(targets_new.revision, Revision(targets.revision.0 + 1));
        }

        #[test]
        #[should_panic]
        fn increment_max_revision(old_files: Vec<File>, new_files: Vec<File>) {
            let targets = TargetMetadata {
                files: old_files,
                revision: Revision(u32::MAX)
            };

            targets.update(new_files);
        }
    }
}

#[derive(Debug, Default)]
pub struct SnapshotMetadata {
    targets: HashMap<String, TargetMetadata>,
    revision: Revision,
    // We omit: expires, spec_version, signatures, _type
}

#[cfg(test)]
fn targets_hash_maps() -> impl Strategy<Value = HashMap<String, TargetMetadata>> {
    prop::collection::hash_map(any::<String>(), any::<TargetMetadata>(), 0..10)
}

#[cfg(test)]
impl Arbitrary for SnapshotMetadata {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (any::<Revision>(), targets_hash_maps())
            .prop_map(|(revision, targets)| SnapshotMetadata { targets, revision })
            .boxed()
    }
}

impl SnapshotMetadata {
    pub fn upsert_target(&mut self, name: String, files: Vec<File>) {
        match self.targets.entry(name) {
            Entry::Occupied(e) => {
                let new_target = e.get().update(files);
                e.replace_entry(new_target);
            }
            Entry::Vacant(e) => {
                e.insert(TargetMetadata::initial(files));
            }
        }
        self.revision.0 = self
            .revision
            .0
            .checked_add(1)
            .expect("Snapshots revision number overflow!");
    }
}

#[cfg(test)]
mod snapshots_tests {
    use super::*;

    #[test]
    fn empty_snapshot() {
        let snapshot = SnapshotMetadata::default();

        assert_eq!(snapshot.targets.len(), 0);
        assert_eq!(snapshot.revision, 0.into());
    }

    proptest! {
        #[test]
        #[should_panic]
        fn increment_max(targets in targets_hash_maps(), target_name : String, files: Vec<File>) {
            let mut snapshots = SnapshotMetadata {
                targets,
                revision: Revision(u32::MAX)
            };

            snapshots.upsert_target(target_name.clone(), files.clone());
        }

        #[test]
        fn test_upsert_new(mut snapshots: SnapshotMetadata, target_name: String, files: Vec<File>) {
            prop_assume!(snapshots.targets.get(&target_name).is_none());
            let old_revision = snapshots.revision;

            snapshots.upsert_target(target_name.clone(), files.clone());

            prop_assert_eq!(snapshots.revision, Revision(old_revision.0 + 1));
            let target = snapshots
                .targets
                .get(&target_name)
                .expect("SnapshotMetadata missing the target we just added!");
            prop_assert_eq!(target.revision, Revision(0));
            prop_assert_eq!(&target.files, &files);
        }

        #[test]
        fn test_upsert_existing(
            mut snapshots: SnapshotMetadata,
            target_name: String,
            files_old: Vec<File>,
            files_new: Vec<File>,
        ) {
            snapshots.upsert_target(target_name.clone(), files_old);

            let old_snapshots_revision = snapshots.revision;
            let old_targets_revision = snapshots.targets.get(&target_name).unwrap().revision;

            snapshots.upsert_target(target_name.clone(), files_new.clone());

            prop_assert_eq!(snapshots.revision, Revision(old_snapshots_revision.0 + 1));
            let target = snapshots.targets.get(&target_name).unwrap();
            prop_assert_eq!(target.revision, Revision(old_targets_revision.0 + 1));
            prop_assert_eq!(&target.files, &files_new);
        }
    }
}
