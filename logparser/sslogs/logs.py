"""

Keep in sync with $REPO/src/log.rs.
"""
import dataclasses
import datetime

from dataclasses import dataclass
from typing import Union, List, Dict, Any, Optional


@dataclass
class Package:
    id: str
    length: Optional[int] = None


@dataclass
class Download:
    user: int
    package: Package


@dataclass
class RefreshMetadata:
    user: int


@dataclass
class Publish:
    package: Package


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
            action=Download(user=1, package=Package(id="openssl", length=None)),
        ),
        LogEntry(
            timestamp=datetime.datetime(
                1970, 1, 1, 0, 0, 0, tzinfo=datetime.timezone.utc
            ),
            action=Download(user=1, package=Package(id="libc", length=1000)),
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
            action=Publish(package=Package(id="openssl", length=1000)),
        ),
        LogEntry(
            timestamp=datetime.datetime(
                1970, 1, 1, 0, 0, 2, tzinfo=datetime.timezone.utc
            ),
            action=Publish(package=Package(id="libc", length=None)),
        ),
    ]
    import json

    for log_entry in log:
        print(json.dumps(log_entry.to_dict(), default=datetime.datetime.isoformat))


if __name__ == "__main__":
    main()
