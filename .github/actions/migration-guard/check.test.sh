#!/usr/bin/env bash
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/check.sh"
dir=control-plane/shared/migration/src
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
git init -q
git config user.email test@example.invalid
git config user.name test
mkdir -p "$dir"
touch "$dir/m20261116_010000_add_audit_log.rs" "$dir/m20261117_000000_add_hotpath_indexes.rs" "$dir/runner.rs"
git add -A
git commit -q -m base
git tag previous

expect_success() {
  bash "$script" previous > /dev/null || { echo "[ERROR] expected success case=$1"; exit 1; }
}

expect_failure() {
  ! bash "$script" previous > /dev/null 2>&1 || { echo "[ERROR] expected failure case=$1"; exit 1; }
}

expect_success unchanged
touch "$dir/m20261118_000000_add_exports.rs"
expect_success added
rm "$dir/m20261117_000000_add_hotpath_indexes.rs"
expect_failure removed
touch "$dir/m20261117_000001_add_hotpath_indexes.rs"
expect_failure renamed
echo "[INFO] migration guard tests passed"
