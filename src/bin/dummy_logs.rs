use std::io::{self, Write};

use time::macros::datetime;

use sssim::log::{Action, Entry, Log, Package, PackageId, UserId};

fn main() {
    let log = Log::from(vec![
        Entry::new(
            datetime!(1970-01-01 00:00:00).assume_utc(),
            Action::Download {
                user: UserId::from("1".to_string()),
                package: Package {
                    id: "openssl".to_string().into(),
                    length: None,
                },
            },
        ),
        Entry::new(
            datetime!(1970-01-01 00:00:00).assume_utc(),
            Action::Download {
                user: UserId::from("1".to_string()),
                package: Package {
                    id: PackageId::from("libc".to_string()),
                    length: Some(1000),
                },
            },
        ),
        Entry::new(
            datetime!(1970-01-01 00:00:01).assume_utc(),
            Action::RefreshMetadata {
                user: UserId::from("2".to_string()),
            },
        ),
        Entry::new(
            datetime!(1970-01-01 00:00:02).assume_utc(),
            Action::Publish {
                package: Package {
                    id: PackageId::from("openssl".to_string()),
                    length: Some(1000),
                },
            },
        ),
        Entry::new(
            datetime!(1970-01-01 00:00:02).assume_utc(),
            Action::Publish {
                package: Package {
                    id: PackageId::from("libc".to_string()),
                    length: None,
                },
            },
        ),
    ]);
    let mut stdout = io::stdout();
    for action in log {
        serde_json::to_writer(&stdout, &action).expect("stdout failed");
        stdout.write_all(b"\n").expect("stdout failed");
    }
}
