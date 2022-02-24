from dataclasses import dataclass
from enum import Enum, auto
from itertools import product
from typing import Dict, List, Any, TypeVar

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

DOWNLOADS_BY_PACKAGE = stats.logser(0.1, -1)  # TODO: cap number unique packages
DOWNLOADS_BY_USER = stats.logser(0.1, -1)  # TODO: cap number unique users

DOWNLOADS_BY_HOUR_OF_WEEK = stats.randint(0, len(HOURS_OF_WEEK))

MINUTES_BY_HOUR = stats.randint(0, 60)
SECONDS_BY_MINUTE = stats.randint(0, 60)


@dataclass
class SimpleDownload:
    user: int
    package: int
    datetime: datetime.datetime


def sample_simple(start_date: datetime.date) -> SimpleDownload:
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
    return SimpleDownload(user, package, dt)


def sample_many_simple(start_date: datetime.date) -> List[SimpleDownload]:
    return [sample_simple(start_date) for _ in range(NUM_DOWNLOADS)]
