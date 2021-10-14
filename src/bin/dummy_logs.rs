use std::io::{self, Write};

use chrono::prelude::*;

use sssim::log::{Action, File, FileRequest, FilesRequest, Log, LogEntry, PackageRelease, UserId};

fn main() {
    let log = Log::from(vec![
        LogEntry::new(
            Utc.ymd(1970, 1, 1).and_hms(0, 0, 0),
            Action::Download {
                user: UserId::from(1),
                files: FilesRequest::from(vec![
                    FileRequest::new(
                        "libc".to_string().into(),
                        "0.5".to_string().into(),
                        "libc-0.5.tar.gz".to_string().into(),
                    ),
                    FileRequest::new(
                        "libc".to_string().into(),
                        "0.5".to_string().into(),
                        "libc-0.5-docs.tar.gz".to_string().into(),
                    ),
                ]),
            },
        ),
        LogEntry::new(
            Utc.ymd(1970, 1, 1).and_hms(0, 0, 1),
            Action::RefreshMetadata {
                user: UserId::from(2),
            },
        ),
        LogEntry::new(
            Utc.ymd(1970, 1, 1).and_hms(0, 0, 2),
            Action::Publish {
                package: "openssl".to_string().into(),
                release: PackageRelease::new(
                    "1.0.0".to_string().into(),
                    vec![
                        File::new("openssl-1.0.0-sparc.tar.gz".to_string().into(), 1000),
                        File::new("openssl-1.0.0-amd64.tar.gz".to_string().into(), 2000),
                    ],
                ),
            },
        ),
    ]);
    let mut stdout = io::stdout();
    for action in log {
        serde_json::to_writer(&stdout, &action).expect("stdout failed");
        stdout.write_all(b"\n").expect("stdout failed");
    }
}
