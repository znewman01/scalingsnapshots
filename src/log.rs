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
    fn packages(&self) -> Vec<PackageId> {
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
