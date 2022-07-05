import sys
import io

import argparse

from typing import List, Optional, Iterator, Callable, Iterable

import sslogs.identity
import sslogs.args
import sslogs.pypi
import sslogs.goodbye


def main(argv: Optional[List[str]] = None):
    try:
        prog = argv.pop(0)
    except (IndexError, AttributeError):
        prog = "logparser"
    parser = argparse.ArgumentParser(
        prog=prog,
        description="Do log parsing for scalingsnapshots.",
    )

    sslogs.args.add_common_args(parser)

    subparsers = parser.add_subparsers(
        title="subcommands",
        description="Different subcommands for different log sources",
    )
    for log_source in [sslogs.identity, sslogs.pypi, sslogs.goodbye]:
        log_source.add_args(subparsers)

    args = parser.parse_args(argv or sys.argv[1:])
    args.func(args)


if __name__ == "__main__":
    main(sys.argv)
