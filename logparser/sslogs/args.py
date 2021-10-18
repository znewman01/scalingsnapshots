import argparse
import sys


def add_common_args(parser: argparse.ArgumentParser):
    parser.add_argument(
        "--output",
        help="The output data file to write to (default STDOUT)",
        type=argparse.FileType("w"),
        default=sys.stdout,
    )


def add_input_arg(parser: argparse.ArgumentParser):
    parser.add_argument(
        "--input",
        help="The input log file to parse (default STDIN)",
        type=argparse.FileType("r"),
        default=sys.stdin,
    )
