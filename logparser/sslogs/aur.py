"""Log parser for AUR data.

For upload information, we use data from `packages-meta-v1`, which contains metadata, including las modified time, for all packages.

`packages-meta-v1` is updated every 5 minutes, so for each file we consider packages modified in the last 5 minutes.

# Nginx download logs

TODO (hopefully)
"""
from __future__ import annotations

import gzip
import datetime
import itertools

from operator import attrgetter
from typing import Iterable

from sslogs.logs import FilePath, LogEntry, Publish, File


def json_to_log_entry(entry) -> LogEntry:
    mtime = datetime.datetime.fromtimestamp(entry["LastModified"])
    # TODO length is not included in this metadata
    return LogEntry(
        timestamp=mtime,
        action=Publish(
            package=entry["Name"],
            release=PackageRelease(
                version=entry["Version"],
                files=[File(name=entry["Name"], length=0)],
            ),
        ),
    )


# def _tar_info_to_triplet(info: tarfile.TarInfo) -> Tuple[Tuple[package,]
def process(archive: Iterable[file], curtime: int):
    archive_filtered = (
        json.load(f) for f in archive if json.load(f)["LastModified"] > curtime
    )
    return [
        json_to_log_entry(j)
        for j in archive_filtered
    ]


def main():
    archive = gzip.open("/tmp/packages-meta-v1.json", "r:")
    log = process(archive, 1436303556)
    log.sort(key=attrgetter("timestamp"))
    breakpoint()
    print("done")


if __name__ == "__main__":
    main()
