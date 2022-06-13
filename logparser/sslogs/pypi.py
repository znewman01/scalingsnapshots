"""Python Package Index (PyPI) log parser.

The PyPI data is in Google Cloud BigQuery.

Must set environment variable $GOOGLE_APPLICATION_CREDENTIALS to point to a
.json for a valid Google Cloud Platform service account with read access to
BigQuery.
"""
import argparse
import io

from typing import Any
from google.cloud import bigquery

import sslogs.args

# https://packaging.python.org/guides/analyzing-pypi-package-downloads/
# Table: bigquery-public-data.pypi.file_downloads
# Columns:
# - timestamp, country_code, url, project, tls_protocol, tls_cipher
# - file.*: filename, project, version, type
# - details.*: installer.*, python, implementation.*, distro.*, system.*, details.cpu, details.openssl_version, details.setuptools_version
_QUERY = "SELECT 1"


def main(args: argparse.Namespace):
    output: io.TextIOBase = args.output
    client = bigquery.Client()
    job = client.query(_QUERY)
    for row in job.result():
        output.write(f"{row}\n")


def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "pypi", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.set_defaults(func=main)
    sslogs.args.add_input_arg(subparser)
