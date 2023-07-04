"""Log parser for AUR data.

For upload information, we use data from `packages-meta-v1`, which contains metadata, including las modified time, for all packages.

`packages-meta-v1` is updated every 5 minutes, so for each file we consider packages modified in the last 5 minutes.

# Nginx download logs

TODO (hopefully)
"""
from __future__ import annotations

import datetime
import json

from operator import attrgetter
from typing import Iterable, Any, Dict


from sslogs.logs import LogEntry, Publish, Package


def json_to_log_entry(entry) -> LogEntry:
    mtime = datetime.datetime.fromtimestamp(entry["LastModified"])
    # TODO length is not included in this metadata
    return LogEntry(
        timestamp=mtime,
        action=Publish(package=Package(id=entry["Name"], length=0)),
    )


def process(archive: Iterable[Dict[str, Any]], min_update_time: int):
    # only consider files added since the file was updated
    # curtime is the beginning of the current range
    archive_filtered = (l for l in archive if l["LastModified"] > min_update_time)
    return [json_to_log_entry(j) for j in archive_filtered]


def main():
    archive = open("testdata.json", "r")
    contents = json.load(archive)
    log = process(contents, 1436303556)
    log.sort(key=attrgetter("timestamp"))


if __name__ == "__main__":
    main()
