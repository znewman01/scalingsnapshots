import sys

import argparse
import matplotlib.pyplot as plt

from typing import List, Optional
from pathlib import Path


def dump_log(data: str, output_dir: Path):
    (output_dir / "input.log").write_text(data)


def plot(data: str, path: Path):
    plt.title(title)
    plt.plot([1, 2, 2, 4])
    plt.savefig(path / "hello.png")

    plt.title("Goodbye")
    plt.plot([4, 2, 3, 1])
    plt.savefig(path / "goodbye.png")


def main(argv: Optional[List[str]] = None):
    try:
        prog = argv.pop(0)
    except (IndexError, AttributeError):
        prog = "ssanalyze"
    parser = argparse.ArgumentParser(
        prog=prog,
        description="Do analysis for scalingsnapshots.",
    )
    parser.add_argument(
        "--input",
        help="The input data file to analyze (default STDIN)",
        type=argparse.FileType("r"),
        default=sys.stdin,
    )
    parser.add_argument(
        "--non-sensitive-data",
        help="Passing this flag indicates that the data provided as input is not sensitive (and can be written to disk).",
        action="store_true",
    )
    parser.add_argument(
        "--output",
        help="Directory to write analysis output.",
        type=Path,
        required=True,
    )
    args = parser.parse_args(argv or sys.argv[1:])

    data_string = args.input.read()

    if args.non_sensitive_data:
        dump_log(data_string, args.output)

    plot(data_string, args.output)


if __name__ == "__main__":
    main(sys.argv)
