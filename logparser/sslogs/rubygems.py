"""RubyGems log parser.

RubyGems data can be downloaded raw from https://ui.honeycomb.io/ruby-together/datasets/rubygems.org/result/HPagwuSXsGZ?tab=raw
If the above link expires, it can be generated from the following parameters to the for     m at https://ui.honeycomb.io/ruby-together/datasets/rubygems.org:
     * WHERE url contains .gem AND url does-not-contain gemspec
     * GROUP BY downloaded_gem_name
     * ensure the query runs against rubygems.org
"""
import argparse

from typing import Any

import sslogs.args


def main(args: argparse.Namespace):
    output: io.TextIOBase = args.output
    file: io.TextIOBase = args.input

    contents = json.load(file)

    for row in contents:
        user = row["client_ip_hash"]
        ts = datetime.datetime.strptime(row["timestamp"], '%Y-%m-%dT%H:%M:%SZ')
        refresh = sslogs.logs.LogEntry(
            timestamp=ts, action=sslogs.logs.RefreshMetadata(user=user)
        )
        # TODO: refresh metadata better
        download = sslogs.logs.LogEntry(
            timestamp=row["timestamp"],
            action=sslogs.logs.Download(
                user=user,
                package=sslogs.logs.Package(
                    id=row["downloaded_gem_name"], length=row["resp_body_size"]
                ),
            ),
        )
        output.write(json.dumps(refresh.to_dict()) + "\n")
        output.write(json.dumps(download.to_dict()) + "\n")


def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "rubygems", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.set_defaults(func=main)
    sslogs.args.add_input_arg(subparser)
