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
import json

from operator import attrgetter
from typing import Iterable

from sslogs.logs import FilePath, LogEntry, Publish, File, PackageRelease


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
    # only consider files added since the file was updated
    # curtime is the beginning of the current range
    # archive_filtered = [f for f in map(json.load, archive) if f["LastModified"] > curtime]
    archive_filtered = (
        l for l in archive if l["LastModified"] > curtime
    )
    return [
        json_to_log_entry(j)
        for j in archive_filtered
    ]


def main():
    archive = open("testdata.json", "r")
    contents = json.load(archive)
    log = process(contents, 1436303556)
    log.sort(key=attrgetter("timestamp"))

if __name__ == "__main__":
    main()
