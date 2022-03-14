import sys
import datetime
from dataclasses import dataclass, field
from collections import Counter
from typing import Optional, Set
from pathlib import Path

INPUT_FILENAME = Path(__file__).parent.parent / "testdata"
SESSION_DURATION = datetime.timedelta(minutes=1)


@dataclass
class PackageData:

    dls_by_package: Counter[str] = field(default_factory=Counter)
    dls_by_user: Counter[str] = field(default_factory=Counter)
    dls_by_hour: Counter[tuple[int, int]] = field(default_factory=Counter)

    sessions_by_user: Counter[str] = field(default_factory=Counter)
    sessions_by_hour: Counter[tuple[int, int]] = field(default_factory=Counter)
    sessions_by_count: Counter[int] = field(default_factory=Counter)

    users_in_cur_session: Set[str] = field(default_factory=set)
    cur_session_start: Optional[datetime.datetime] = None
    downloads_in_cur_session: int = 0

    sessions_count: int = 0
    dls_count: int = 0

    start_time: Optional[datetime.datetime] = None
    end_time: Optional[datetime.datetime] = None

    _TIMESTAMP_FMT = "%Y-%m-%dT%H:%M:%S.%f"

    def _handle_download(self, userid, packageid, timestamp):
        if not self.start_time:
            self.start_time = timestamp

        self.dls_by_package[packageid] += 1
        self.dls_by_user[userid] += 1
        hour_of_week = (timestamp.isoweekday(), timestamp.hour)
        self.dls_by_hour[hour_of_week] += 1
        self.dls_count += 1

        if (
            self.cur_session_start is None
            or self.cur_session_start + SESSION_DURATION < timestamp
        ):
            # if there was a previous session, add to sessions_by_count
            if self.cur_session_start is not None:
                self.sessions_by_count[self.downloads_in_cur_session] += 1
            self.cur_session_start = timestamp
            self.sessions_count += 1
            self.sessions_by_hour[hour_of_week] += 1
            self.users_in_cur_session = set()
            self.downloads_in_cur_session = 0

        self.downloads_in_cur_session += 1

        if userid not in self.users_in_cur_session:
            self.users_in_cur_session.add(userid)
            self.sessions_by_user[userid] += 1

    def handle_downloads(self, lines):
        timestamp = None
        for l in lines:
            (user_id, package_id, timestamp_str) = l.split(",")
            timestamp = datetime.datetime.strptime(
                timestamp_str.strip(), self._TIMESTAMP_FMT
            )
            self._handle_download(user_id, package_id, timestamp)
        if timestamp is not None:
            self.end_time = timestamp
        # add the last session count
        self.sessions_by_count[self.downloads_in_cur_session] += 1


def main():
    data = PackageData()
    try:
        fname = sys.argv[1]
    except IndexError:
        fname = INPUT_FILENAME
    with open(fname) as f:
        data.handle_downloads(f)

    print("Downloads by user:", data.dls_by_user)
    print("Sessions by user:", data.sessions_by_user)

    print("Downloads by package:", data.dls_by_package)
    print("Downloads by hour:", data.dls_by_hour)
    print("Total downloads:", data.dls_count)

    print("Sessions by count:", data.sessions_by_count)
    print("Total sessions:", data.sessions_count)
    print("Sessions by hour:", data.sessions_by_hour)


if __name__ == "__main__":
    main()
