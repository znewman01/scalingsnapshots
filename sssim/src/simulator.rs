use std::collections::HashMap;

use crate::authenticator::ClientSnapshot;
use crate::log::{Action, Package, PackageId, UserId};
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

// TODO: split up by action, they all have different resource usages
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
    server_storage: DataSize,
    #[serde(rename = "user_storage_bytes", serialize_with = "data_size_as_bytes")]
    user_storage: DataSize,
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
    /// Keep track of the length of the latest version of each package, if provided.
    package_lengths: HashMap<PackageId, u64>,
}

// TODO: investigate the clones, see if you can get rid of them
impl<S: ClientSnapshot, A: Authenticator<S>> Simulator<S, A>
where
    S: Default + DataSized,
{
    pub fn new(authenticator: A) -> Self {
        Self {
            authenticator,
            snapshots: HashMap::default(),
            package_lengths: HashMap::default(),
        }
    }

    fn process_download(&mut self, user: UserId, package: &mut Package) -> ResourceUsage {
        if package.length.is_none() {
            // If package length is unset, set it to the length of the *latest* package in the map.
            package.length = self.package_lengths.get(&package.id).copied();
        }

        let user_snapshot = self.snapshots.entry(user).or_insert_with(Default::default);
        let (server_request_time, (revision, proof)) = Duration::time_fn(|| {
            self.authenticator
                .request_file(user_snapshot.id(), &package.id)
        });
        let bandwidth = proof.size();
        let (user_verify_time, _) = Duration::time_fn(|| {
            assert!(user_snapshot.verify_membership(&package.id, revision, proof))
        });

        ResourceUsage {
            server_compute: server_request_time,
            user_compute: user_verify_time,
            bandwidth,
            server_storage: self.authenticator.size(),
            user_storage: user_snapshot.size(),
        }
    }

    fn process_refresh_metadata(&mut self, user: UserId) -> ResourceUsage {
        // Get the snapshot ID for the user's current snapshot.
        let snapshot = self.snapshots.entry(user).or_insert_with(Default::default);

        // Answer the update metadata server-side.
        let (server_compute, maybe_snapshot_diff) =
            Duration::time_fn(|| self.authenticator.refresh_metadata(snapshot.id()));

        // TODO: make this a 2 part dance

        let snapshot_size = maybe_snapshot_diff.size();

        let user_compute = if let Some(snapshot_diff) = maybe_snapshot_diff {
            // Check the new snapshot for rollbacks and store it.
            let (user_compute_verify, _) = Duration::time_fn(|| {
                assert!(snapshot.check_no_rollback(&snapshot_diff));
            });
            let (user_compute_update, _) = Duration::time_fn(|| {
                snapshot.update(snapshot_diff);
            });
            user_compute_verify + user_compute_update
        } else {
            Duration::ZERO
        };
        ResourceUsage {
            server_compute,
            user_compute,
            bandwidth: snapshot_size,
            server_storage: self.authenticator.size(),
            user_storage: snapshot.size(),
        }
    }

    fn process_publish(&mut self, package: &Package) -> ResourceUsage {
        if let Some(length) = package.length {
            self.package_lengths.insert(package.id.clone(), length);
        }
        let (server_upload, _) = Duration::time_fn(|| self.authenticator.publish(&package.id));
        ResourceUsage {
            server_compute: server_upload,
            user_compute: Duration::ZERO,
            bandwidth: DataSize::zero(),
            server_storage: self.authenticator.size(),
            user_storage: DataSize::zero(),
        }
    }

    pub fn process(&mut self, action: &mut Action) -> ResourceUsage {
        // TODO: add a map of package name -> package number
        // TODO: when a new package comes in, give it a number (the lowest number that hasn't been given out)
        // TODO: batch publish?
        match action {
            Action::Download { user, package } => {
                self.process_publish(package); // TODO: this is bad
                self.process_refresh_metadata(user.clone());
                self.process_download(user.clone(), package)
            }
            Action::RefreshMetadata { user } => self.process_refresh_metadata(user.clone()),
            Action::Publish { package } => self.process_publish(package),
        }
    }
}
