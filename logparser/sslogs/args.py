import argparse
import sys
import datetime


def parse_time(t):
    return datetime.datetime.fromisoformat(t).replace(tzinfo=datetime.timezone.utc)


def parse_duration(d):
    quantity, unit = d[:-1], d[-1]
    if unit == "m":
        return datetime.timedelta(minutes=int(quantity))
    elif unit == "h":
        return datetime.timedelta(hours=int(quantity))
    elif unit == "d":
        return datetime.timedelta(days=int(quantity))
    else:
        raise ValueError("Invalid argument {d}. Should be 10m, 1d, 5h.")


def add_common_args(parser: argparse.ArgumentParser):
    parser.add_argument(
        "--output",
        help="The output data file to write to (default STDOUT)",
        type=argparse.FileType("w"),
        default=sys.stdout,
    )
    parser.add_argument(
        "--initial-output",
        help="The output data file for the initial state to write to",
        type=argparse.FileType("w"),
        required=True,
    )


def add_input_arg(parser: argparse.ArgumentParser):
    parser.add_argument(
        "--input",
        help="The input log file to parse (default STDIN)",
        type=argparse.FileType("r"),
        default=sys.stdin,
    )
