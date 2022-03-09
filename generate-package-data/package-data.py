import datetime
from dataclasses import dataclass
from collections import Counter
from typing import Dict, Set

INPUT_FILENAME = "testdata"
SESSION_DURATION = datetime.timedelta(minutes=1)

@dataclass
class PackageData:

    dls_by_package: Counter[str, int]
    dls_by_user: Counter[str, int]
    dls_by_hour: Counter[tuple[int, int], int]

    sessions_by_user: Counter[str, int]
    sessions_by_hour: Counter[tuple[int, int], int]
    sessions_by_count: Counter[int, int]

    users_in_cur_session: Set[str]
    cur_session_start: datetime.datetime = None
    downloads_in_cur_session : int = 0

    sessions_count: int = 0
    dls_count: int = 0

    start_time: datetime.datetime = None
    end_time: datetime.datetime = None

    def __init__(self):
        self.dls_by_package = Counter()
        self.dls_by_user = Counter()
        self.dls_by_hour = Counter()

        self.sessions_by_user = Counter()
        self.sessions_by_hour = Counter()
        self.sessions_by_count = Counter()

        self.users_in_cur_session = set()

    def handle_download(self, userid, packageid, timestamp):
        if not self.start_time:
            self.start_time = timestamp
        # can do this more efficiently by setting it once at the end
        self.end_time = timestamp

        self.dls_by_package[packageid] += 1
        self.dls_by_user[userid] += 1
        hour_of_week = (timestamp.isoweekday(), timestamp.hour)
        self.dls_by_hour[hour_of_week] += 1
        self.dls_count += 1

        if self.cur_session_start is None or self.cur_session_start + SESSION_DURATION < timestamp:
            # if there was a previous session, add to sessions_by_count
            if self.cur_session_start is not None:
                self.sessions_by_count[self.downloads_in_cur_session] += 1
            self.cur_session_start = timestamp
            self.sessions_count += 1
            self.sessions_by_hour[hour_of_week] += 1
            self.users_in_cur_session = []
            self.downloads_in_cur_session = 0

        self.downloads_in_cur_session += 1

        if userid not in self.users_in_cur_session:
            self.users_in_cur_session.append(userid)
            self.sessions_by_user[userid] += 1


    def finish_downloads(self):
        # add the last session count
        self.sessions_by_count[self.downloads_in_cur_session] += 1

    def handle_downloads(self, lines):
        for l in lines:
            words = l.split(',')
            self.handle_download(words[0], words[1], datetime.datetime.strptime(words[2].strip(), "%Y-%m-%dT%H:%M:%S.%f"))
        self.finish_downloads()



if __name__ == '__main__':
    data = PackageData()
    f = open(INPUT_FILENAME, 'r')
    data.handle_downloads(f)

    print(data.dls_by_user)
    print(data.sessions_by_user)

    print(data.dls_by_package)
    print(data.dls_by_hour)
    print(data.dls_count)

    print(data.sessions_by_count)
    print(data.sessions_count)
    print(data.sessions_by_hour)

