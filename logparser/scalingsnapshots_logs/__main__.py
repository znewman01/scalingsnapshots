import sys
import io

import argparse

from typing import List, Optional, Iterator, Callable, Iterable


def _identity(input: io.TextIOBase, output: io.TextIOBase):
    for line in input.readlines():
        output.write(line)


_LOG_PARSERS = {
    # No-op: just emit the logs line-by-line as you get them in.
    "identity": _identity,
}


def main(argv: Optional[List[str]] = None):
    try:
        prog = argv.pop(0)
    except (IndexError, AttributeError):
        prog = "logparser"
    parser = argparse.ArgumentParser(
        prog=prog,
        description="Do log parsing for scalingsnapshots.",
    )
    # TODO: soon this will handle multiple types of log file.
    #
    # Multiple entry points?
    parser.add_argument(
        "--input",
        help="The input log file to parse (default STDIN)",
        type=argparse.FileType("r"),
        default=sys.stdin,
    )
    parser.add_argument(
        "--log-type",
        help="The type of log file.",
        choices=_LOG_PARSERS.keys(),
    )
    parser.add_argument(
        "--output",
        help="The output data file to analyze (default STDOUT)",
        type=argparse.FileType("w"),
        default=sys.stdout,
    )
    args = parser.parse_args(argv or sys.argv[1:])

    log_parser = _LOG_PARSERS[args.log_type]
    log_parser(args.input, args.output)


if __name__ == "__main__":
    main(sys.argv)
