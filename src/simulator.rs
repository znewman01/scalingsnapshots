use crate::log::{Action, FilesRequest, PackageId, PackageRelease, UserId};
use crate::Authenticator;
use chrono::Duration;
use serde::{Serialize, Serializer};

fn serialize_ns<S>(duration: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_i64(duration.num_nanoseconds().expect("too many nanos"))
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
pub struct Simulator<A: Authenticator> {
    #[allow(dead_code)] // TODO: remove once this is actually implemented
    authenticator: A,
}

#[allow(unused_variables)] // TODO: remove once this is actually implemented
impl<A: Authenticator> Simulator<A> {
    pub fn new(authenticator: A) -> Self {
        Self { authenticator }
    }

    #[allow(dead_code)] // TODO
    fn process_download(&self, user: UserId, files: &FilesRequest) -> ResourceUsage {
        // 1. server processes files request (may need data on what user has) -> proof
        //    - time this
        //    - measure proof bandwidth
        // 2. user verifies proof
        //    - time this
        // 3. measure authenticator storage size
        //    - TODO: think about how to account for TUF base
        // TODO: manage user metadata here? (configurable?)
        ResourceUsage {
            server_compute: Duration::zero(),
            user_compute: Duration::zero(),
            bandwidth: DataSize::bytes(0),
            storage: DataSize::bytes(0),
        }
    }

    #[allow(dead_code)] // TODO
    fn process_refresh_metadata(&self, user: UserId) -> ResourceUsage {
        // 1. server gives client new metadata, along with (optional) rollback proof
        //    - time this
        //    - measure proof bandwidth
        // 2. user verifies rollback (optional)
        //    - measure user time
        // 3. measure storage size? (shouldn't change)
        ResourceUsage {
            server_compute: Duration::zero(),
            user_compute: Duration::zero(),
            bandwidth: DataSize::bytes(0),
            storage: DataSize::bytes(0),
        }
    }

    fn process_publish(&self, package: &PackageId, release: &PackageRelease) -> ResourceUsage {
        // 1. server updates
        //    - TODO: update all proofs? or lazily? or batch?
        //    - time this
        // 2. measure storage size
        // no user time, bandwidth
        ResourceUsage {
            server_compute: Duration::zero(),
            user_compute: Duration::zero(),
            bandwidth: DataSize::bytes(0),
            storage: DataSize::bytes(0),
        }
    }

    pub fn process(&self, action: &Action) -> ResourceUsage {
        match action {
            Action::Download { user, files } => self.process_download(*user, files),
            Action::RefreshMetadata { user } => self.process_refresh_metadata(*user),
            Action::Publish { package, release } => self.process_publish(package, release),
        }
    }
}
