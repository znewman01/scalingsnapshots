import tarfile
import datetime
import sslogs.hackage
from sslogs.logs import FilePath, LogEntry, Publish, File, PackageRelease


def test_process_empty():
    archive = []
    log = sslogs.hackage.process(archive)
    assert log == []


def test_process():
    archive = [
        # release 1
        tarfile.TarInfo(name="foo/0.0.1/foo.tar.gz"),
        tarfile.TarInfo(name="foo/0.0.1/foo.cabal"),
        tarfile.TarInfo(name="foo/0.0.1/package.json"),
        # release 2
        tarfile.TarInfo(name="bar/0.0.2/package.json"),
        # release 3
        tarfile.TarInfo(name="baz/0.0.3/package.json"),
        # release 4 (NOT release 2!)
        tarfile.TarInfo(name="bar/0.0.2/bar.cabal"),
    ]
    times = [
        datetime.datetime(2021, 1, 1, 0, 0, 1),
        datetime.datetime(2021, 1, 1, 0, 0, 3),  # biggest timestamp in the middle
        datetime.datetime(2021, 1, 1, 0, 0, 2),
        datetime.datetime(2021, 1, 1, 0, 0, 4),
        datetime.datetime(2021, 1, 1, 0, 0, 6),  # out of order packages
        datetime.datetime(2021, 1, 1, 0, 0, 5),
    ]
    for idx, (info, time) in enumerate(zip(archive, times)):
        info.mtime = time.timestamp()
        info.size = idx * 1000
    actual = sslogs.hackage.process(archive)
    expected = [
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 3),
            action=Publish(
                package="foo",
                release=PackageRelease(
                    version="0.0.1",
                    files=[
                        File(name="foo/0.0.1/foo.tar.gz", length=0),
                        File(name="foo/0.0.1/foo.cabal", length=1000),
                        File(name="foo/0.0.1/package.json", length=2000),
                    ],
                ),
            ),
        ),
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 4),
            action=Publish(
                package="bar",
                release=PackageRelease(
                    version="0.0.2",
                    files=[File(name="bar/0.0.2/package.json", length=3000)],
                ),
            ),
        ),
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 6),
            action=Publish(
                package="baz",
                release=PackageRelease(
                    version="0.0.3",
                    files=[File(name="baz/0.0.3/package.json", length=4000)],
                ),
            ),
        ),
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 5),
            action=Publish(
                package="bar",
                release=PackageRelease(
                    version="0.0.2",
                    files=[File(name="bar/0.0.2/bar.cabal", length=5000)],
                ),
            ),
        ),
    ]
    assert actual == expected
