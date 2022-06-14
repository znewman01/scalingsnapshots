//! Concepts for log inputs.
//!
//! Most package repos have a model that involves the concepts:
//!
//! - "files": individual artifacts that a user might download
//!
//! "release" -> "package" -> "version" -> "files"
//!
//! The TUF concepts are a little different. It's up to the Repository
//! Simulator to translate between them.
use serde::Deserialize;
use serde::Serialize;
use time::serde::format_description;
use time::OffsetDateTime;

#[cfg(test)]
use proptest_derive::Arbitrary;

format_description!(
    simple_dt_8601,
    OffsetDateTime,
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond] [offset_hour sign:mandatory][offset_minute]"
);

// Primitives

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct UserId(String);

impl From<String> for UserId {
    fn from(id: String) -> Self {
        UserId(id)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct PackageId(pub String);

impl From<PackageId> for String {
    fn from(id: PackageId) -> String {
        id.0
    }
}

impl From<String> for PackageId {
    fn from(id: String) -> Self {
        PackageId(id)
    }
}

// Concepts

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: PackageId,
    pub length: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Action {
    Download { user: UserId, package: Package },
    RefreshMetadata { user: UserId },
    Publish { package: Package },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
    #[serde(with = "simple_dt_8601")]
    pub timestamp: OffsetDateTime,
    pub action: Action,
}

impl Entry {
    #[must_use]
    pub fn new(timestamp: OffsetDateTime, action: Action) -> Self {
        Self { timestamp, action }
    }

    #[must_use]
    pub fn action(&self) -> &Action {
        &self.action
    }
}

#[derive(Debug)]
pub struct Log(Vec<Entry>);

impl From<Vec<Entry>> for Log {
    fn from(entries: Vec<Entry>) -> Self {
        // Log entries must be in sorted order by timestamp.
        let mut last: Option<OffsetDateTime> = None;
        for entry in &entries {
            if let Some(last) = last {
                assert!(entry.timestamp >= last, "Invalid log!");
            }
            last = Some(entry.timestamp);
        }

        Self(entries)
    }
}

impl IntoIterator for Log {
    type Item = Entry;
    type IntoIter = <Vec<Entry> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
