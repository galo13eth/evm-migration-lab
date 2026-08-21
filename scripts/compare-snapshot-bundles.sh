#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <provider-a-bundle> <provider-b-bundle>" >&2
  exit 2
fi

left=$1
right=$2
files=(manifest.json proofs.json root.txt reconciliation.json artifact-digests.json)

for bundle in "$left" "$right"; do
  [[ -f "$bundle/READY" ]] || { echo "bundle is not ready: $bundle" >&2; exit 1; }
done

for file in "${files[@]}"; do
  cmp --silent "$left/$file" "$right/$file" || {
    echo "provider outputs differ: $file" >&2
    exit 1
  }
done

echo "provider outputs are byte-identical across ${#files[@]} committed artifacts"
