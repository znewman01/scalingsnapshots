"""Python Package Index (PyPI) log parser.

The PyPI data is in Google Cloud BigQuery.

Must set environment variable $GOOGLE_APPLICATION_CREDENTIALS to point to a
.json for a valid Google Cloud Platform service account with read access to
BigQuery.
"""
import argparse
import io
import json
import base64

from typing import Any
from google.cloud import bigquery

import sslogs.args
import sslogs.logs

import datetime

# https://packaging.python.org/guides/analyzing-pypi-package-downloads/
# Table: bigquery-public-data.pypi.file_downloads
# Columns:
# - timestamp, country_code, url, project, tls_protocol, tls_cipher
# - file.*: filename, project, version, type
# - details.*: installer.*, python, implementation.*, distro.*, system.*, details.cpu, details.openssl_version, details.setuptools_version
_QUERY = """
SELECT timestamp, MD5(CONCAT(TO_JSON_STRING(country_code), TO_JSON_STRING(details), tls_protocol, tls_cipher)), project FROM bigquery-public-data.pypi.file_downloads
         WHERE timestamp BETWEEN TIMESTAMP("{start}") AND TIMESTAMP("{end}")
         ORDER BY timestamp
         {limit}
"""

_QUERY_UPLOAD = """
SELECT MIN(upload_time), name FROM bigquery-public-data.pypi.distribution_metadata
         WHERE upload_time BETWEEN TIMESTAMP("{start}") AND TIMESTAMP("{end}")
         GROUP BY name, version
         ORDER BY MIN(upload_time)
         {limit}
"""

_QUERY_INITIAL = """
SELECT name FROM bigquery-public-data.pypi.distribution_metadata
        WHERE upload_time < TIMESTAMP("{start}")
        GROUP BY name
"""


def _query_download(start, duration, limit):
    if limit:
        return _QUERY.format(
            start=start.isoformat(),
            end=(start + duration).isoformat(),
            limit=f"LIMIT {limit}",
        )
    else:
        return _QUERY.format(
            start=start.isoformat(), end=(start + duration).isoformat(), limit=""
        )


def _query_upload(start, duration, limit):
    if limit:
        return _QUERY_UPLOAD.format(
            start=start.isoformat(),
            end=(start + duration).isoformat(),
            limit=f"LIMIT {limit}",
        )
    else:
        return _QUERY_UPLOAD.format(
            start=start.isoformat(), end=(start + duration).isoformat(), limit=""
        )


def _query_initial(start):
    return _QUERY_INITIAL.format(start=start.isoformat())


def main(args: argparse.Namespace):
    initial_output: io.TextIOBase = args.initial_output
    output: io.TextIOBase = args.output
    client = bigquery.Client()

    # initial output
    initial_job = client.query(_query_initial(args.start))
    for row in initial_job:
        row[0]
        entry = sslogs.logs.LogEntry(
            timestamp=args.start,
            action=sslogs.logs.Publish(package=sslogs.logs.Package(row[0])),
        )
        initial_output.write(json.dumps(entry.to_dict()) + "\n")

    download_job = iter(
        client.query(_query_download(args.start, args.duration, args.limit))
    )
    upload_job = iter(
        client.query(_query_upload(args.start, args.duration, args.limit))
    )
    next_upload = next(upload_job)
    next_download = next(download_job)

    # merge-sort ish to do in timestamp order
    while True:
        if next_download is None and next_upload is None:
            break
        if next_upload is None or (next_download and next_download[0] < next_upload[0]):
            user = base64.b64encode(next_download[1]).decode("ascii")
            entry = sslogs.logs.LogEntry(
                timestamp=next_download[0],
                action=sslogs.logs.Download(
                    user=user, package=sslogs.logs.Package(id=next_download[2])
                ),
            )
            try:
                next_download = next(download_job)
            except StopIteration:
                next_download = None
        else:
            entry = sslogs.logs.LogEntry(
                timestamp=next_upload[0],
                action=sslogs.logs.Publish(
                    package=sslogs.logs.Package(id=next_upload[1])
                ),
            )
            try:
                next_upload = next(upload_job)
            except StopIteration:
                next_upload = None
        output.write(json.dumps(entry.to_dict()) + "\n")


def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "pypi", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.add_argument(
        "--start",
        help="Start of the query period",
        type=sslogs.args.parse_time,
        default=datetime.datetime(2021, 10, 18, 10, 10, tzinfo=datetime.timezone.utc),
    )
    subparser.add_argument(
        "--duration",
        help="Duration of the query period (eg 10d)",
        type=sslogs.args.parse_duration,
        default=datetime.timedelta(minutes=10),
    )
    subparser.add_argument(
        "--limit", help="SQL limit (pass 0 for no limit)", type=int, default=10
    )

    subparser.set_defaults(func=main)
    sslogs.args.add_input_arg(subparser)
