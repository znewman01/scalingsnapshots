"""No-op, dummy log parser.

It's a much slower `cat`.

This is just here so we can test the data pipeline end-to-end with our fake data.
"""
import argparse

from typing import Any

import sslogs.args


def main(args: argparse.Namespace):
    input: io.TextIOBase = args.input
    output: io.TextIOBase = args.output
    for line in input.readlines():
        output.write(line)


def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "identity", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.set_defaults(func=main)
    sslogs.args.add_input_arg(subparser)
