"""Python Package Index (PyPI) log parser.

The PyPI data is in Google Cloud BigQuery.

Must set environment variable $GOOGLE_APPLICATION_CREDENTIALS to point to a
.json for a valid Google Cloud Platform service account with read access to
BigQuery.
"""
import argparse

from typing import Any

import sslogs.args


def main(args: argparse.Namespace):
    output: io.TextIOBase = args.output
    output.write("foo")


def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "pypi", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.set_defaults(func=main)
    sslogs.args.add_input_arg(subparser)
