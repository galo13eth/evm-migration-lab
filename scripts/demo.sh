#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
run_dir="$(mktemp -d)"
source_rpc="http://127.0.0.1:8545"
destination_rpc="http://127.0.0.1:8546"

cleanup() {
  kill "$source_pid" "$destination_pid" 2>/dev/null || true
  wait "$source_pid" "$destination_pid" 2>/dev/null || true
  rm -rf "$run_dir"
}
trap cleanup EXIT

anvil --silent --port 8545 --chain-id 31337 >"$run_dir/source.log" 2>&1 &
source_pid=$!
anvil --silent --port 8546 --chain-id 31338 >"$run_dir/destination.log" 2>&1 &
destination_pid=$!

for rpc in "$source_rpc" "$destination_rpc"; do
  for _ in {1..50}; do
    cast chain-id --rpc-url "$rpc" >/dev/null 2>&1 && break
    sleep 0.1
  done
  cast chain-id --rpc-url "$rpc" >/dev/null
done

SOURCE_RPC_URL="$source_rpc" \
DESTINATION_RPC_URL="$destination_rpc" \
CARGO="${CARGO:-cargo}" \
npm test --prefix "$repo_root/e2e"
