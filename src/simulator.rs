use crate::log::Entry;
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
pub struct ResourceUsage {
    /// Server-side computation time used to handle this request.
    #[serde(rename = "server_compute_ns", serialize_with = "serialize_ns")]
    server_compute: Duration,
    /// Client-side computation time used to handle this request.
    #[serde(rename = "user_compute_ns", serialize_with = "serialize_ns")]
    user_compute: Duration,
    // TODO: bandwidth
    // TODO: storage
}

/// A simulator for a secure software repository.
///
/// Handles what we care about (timing, bandwidth, storage) and ignores what we
/// don't (actual network calls).
///
/// TODO: should be just a function?
#[derive(Debug)]
pub struct Simulator<A> {
    authenticator: A,
}

impl<A: Authenticator> Simulator<A> {
    pub fn new(authenticator: A) -> Self {
        Self { authenticator }
    }

    pub fn process(&self, _entry: &Entry) -> ResourceUsage {
        ResourceUsage {
            server_compute: Duration::zero(),
            user_compute: Duration::zero(),
        }
    }
}
