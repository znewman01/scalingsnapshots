use sssim::log::{Action, File, FileRequest, FilesRequest, PackageRelease, UserId};
use std::io::{self, Write};

fn main() {
    let log = vec![
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
        Action::RefreshMetadata {
            user: UserId::from(2),
        },
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
    ];
    let mut stdout = io::stdout();
    for action in log {
        serde_json::to_writer(&stdout, &action).expect("stdout failed");
        stdout.write_all(b"\n").expect("stdout failed");
    }
}
