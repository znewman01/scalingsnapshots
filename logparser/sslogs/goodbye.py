"""
Goodbye
"""
import argparse
import json
import copy

from typing import Any

import sslogs.args
import os

def reverse_readline(filename, buf_size=8192):
    """A generator that returns the lines of a file in reverse order"""
    #with open(filename) as fh:
    fh = filename
    segment = None
    offset = 0
    fh.seek(0, os.SEEK_END)
    file_size = remaining_size = fh.tell()
    while remaining_size > 0:
        offset = min(file_size, offset + buf_size)
        fh.seek(file_size - offset)
        buffer = fh.read(min(remaining_size, buf_size))
        remaining_size -= buf_size
        lines = buffer.split('\n')
        # The first line of the buffer is probably not a complete line so
        # we'll save it and append it to the last line of the next buffer
        # we read
        if segment is not None:
            # If the previous chunk starts right from the beginning of line
            # do not concat the segment to the last line of new chunk.
            # Instead, yield the segment first
            if buffer[-1] != '\n':
                lines[-1] += segment
            else:
                yield segment
        segment = lines[0]
        for index in range(len(lines) - 1, 0, -1):
            if lines[index]:
                yield lines[index]
    # Don't yield None if the file was empty
    if segment is not None:
        yield segment



def main(args: argparse.Namespace):
    input: io.TextIOBase = args.input
    output: io.TextIOBase = args.output
    tempfile_string = args.tempfile
    tempfile = open(tempfile_string, "w")
    seen = set()
    for line in reverse_readline(input):
        # parse
        parsed = json.loads(line)
        action = parsed['action']
        # if download and not in seen
        if "Download" in action and action["Download"]["user"] not in seen:
        #   add goodbye event
            goodbye = copy.deepcopy(parsed)
            goodbye["action"] = {"Goodbye": {"user": action["Download"]["user"]}}
            tempfile.write(json.dumps(goodbye) + "\n")
            seen.add(action["Download"]["user"])
        # passthrough line
        tempfile.write(line + "\n")

    #seek to start of tempfile
    tempfile.close()
    tempfile = open(tempfile_string, "r")

    for line in reverse_readline(tempfile):
        output.write(line + "\n")
    tempfile.close()



def add_args(parser: Any):
    subparser: argparse.ArgumentParser = parser.add_parser(
        "goodbye", help=__doc__.split("\n")[0], description=__doc__
    )
    subparser.add_argument(
        "--tempfile",
        help="Temp file location",
        type = str,
        required=True
    )
    subparser.set_defaults(func=main)
    sslogs.args.add_input_arg(subparser)
