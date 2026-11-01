#!/usr/bin/env bash
set -euo pipefail

port=18200
bao server -dev -dev-root-token-id=root -dev-listen-address="127.0.0.1:${port}" > /dev/null 2>&1 &
server=$!
trap 'kill "${server}"' EXIT

for _ in $(seq 1 30); do
  curl -sf "http://127.0.0.1:${port}/v1/sys/health" > /dev/null && break
  sleep 1
done

BAO_ADDR="http://127.0.0.1:${port}" python3 "$(dirname "$0")/isolation_test.py"
