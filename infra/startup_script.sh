#!/usr/bin/env bash
set -xeufo pipefail

BUCKET=zjn-scaling-tuf-data

main() {
  TMPDIR=$(mktemp -d)
  cd $TMPDIR

  # Install prerequisites.
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  HOME="/root" source "/root/.cargo/env"
  sudo apt-get install -y m4 make gcc
  rustup toolchain install nightly-2022-03-08

  # Fetch the data.
  DATA_URL=$(curl "http://metadata.google.internal/computeMetadata/v1/instance/attributes/data-url" -H "Metadata-Flavor: Google")
  gsutil cp $DATA_URL ./data.tar.gz
  tar xzf data.tar.gz
  # Assume: data/init.jsonl, data/events.jsonl

  # Fetch and build the source for the simulator.
  gsutil cp "gs://$BUCKET/source.tar.gz" ./source.tar.gz
  tar xzf source.tar.gz
  cd scaling-tuf/sssim
  cargo build --release

  # Run the simulator.
  AUTHENTICATOR=$(curl "http://metadata.google.internal/computeMetadata/v1/instance/attributes/authenticator" -H "Metadata-Flavor: Google")
  mkdir -p "$TMPDIR/output"
  cargo run --release --bin sssim -- \
      --init-path "$TMPDIR/data/init.jsonl" \
      --events-path "$TMPDIR/data/events.jsonl" \
      --authenticator-config-path <(echo "$AUTHENTICATOR") \
      --output-directory "$TMPDIR/output"

  # Upload the results!
  cd $TMPDIR
  tar czf results.tar.gz output/
  DATA_NAME="$(basename $DATA_URL)"  # foo.tar.gz
  DATA_NAME="${DATA_NAME%.*}"        # foo.tar
  DATA_NAME="${DATA_NAME%.*}"        # foo
  RESULTS_ARCHIVE="$DATA_NAME-$AUTHENTICATOR.tar.gz"
  gsutil cp results.tar.gz "gs://$BUCKET/results/$RESULTS_ARCHIVE"

  # Shut off, to signal to the client that we're done running.
  poweroff
}

main
