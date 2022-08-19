#!/usr/bin/env bash
set -xeufo pipefail

BUCKET=zjn-scaling-tuf-data

main() {

  # Tar up current source, and copy to GCS
  TMPDIR=$(mktemp -d)
  pushd ../
  git archive \
      --prefix "scaling-tuf/" \
      --output "$TMPDIR/source.tar.gz" \
      HEAD
  gsutil cp "$TMPDIR/source.tar.gz" "gs://$BUCKET/source.tar.gz"
  rm -rf "$TMPDIR"
  popd

  # The infrastructure will just run everything it needs to.
  terraform apply -auto-approve

  # Retry until there are *no more* running instances.
  while [ ! -z "$(gcloud compute instances list --filter STATUS=RUNNING)" ]; do
    echo Retry...
    echo "If you want to check up, run \"gcloud compute ssh simulator-1 --command \'journalctl --unit google-startup-scripts -f\'\"".
    sleep 5
  done

  # See where the results wound up.
  gsutil ls "gs://$BUCKET/results"
}

main
