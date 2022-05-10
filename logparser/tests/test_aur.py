import json
import sslogs.aur as aur

from pathlib import Path

# TODO: rename uploads->packages-meta-v1.json
_INPUT_PATH = Path(__file__).parent / "data" / "aur-uploads.json"


def test_foo():
    cur_time = 1436303556
    with _INPUT_PATH.open() as f:
        contents = json.load(f)
        log = aur.process(contents, cur_time)
    for entry in log:
        assert entry.timestamp > 0
