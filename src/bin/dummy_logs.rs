use std::io::{self, Write};

use time::macros::datetime;

use sssim::log::{Action, Entry, FileRequest, Log, QualifiedFile, QualifiedFiles, UserId};

fn main() {
    let log = Log::from(vec![
        Entry::new(
            datetime!(1970-01-01 00:00:00).assume_utc(),
            Action::Download {
                user: UserId::from(1),
                files: QualifiedFiles::from(vec![
                    QualifiedFile::new(
                        FileRequest::new(
                            "libc".to_string().into(),
                            "0.5".to_string().into(),
                            "libc-0.5.tar.gz".to_string().into(),
                        ),
                        Some(1000),
                    ),
                    QualifiedFile::new(
                        FileRequest::new(
                            "libc".to_string().into(),
                            "0.5".to_string().into(),
                            "libc-0.5-docs.tar.gz".to_string().into(),
                        ),
                        None,
                    ),
                ]),
            },
        ),
        Entry::new(
            datetime!(1970-01-01 00:00:01).assume_utc(),
            Action::RefreshMetadata {
                user: UserId::from(2),
            },
        ),
        Entry::new(
            datetime!(1970-01-01 00:00:02).assume_utc(),
            Action::Publish {
                file: "openssl-1.0.0-sparc.tar.gz".to_string().into(),
            },
        ),
        Entry::new(
            datetime!(1970-01-01 00:00:02).assume_utc(),
            Action::Publish {
                file: "openssl-1.0.0-amd64.tar.gz".to_string().into(),
            },
        ),
    ]);
    let mut stdout = io::stdout();
    for action in log {
        serde_json::to_writer(&stdout, &action).expect("stdout failed");
        stdout.write_all(b"\n").expect("stdout failed");
    }
}
