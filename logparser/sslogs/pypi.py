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
import sys
import itertools
import tempfile

from typing import Any, Optional, Iterator
from pathlib import Path

import tqdm

from google.cloud import bigquery

import sslogs.args
import sslogs.logs
import sslogs.goodbye as goodbye

import datetime

_GCP_PROJECT = "zjn-scaling-tuf"

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


def _canonicalize_package(package: str) -> str:
    return package.replace("_", "-").replace(".", "-").lower()


def log(msg: str):
    sys.stderr.write(f"{msg}\n")
    sys.stderr.flush()


def _run_initial_query(
    client: bigquery.Client, start: datetime.datetime
) -> Iterator[sslogs.logs.LogEntry]:
    # initial output
    log("Issuing initial query.")
    initial_job = client.query(_query_initial(start)).result()
    log("Initial query complete.")
    seen = set()
    for row in tqdm.tqdm(initial_job, total=initial_job.total_rows):
        package = row[0]
        if package is not None:
            package = _canonicalize_package(package)
            if package in seen:
                continue
            seen.add(package)
            yield sslogs.logs.LogEntry(
                timestamp=start,
                action=sslogs.logs.Publish(package=sslogs.logs.Package(package)),
            )


def _run_up_down_query(
    client: bigquery.Client,
    start: datetime.datetime,
    duration: datetime.timedelta,
    limit: Optional[int],
) -> Iterator[sslogs.logs.LogEntry]:
    log("Issuing download query.")
    download_result = client.query(_query_download(start, duration, limit)).result()
    log("Download query complete.")
    download_iter = iter(download_result)

    log("Issuing upload query.")
    upload_result = client.query(_query_upload(start, duration, limit)).result()
    upload_iter = iter(upload_result)
    log("Upload query complete.")

    next_upload = next(upload_iter)
    next_download = next(download_iter)
    # merge-sort ish to do in timestamp order
    total_rows = download_result.total_rows + upload_result.total_rows
    log(f"total rows {total_rows}")
    with tqdm.tqdm(total=total_rows) as pbar:
        while True:
            pbar.update(1)
            entry = None
            if next_download is None and next_upload is None:
                break
            if next_upload is None or (
                next_download and next_download[0] < next_upload[0]
            ):
                user = base64.b64encode(next_download[1]).decode("ascii")
                package = next_download[2]
                if package is not None:
                    package = _canonicalize_package(package)
                    entry = sslogs.logs.LogEntry(
                        timestamp=next_download[0],
                        action=sslogs.logs.Download(
                            user=user, package=sslogs.logs.Package(id=package)
                        ),
                    )
                try:
                    next_download = next(download_iter)
                except StopIteration:
                    next_download = None
            else:
                package = next_upload[1]
                if package is not None:
                    package = _canonicalize_package(package)
                    entry = sslogs.logs.LogEntry(
                        timestamp=next_upload[0],
                        action=sslogs.logs.Publish(
                            package=sslogs.logs.Package(id=next_upload[1])
                        ),
                    )
                try:
                    next_upload = next(upload_iter)
                except StopIteration:
                    next_upload = None
            if entry is not None:
                yield entry


def write_logs(
    client: bigquery.Client,
    initial_output: io.TextIOBase,
    output: io.TextIOBase,
    tmpdir: Path,
    start: datetime.datetime,
    duration: datetime.timedelta,
    limit: Optional[int],
):
    log("Issuing initial state query.")
    for entry in _run_initial_query(client, start):
        initial_output.write(json.dumps(entry.to_dict()) + "\n")
    log("Initial state written.")

    log("Issuing upload/download queries.")
    unprocessed = tmpdir / "unprocessed.jsonl"
    with unprocessed.open("w") as tmpfile:
        for entry in _run_up_down_query(client, start, duration, limit):
            tmpfile.write(json.dumps(entry.to_dict()) + "\n")

    log("Post-processing.")
    with unprocessed.open("r") as tmpfile:
        goodbye.goodbyeify(tmpfile, output, tmpdir / "goodbye-tmp.json")


def main(args: argparse.Namespace):
    log("BigQuery client thinking really hard about auth, not doing anything yet.")
    client = bigquery.Client(project=_GCP_PROJECT)
    with tempfile.TemporaryDirectory() as tmpdir:
        write_logs(
            client,
            args.initial_output,
            args.output,
            Path(tmpdir),
            args.start,
            args.duration,
            args.limit,
        )


def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "pypi", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.add_argument(
        "--start",
        help="Start of the query period",
        type=sslogs.args.parse_time,
        default=datetime.datetime(2022, 8, 1, 0, 0, tzinfo=datetime.timezone.utc),
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
