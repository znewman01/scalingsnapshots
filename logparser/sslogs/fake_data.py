from dataclasses import dataclass
from enum import Enum, auto
from itertools import product
from typing import Dict, List, Any, TypeVar, Tuple

import datetime

from scipy import stats

USERS = range(10)
PACKAGES = range(100)
NUM_DOWNLOADS = 1000


TDayOfWeek = TypeVar("TDayOfWeek", bound="DayOfWeek")


class DayOfWeek(Enum):
    MONDAY = auto()
    TUESDAY = auto()
    WEDNESDAY = auto()
    THURSDAY = auto()
    FRIDAY = auto()
    SATURDAY = auto()
    SUNDAY = auto()

    @classmethod
    def from_date(cls, date: datetime.date) -> TDayOfWeek:
        return cls(date.isoweekday())


HOURS_OF_DAY = range(24)
HOURS_OF_WEEK = list(product(DayOfWeek, HOURS_OF_DAY))

# TODO: none of these distributions are right
DOWNLOADS_BY_PACKAGE = stats.logser(0.1, -1)  # TODO: cap number unique packages
DOWNLOADS_BY_USER = stats.logser(0.1, -1)  # TODO: cap number unique users

DOWNLOADS_BY_HOUR_OF_WEEK = stats.randint(0, len(HOURS_OF_WEEK))

MINUTES_BY_HOUR = stats.randint(0, 60)
SECONDS_BY_MINUTE = stats.randint(0, 60)
NUM_SESSIONS = 10
SESSIONS_BY_HOUR_OF_WEEK = stats.randint(0, len(HOURS_OF_WEEK))
SESSIONS_BY_USER = stats.logser(0.5, -1)
SESSIONS_BY_DOWNLOAD_COUNT = stats.logser(0.6)


@dataclass
class Session:
    start_time: datetime.datetime
    DURATION = datetime.timedelta(minutes=5)  # TODO: maybe try a couple values
    user: int
    dl_count: int


@dataclass
class Download:
    user: int
    package: int
    datetime: datetime.datetime


def sample_simple(start_date: datetime.date) -> Download:
    assert DayOfWeek.from_date(start_date) == DayOfWeek.MONDAY
    start_time = datetime.datetime.combine(start_date, datetime.datetime.min.time())

    user = DOWNLOADS_BY_USER.rvs()
    package = DOWNLOADS_BY_PACKAGE.rvs()
    (day, hour) = HOURS_OF_WEEK[DOWNLOADS_BY_HOUR_OF_WEEK.rvs()]
    minute = MINUTES_BY_HOUR.rvs()
    second = SECONDS_BY_MINUTE.rvs()
    dt = start_time + datetime.timedelta(
        days=day.value - 1, hours=hour, minutes=minute, seconds=second
    )
    return Download(user, package, dt)


def sample_many_simple(start_date: datetime.date) -> List[Download]:
    return [sample_simple(start_date) for _ in range(NUM_DOWNLOADS)]


def sample_session(week_start_time: datetime.datetime) -> Session:
    assert DayOfWeek.from_date(week_start_time.date()) == DayOfWeek.MONDAY

    (start_day, start_hour) = HOURS_OF_WEEK[SESSIONS_BY_HOUR_OF_WEEK.rvs()]
    minute = MINUTES_BY_HOUR.rvs()
    second = SECONDS_BY_MINUTE.rvs()
    dt = week_start_time + datetime.timedelta(
        days=start_day.value - 1, hours=start_hour, minutes=minute, seconds=second
    )
    user = SESSIONS_BY_USER.rvs()
    dl_count = SESSIONS_BY_DOWNLOAD_COUNT.rvs()
    return Session(start_time=dt, user=user, dl_count=dl_count)


def sample_many_session(
    start_date: datetime.date,
) -> Tuple[List[Session], List[Download]]:
    start_time = datetime.datetime.combine(start_date, datetime.datetime.min.time())

    sessions = []
    downloads = []

    for _ in range(NUM_SESSIONS):
        session = sample_session(start_time)
        sessions.append(session)
        for _ in range(session.dl_count):
            package = DOWNLOADS_BY_PACKAGE.rvs()
            seconds = stats.randint(0, Session.DURATION.seconds).rvs()
            dl_time = session.start_time + datetime.timedelta(seconds=seconds)
            download = Download(user=session.user, package=package, datetime=dl_time)
            downloads.append(download)

    return (sessions, downloads)


def main():
    from pprint import pprint
    import operator

    sessions, downloads = sample_many_session(datetime.date(2022, 2, 21))
    sessions.sort(key=operator.attrgetter("start_time"))
    downloads.sort(key=operator.attrgetter("datetime"))
    pprint(sessions)
    pprint(downloads)
    pprint((len(sessions), len(downloads)))


if __name__ == "__main__":
    main()
