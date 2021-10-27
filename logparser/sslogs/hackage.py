"""Log parser for Hackage data.

The Hackage data has two components:

- [01-index.tar]
- Nginx logs (provided by Hackage admins)

[01-index.tar] https://hackage.haskell.org/01-index.tar

# 01-index.tar

01-index.tar is an append-only file representing all Hackage metadata in one directory. For example:

    package-name/preferred-versions
    package-name/0.0.1/package.json
    package-name/0.0.1/package-name.cabal
    ...

In the tarball, these files are in order of their addition to Hackage.

We use a couple heuristics to turn these into "package upload" events:

- we group together adjacent files with the same package/version as one upload
- we use the last "modified time" of all files in the tarball as the time of package publication

## Revisions

Hackage supports a concept of [revisions]: changes to the metadata of an
existing package version. We treat revisions as distinct package upload events
(with the same "version" string). There's an exception: if two revisions were
submitted to Hackage in a row with no other packages in between, we lump them
into the same upload (we could try to detect this by looking for packages that
write the same file twice).

[revisions] https://github.com/haskell-infra/hackage-trustees/blob/4e3a279d1233a18a6b31740cd8314090f3e6091d/revisions-information.md

# Nginx logs

TODO (hopefully)
"""
from __future__ import annotations

import tarfile
import datetime
import itertools

from operator import attrgetter
from typing import List, Iterable
from dataclasses import dataclass

from tqdm import tqdm

from sslogs.logs import FilePath, LogEntry, Publish, File, PackageRelease


@dataclass
class ReleaseId:
    package: str
    release: str

    @classmethod
    def from_tarinfo(cls, info: tarfile.TarInfo) -> ReleaseId:
        package, release, _ = info.name.split("/")
        return ReleaseId(package=package, release=release)


def tar_entries_to_log_entry(
    release_id: ReleaseId, infos: List[tarfile.TarInfo]
) -> LogEntry:
    mtime = datetime.datetime.fromtimestamp(max(info.mtime for info in infos))
    return LogEntry(
        timestamp=mtime,
        action=Publish(
            package=release_id.package,
            release=PackageRelease(
                version=release_id.release,
                files=[File(name=info.name, length=info.size) for info in infos],
            ),
        ),
    )


# def _tar_info_to_triplet(info: tarfile.TarInfo) -> Tuple[Tuple[package,]
def process(archive: Iterable[tarfile.TarInfo]):
    archive_filtered = (
        f for f in archive if f.name.split("/")[-1] != "preferred-versions"
    )
    return [
        tar_entries_to_log_entry(r, list(i))
        for r, i in itertools.groupby(archive_filtered, key=ReleaseId.from_tarinfo)
    ]


def main():
    archive = tarfile.open("/tmp/01-index.tar", "r:")
    log = process(tqdm(archive, total=280000, desc="Processing 01-index.tar"))
    log.sort(key=attrgetter("timestamp"))
    breakpoint()
    print("done")

    # open questions:
    # - are timestamps accurate: i think they're the best we can get
    # - do you ever overwrite the same file: yes!
    #   - HTTP/4000.2.4/HTTP.cabal: 2012-08-31 and 2014-06-08
    #   - similar for template-haskell (many versions)


if __name__ == "__main__":
    main()
