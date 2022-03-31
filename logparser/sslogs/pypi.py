"""Python Package Index (PyPI) log parser.

The PyPI data is in Google Cloud BigQuery.

Must set environment variable $GOOGLE_APPLICATION_CREDENTIALS to point to a
.json for a valid Google Cloud Platform service account with read access to
BigQuery.
"""
import argparse
import json

from typing import Any
from google.cloud import bigquery

import sslogs.args
import sslogs.logs

# https://packaging.python.org/guides/analyzing-pypi-package-downloads/
# Table: bigquery-public-data.pypi.file_downloads
# Columns:
# - timestamp, country_code, url, project, tls_protocol, tls_cipher
# - file.*: filename, project, version, type
# - details.*: installer.*, python, implementation.*, distro.*, system.*, details.cpu, details.openssl_version, details.setuptools_version
_QUERY = """
SELECT * FROM bigquery-public-data.pypi.file_downloads
         WHERE timestamp BETWEEN TIMESTAMP("2021-10-18 10:10:00+00") AND TIMESTAMP("2021-10-18 10:20:00+00")
         ORDER BY timestamp
         LIMIT 1000
"""


def main(args: argparse.Namespace):
    output: io.TextIOBase = args.output
    client = bigquery.Client()
    job = client.query(_QUERY)
    for row in job.result():
        user = str(abs(hash(str(row[1]) + str(row[5]) + str(row[6]) + str(row[7]))))
        refresh = sslogs.logs.LogEntry(
            timestamp=row[0], action=sslogs.logs.RefreshMetadata(user=user)
        )
        # TODO: refresh metadata better
        download = sslogs.logs.LogEntry(
            timestamp=row[0],
            action=sslogs.logs.Download(
                user=user, package=sslogs.logs.Package(id=row[3])
            ),
        )
        output.write(json.dumps(refresh.to_dict()) + "\n")
        output.write(json.dumps(download.to_dict()) + "\n")


def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "pypi", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.set_defaults(func=main)
    sslogs.args.add_input_arg(subparser)
