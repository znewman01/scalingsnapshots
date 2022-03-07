import datetime

INPUT_FILENAME = "testdata"

class Package_data:
    start_time = None
    end_time = None

    dls_by_package = {}
    dls_by_user = {}
    dls_by_hour = {}
    dls_count = 0

    sessions_by_user = {}
    sessions_by_hour = {}
    sessions_by_count = {}
    sessions_count = 0

    cur_session_start = None
    users_in_cur_session = []
    downloads_in_cur_session = 0

    def parse_line(self, userid, packageid, timestamp):
        if not self.start_time:
            self.start_time = timestamp
        # can do this more efficiently by setting it once at the end
        self.end_time = timestamp

        self.dls_by_package[packageid] = self.dls_by_package.get(packageid, 0) + 1
        self.dls_by_user[userid] = self.dls_by_user.get(userid, 0) + 1
        hour = (timestamp.isoweekday(), timestamp.hour)
        self.dls_by_hour[hour] = self.dls_by_hour.get(hour, 0) + 1
        self.dls_count = self.dls_count + 1

        if self.cur_session_start is None or self.cur_session_start + datetime.timedelta(minutes=1) < timestamp:
            # if there was a previous session, add to sessions_by_count
            if self.cur_session_start is not None:
                self.sessions_by_count[self.downloads_in_cur_session] = self.sessions_by_count.get(self.downloads_in_cur_session, 0) + 1
            self.cur_session_start = timestamp
            self.sessions_count = self.sessions_count + 1
            self.sessions_by_hour[hour] = self.sessions_by_hour.get(hour, 0) + 1
            self.users_in_cur_session = []
            self.downloads_in_cur_session = 0

        self.downloads_in_cur_session = self.downloads_in_cur_session + 1

        if userid not in self.users_in_cur_session:
            self.users_in_cur_session.append(userid)
            self.sessions_by_user[userid] = self.sessions_by_user.get(userid, 0) + 1


    def finish_parsing(self):
        # add the last session count
        self.sessions_by_count[self.downloads_in_cur_session] = self.sessions_by_count.get(self.downloads_in_cur_session, 0) + 1



if __name__ == '__main__':
    data = Package_data()
    f = open(INPUT_FILENAME, 'r')
    # TODO: read a stream
    lines = f.readlines()
    for l in lines:
        words = l.split(',')
        data.parse_line(words[0], words[1], datetime.datetime.strptime(words[2].strip(), "%Y-%m-%dT%H:%M:%S.%f"))
    data.finish_parsing()

    print(data.dls_by_user)
    print(data.sessions_by_user)

    print(data.dls_by_package)
    print(data.dls_by_hour)
    print(data.dls_count)

    print(data.sessions_by_count)
    print(data.sessions_count)
    print(data.sessions_by_hour)

