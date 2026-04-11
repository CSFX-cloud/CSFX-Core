#!/usr/bin/env bash

set -euo pipefail

TARGET_HOST="${1:-rootcsfx@192.168.1.36}"
FLAKE_NAME="csfx-server"

echo "target=$TARGET_HOST flake=$FLAKE_NAME"

if ! ssh -o ConnectTimeout=5 "$TARGET_HOST" "echo ok" > /dev/null 2>&1; then
  echo "[ERROR] ssh connection failed target=$TARGET_HOST"
  exit 1
fi

REMOTE_DIR="/tmp/csfx-nixos-deploy-$$"
ssh "$TARGET_HOST" "mkdir -p $REMOTE_DIR"
rsync -az --delete \
  --exclude='.git' \
  --exclude='*.qcow2' \
  --exclude='result' \
  ./ "$TARGET_HOST:$REMOTE_DIR/"

ssh -t "$TARGET_HOST" "cd $REMOTE_DIR && sudo nixos-rebuild switch --flake .#$FLAKE_NAME"

ssh "$TARGET_HOST" "rm -rf $REMOTE_DIR"

echo "[INFO] deployment complete target=$TARGET_HOST"
