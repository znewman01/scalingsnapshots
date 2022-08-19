import io
import json
import argparse

from collections import Counter
from itertools import islice
from typing import Optional


def get_name(log_entry):
    action = log_entry["action"]
    if "Publish" in action:
        return action["Publish"]["package"]["id"]
    elif "Download" in action:
        return action["Download"]["package"]["id"]
    else:
        assert "Goodbye" in action
        return None


def trim(
    input_initial: io.TextIOBase,
    input_events: io.TextIOBase,
    output_initial: io.TextIOBase,
    output_events: io.TextIOBase,
    num_packages: Optional[int],
    num_entries: Optional[int],
):
    # Find most popular `num_packages` downloads
    counts = Counter(map(get_name, map(json.loads, input_initial)))
    top = set([value for value, _ in counts.most_common(num_packages)])

    # Predicate: entries that are in the top `num_packages` downloads
    in_top = lambda e: get_name(e) in top

    # Write upload events for the top `num_packages` to `output_initial`.
    input_initial.seek(0)
    for entry in filter(in_top, map(json.loads, input_initial)):
        output_initial.write(json.dumps(entry) + "\n")

    # Write up to `num_entries` download events for the top `num_packages` to `output_events`.
    for entry in islice(filter(in_top, map(json.loads, input_events)), num_entries):
        output_events.write(json.dumps(entry) + "\n")


def parse_args():
    parser = argparse.ArgumentParser(__file__)
    parser.add_argument("--input-initial", required=True, type=argparse.FileType("r"))
    parser.add_argument("--input-events", required=True, type=argparse.FileType("r"))
    parser.add_argument("--output-initial", required=True, type=argparse.FileType("w"))
    parser.add_argument("--output-events", required=True, type=argparse.FileType("w"))
    parser.add_argument(
        "--num-packages",
        type=int,
        help="How many distinct packages to keep in the trimmed data.",
    )
    parser.add_argument(
        "--num-entries",
        type=int,
        help="How many upload/download events to keep in the trimmed data (may be limited by `--num-packages`",
    )
    return parser.parse_args()


def main():
    args = parse_args()
    trim(
        args.input_initial,
        args.input_events,
        args.output_initial,
        args.output_events,
        args.num_packages,
        args.num_entries,
    )


if __name__ == "__main__":
    main()
