import tarfile
import datetime
import sslogs.hackage
from sslogs.logs import Package, LogEntry, Publish


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
        tarfile.TarInfo(name="bar/0.0.2/bar.tar.gz"),
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
        datetime.datetime(2021, 1, 1, 0, 0, 5),
        datetime.datetime(2021, 1, 1, 0, 0, 7),  # out of order packages
        datetime.datetime(2021, 1, 1, 0, 0, 6),
    ]
    for idx, (info, time) in enumerate(zip(archive, times)):
        info.mtime = time.timestamp()
        # This trick lets us uniquely identify the sum of any subset of files.
        info.size = 1 << idx
    actual = sslogs.hackage.process(archive)
    expected = [
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 3),
            action=Publish(Package("foo", 3)),
        ),
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 5),
            action=Publish(Package("bar", 16)),
        ),
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 7),
            action=Publish(Package("baz", 0)),
        ),
        LogEntry(
            timestamp=datetime.datetime(2021, 1, 1, 0, 0, 6),
            action=Publish(Package("bar", 64)),
        ),
    ]
    assert actual == expected
