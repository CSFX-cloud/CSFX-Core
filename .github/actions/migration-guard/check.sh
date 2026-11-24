#!/usr/bin/env bash
set -euo pipefail

previous_ref="${1:?previous ref required}"
migration_dir="${2:-control-plane/shared/migration/src}"
pattern='^m[0-9]{8}_[0-9]{6}_.*\.rs$'

previous=$(git ls-tree -r --name-only "$previous_ref" -- "$migration_dir" | xargs -n1 basename | { grep -E "$pattern" || true; } | sort)
current=$(ls "$migration_dir" | { grep -E "$pattern" || true; } | sort)
missing=$(comm -23 <(printf '%s\n' "$previous") <(printf '%s\n' "$current") | sed '/^$/d')

if [ -n "$missing" ]; then
  while read -r file; do
    echo "[ERROR] migration of the previous release is missing in this build previous=${previous_ref} file=${file}"
  done <<< "$missing"
  exit 1
fi

echo "[INFO] migrations are a superset of the previous release previous=${previous_ref}"
