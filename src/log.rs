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
use chrono::prelude::*;
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;

// Primitives

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct UserId(u64);

impl From<u64> for UserId {
    fn from(id: u64) -> Self {
        UserId(id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct PackageId(String);

impl From<String> for PackageId {
    fn from(id: String) -> Self {
        PackageId(id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct FileName(String);

impl From<String> for FileName {
    fn from(id: String) -> Self {
        FileName(id)
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

#[derive(Serialize, Deserialize, Debug)]
pub struct File {
    name: FileName,
    length: u64,
}

impl File {
    pub fn new(name: FileName, length: u64) -> Self {
        Self { name, length }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PackageRelease {
    version: PackageReleaseId,
    files: Vec<File>,
}

impl PackageRelease {
    pub fn new(version: PackageReleaseId, files: Vec<File>) -> Self {
        Self { version, files }
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

#[derive(Serialize, Deserialize, Debug)]
pub struct FilesRequest(Vec<FileRequest>);

impl From<Vec<FileRequest>> for FilesRequest {
    fn from(requests: Vec<FileRequest>) -> Self {
        Self(requests)
    }
}

impl FilesRequest {
    /// Return a list of unique package IDs in this FilesRequest.
    pub fn packages(&self) -> Vec<PackageId> {
        self.0.iter().map(|r| r.package.clone()).unique().collect()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Action {
    Download {
        user: UserId,
        files: FilesRequest,
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
    timestamp: DateTime<Utc>,
    action: Action,
}

impl Entry {
    pub fn new(timestamp: DateTime<Utc>, action: Action) -> Self {
        Self { timestamp, action }
    }
}

#[derive(Debug)]
pub struct Log(Vec<Entry>);

impl From<Vec<Entry>> for Log {
    fn from(entries: Vec<Entry>) -> Self {
        // Log entries must be in sorted order by timestamp.
        let mut last: Option<DateTime<Utc>> = None;
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
