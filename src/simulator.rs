use std::collections::HashMap;

use crate::authenticator::ClientSnapshot;
use crate::log::{Action, PackageId, PackageRelease, QualifiedFiles, UserId};
use crate::tuf::{self, SnapshotMetadata};
use crate::util::{data_size_as_bytes, DataSize, DataSized};
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
pub struct ResourceUsage {
    /// Server-side computation time used to handle this request.
    #[serde(rename = "server_compute_ns", serialize_with = "serialize_ns")]
    server_compute: Duration,
    /// Client-side computation time used to handle this request.
    #[serde(rename = "user_compute_ns", serialize_with = "serialize_ns")]
    user_compute: Duration, // TODO: make optional
    #[serde(rename = "bandwidth_bytes", serialize_with = "data_size_as_bytes")]
    bandwidth: DataSize,
    #[serde(rename = "server_storage_bytes", serialize_with = "data_size_as_bytes")]
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
    authenticator: A,
    snapshots: HashMap<UserId, S>,
    tuf: SnapshotMetadata,
}

// TODO: investigate the clones, see if you can get rid of them
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

    fn process_download(&mut self, user: UserId, files: &QualifiedFiles) -> ResourceUsage {
        let file = &files.0[0]; // TODO: just pass in a QualifiedFile
        let user_snapshot = self.snapshots.entry(user).or_insert_with(Default::default);
        let user_snapshot_identifier = user_snapshot.id();
        let (server_request_time, proof) = Duration::time_fn(|| {
            self.authenticator
                .request_file(user_snapshot_identifier, file.clone().into())
        });
        let (user_verify_time, _) = Duration::time_fn(|| {
            assert!(user_snapshot.verify_membership(file.clone().into(), proof.clone()))
        });
        ResourceUsage {
            server_compute: server_request_time,
            user_compute: user_verify_time,
            bandwidth: proof.size(),
            storage: self.authenticator.size(),
        }
    }

    fn process_refresh_metadata(&mut self, user: UserId) -> ResourceUsage {
        // Get the snapshot ID for the user's current snapshot.
        let snapshot = self.snapshots.entry(user).or_insert_with(Default::default);
        let (user_compute_id_time, old_snapshot_id) = Duration::time_fn(|| snapshot.id());

        // Answer the update metadata server-side.
        let (server_compute, snapshot_diff) =
            Duration::time_fn(|| self.authenticator.refresh_metadata(&old_snapshot_id));

        // Check the new snapshot for rollbacks and store it.
        let snapshot = self
            .snapshots
            .get_mut(&user)
            .expect("Snapshot was populated, but was then empty.");
        let (user_compute_verify, _) = Duration::time_fn(|| {
            assert!(snapshot.check_no_rollback(&snapshot_diff));
        });
        let (user_compute_update, _) = Duration::time_fn(|| {
            snapshot.update(snapshot_diff.clone());
        });

        ResourceUsage {
            server_compute,
            user_compute: user_compute_id_time + user_compute_verify + user_compute_update,
            bandwidth: snapshot_diff.size(),
            storage: self.authenticator.size(),
        }
    }

    fn process_publish(&mut self, package: PackageId, release: PackageRelease) -> ResourceUsage {
        let files: Vec<tuf::File> = release
            .clone()
            .files()
            .into_iter()
            .map(tuf::File::from)
            .collect();
        self.tuf.upsert_target(package.into(), files);
        let (server_upload, _) = Duration::time_fn(|| self.authenticator.publish(&release));
        ResourceUsage {
            server_compute: server_upload,
            user_compute: Duration::ZERO,
            bandwidth: DataSize::zero(),
            storage: self.authenticator.size(),
        }
    }

    pub fn process(&mut self, action: Action) -> ResourceUsage {
        // TODO: batch publish?
        match action {
            Action::Download { user, files } => self.process_download(user, &files),
            Action::RefreshMetadata { user } => self.process_refresh_metadata(user),
            Action::Publish { package, release } => self.process_publish(package, release),
        }
    }
}
