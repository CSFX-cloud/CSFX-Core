#!/bin/sh
set -eu

DATA_DIR="${DATA_DIR:-/data}"
PAYLOAD_BYTES="${PAYLOAD_BYTES:-104857600}"

cd "$DATA_DIR"

if [ ! -f payload.bin ]; then
    head -c "$PAYLOAD_BYTES" /dev/urandom > payload.bin
    sha256sum payload.bin > payload.sha256
    echo "first_boot $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> boots.log
    sync
fi

echo "boot $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> boots.log
sync

sha256sum -c payload.sha256
cat boots.log

trap 'exit 0' TERM INT
while true; do
    sleep 1 &
    wait $!
done
