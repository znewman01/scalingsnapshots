use std::collections::HashMap;

use crate::authenticator::ClientSnapshot;
use crate::log::{Action, PackageId, PackageRelease, QualifiedFiles, UserId};
use crate::tuf::{self, SnapshotMetadata};
use crate::Authenticator;
use serde::{Serialize, Serializer};
use time::Duration;

fn serialize_ns<S>(duration: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_i64(
        duration
            .whole_nanoseconds()
            .try_into()
            .expect("too many nanos"),
    )
}

#[derive(Debug, Serialize)]
pub struct DataSize {
    bytes: u64,
}

impl DataSize {
    pub fn bytes(bytes: u64) -> DataSize {
        DataSize { bytes }
    }
}

#[derive(Debug, Serialize)]
pub struct ResourceUsage {
    /// Server-side computation time used to handle this request.
    #[serde(rename = "server_compute_ns", serialize_with = "serialize_ns")]
    server_compute: Duration,
    /// Client-side computation time used to handle this request.
    #[serde(rename = "user_compute_ns", serialize_with = "serialize_ns")]
    user_compute: Duration, // TODO: make optional
    #[serde(flatten)]
    bandwidth: DataSize,
    #[serde(flatten)]
    storage: DataSize,
}

/// A simulator for a secure software repository.
///
/// Handles what we care about (timing, bandwidth, storage) and ignores what we
/// don't (actual network calls).
///
/// TODO: user state?
/// TODO: manage TUF repo?
#[derive(Debug)]
pub struct Simulator<S: ClientSnapshot, A: Authenticator<S>> {
    #[allow(dead_code)] // TODO: remove once this is actually implemented
    authenticator: A,
    snapshots: HashMap<UserId, S>,
    tuf: SnapshotMetadata,
}

#[allow(unused_variables)] // TODO: remove once this is actually implemented
impl<S: ClientSnapshot, A: Authenticator<S>> Simulator<S, A>
where
    S: Default,
{
    pub fn new(authenticator: A, tuf: SnapshotMetadata) -> Self {
        Self {
            authenticator,
            snapshots: HashMap::default(),
            tuf,
        }
    }

    #[allow(dead_code)] // TODO
    fn process_download(&self, user: UserId, files: &QualifiedFiles) -> ResourceUsage {
        // 1. server processes files request (may need data on what user has) -> proof
        //    - time this
        //    - measure proof bandwidth
        // 2. user verifies proof
        //    - time this
        // 3. measure authenticator storage size
        //    - TODO: think about how to account for TUF base
        // TODO: manage user metadata here? (configurable?)
        ResourceUsage {
            server_compute: Duration::ZERO,
            user_compute: Duration::ZERO,
            bandwidth: DataSize::bytes(0),
            storage: DataSize::bytes(0),
        }
    }

    fn process_refresh_metadata(&mut self, user: UserId) -> ResourceUsage {
        // Get the snapshot ID for the user's current snapshot.
        let (old_snapshot_id, user_compute_id_time) = {
            let snapshot = self.snapshots.entry(user).or_insert_with(Default::default);
            time(|| snapshot.id())
        };

        // Answer the update metadata server-side.
        let (server_compute, snapshot_diff) =
            Duration::time_fn(|| self.authenticator.refresh_metadata(&old_snapshot_id));
        let bandwidth = DataSize::bytes(0); // TODO: implement on new_snapshot
        let storage = DataSize::bytes(0); // TODO: implement on Authenticator

        // Check the new snapshot for rollbacks and store it.
        let snapshot = self.snapshots.get_mut(&user).expect("Snapshot was populated, but was then empty.");
        let user_compute_verify = Duration::span(|| {
            assert!(snapshot.check_no_rollback(&snapshot_diff));
        });
        snapshot.update(snapshot_diff);

        ResourceUsage {
            server_compute,
            user_compute: user_compute_id_time + user_compute_verify,
            bandwidth,
            storage,
        }
    }

    fn process_publish(&mut self, package: PackageId, release: PackageRelease) -> ResourceUsage {
        let files: Vec<tuf::File> = release.files().into_iter().map(tuf::File::from).collect();
        self.tuf.upsert_target(package.into(), files);
        // 1. server updates
        //    - TODO: update all proofs? or lazily? or batch?
        //    - time this
        // 2. measure storage size
        // no user time, bandwidth
        ResourceUsage {
            server_compute: Duration::ZERO,
            user_compute: Duration::ZERO,
            bandwidth: DataSize::bytes(0),
            storage: DataSize::bytes(0),
        }
    }

    pub fn process(&mut self, action: Action) -> ResourceUsage {
        match action {
            Action::Download { user, files } => self.process_download(user, &files),
            Action::RefreshMetadata { user } => self.process_refresh_metadata(user),
            Action::Publish { package, release } => self.process_publish(package, release),
        }
    }
}
