"""

Keep in sync with $REPO/src/log.rs.
"""
import dataclasses
import datetime

from dataclasses import dataclass
from typing import Union, List, Dict, Any, Optional


@dataclass
class File:
    name: str
    length: Optional[int] = None


@dataclass
class FilePath:
    package: str
    release: str
    file: str


@dataclass
class QualifiedFile:
    path: FilePath
    length: Optional[int] = None


@dataclass
class PackageRelease:
    version: str
    files: List[File]


@dataclass
class Download:
    user: int
    files: List[QualifiedFile]


@dataclass
class RefreshMetadata:
    user: int


@dataclass
class Publish:
    file: str


@dataclass
class LogEntry:
    timestamp: datetime.datetime
    action: Union[Download, RefreshMetadata, Publish]

    def to_dict(self) -> Dict[str, Any]:
        return {
            "action": {type(self.action).__name__: dataclasses.asdict(self.action)},
            "timestamp": self.timestamp.strftime("%Y-%m-%d %H:%M:%S.0 %z"),
        }


def main():
    log = [
        LogEntry(
            timestamp=datetime.datetime(
                1970, 1, 1, 0, 0, 0, tzinfo=datetime.timezone.utc
            ),
            action=Download(
                user=1,
                files=[
                    QualifiedFile(
                        path=FilePath(
                            package="libc", release="0.5", file="libc-0.5.tar.gz"
                        ),
                        length=1000,
                    ),
                    QualifiedFile(
                        FilePath(
                            package="libc", release="0.5", file="libc-0.5-docs.tar.gz"
                        ),
                    ),
                ],
            ),
        ),
        LogEntry(
            timestamp=datetime.datetime(
                1970, 1, 1, 0, 0, 1, tzinfo=datetime.timezone.utc
            ),
            action=RefreshMetadata(user=2),
        ),
        LogEntry(
            timestamp=datetime.datetime(
                1970, 1, 1, 0, 0, 2, tzinfo=datetime.timezone.utc
            ),
            action=Publish(
                file="openssl-1.0.0-sparc.tar.gz",
            ),
        ),
        LogEntry(
            timestamp=datetime.datetime(
                1970, 1, 1, 0, 0, 2, tzinfo=datetime.timezone.utc
            ),
            action=Publish(
                file="openssl-1.0.0-amd64.tar.gz",
            ),
        ),
    ]
    import json

    for log_entry in log:
        print(json.dumps(log_entry.to_dict(), default=datetime.datetime.isoformat))


if __name__ == "__main__":
    main()
