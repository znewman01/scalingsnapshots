#!/bin/sh
set -ex -o pipefail

RESULT_DIR="$(nix build --no-link --json .#gce | jq -r '.[0].outputs.out')"
IMG_FILE=$(find -H $RESULT_DIR -type f)
jq -n --arg imgfile "$IMG_FILE" '{"path": $imgfile}'
