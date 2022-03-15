#![allow(dead_code)] // TODO: fix once we're actually using this
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
use itertools::Itertools;
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
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct UserId(u64);

impl From<u64> for UserId {
    fn from(id: u64) -> Self {
        UserId(id)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct PackageId(String);

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

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct FileName(String);

impl From<String> for FileName {
    fn from(id: String) -> Self {
        FileName(id)
    }
}

impl From<FileName> for String {
    fn from(name: FileName) -> Self {
        name.0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct PackageReleaseId(String);

impl From<String> for PackageReleaseId {
    fn from(id: String) -> Self {
        PackageReleaseId(id)
    }
}

// Concepts

#[derive(Debug)]
pub struct Release {
    packages: Vec<Package>,
}

#[derive(Debug)]
pub struct Package {
    id: PackageId,
    releases: Vec<PackageRelease>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageRelease {
    version: PackageReleaseId,
    files: Vec<File>,
}

impl PackageRelease {
    pub fn new(version: PackageReleaseId, files: Vec<File>) -> Self {
        Self { version, files }
    }

    pub fn files(self) -> Vec<File> {
        self.files
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileRequest {
    package: PackageId,
    release: PackageReleaseId,
    file: FileName,
}

impl FileRequest {
    pub fn new(package: PackageId, release: PackageReleaseId, file: FileName) -> Self {
        Self {
            package,
            release,
            file,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QualifiedFile {
    path: FileRequest,
    length: Option<u64>,
}

impl QualifiedFile {
    pub fn new(path: FileRequest, length: Option<u64>) -> Self {
        Self { path, length }
    }
}

impl From<QualifiedFile> for File {
    fn from(file: QualifiedFile) -> Self {
        File::new(file.path.file, file.length)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QualifiedFiles(pub Vec<QualifiedFile>);

impl From<Vec<QualifiedFile>> for QualifiedFiles {
    fn from(requests: Vec<QualifiedFile>) -> Self {
        Self(requests)
    }
}

impl QualifiedFiles {
    /// Return a list of unique package IDs in this `QualifiedFiles`.
    pub fn packages(&self) -> Vec<PackageId> {
        self.0
            .iter()
            .map(|r| r.path.package.clone())
            .unique()
            .collect()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Action {
    Download {
        user: UserId,
        files: QualifiedFiles,
    },
    RefreshMetadata {
        user: UserId,
    },
    Publish {
        package: PackageId,
        release: PackageRelease,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
    #[serde(with = "simple_dt_8601")]
    timestamp: OffsetDateTime,
    action: Action,
}

impl Entry {
    pub fn new(timestamp: OffsetDateTime, action: Action) -> Self {
        Self { timestamp, action }
    }

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
                if entry.timestamp < last {
                    panic!("Invalid log!");
                }
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
