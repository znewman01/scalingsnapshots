import sys

import argparse

from typing import List, Optional


def main(argv: Optional[List[str]] = None):
    try:
        prog = argv.pop(0)
    except (IndexError, AttributeError):
        prog = "logparser"
    parser = argparse.ArgumentParser(
        prog=prog,
        description="Do analysis for scalingsnapshots.",
    )
    args = parser.parse_args(argv or sys.argv[1:])

    print("Hello, world.")


if __name__ == "__main__":
    main(sys.argv)
