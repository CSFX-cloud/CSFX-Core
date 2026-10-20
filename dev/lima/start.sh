#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STATE_DIR="${CSFX_LIMA_STATE:-$HOME/.csfx-lima}"

log() {
  printf '[%s] %s\n' "$1" "$2" >&2
}

current_mode() {
  cat "$STATE_DIR/mode" 2> /dev/null || echo none
}

confirm_switch() {
  local answer
  read -r -p "switching from $1 to $2 destroys the current vms, continue? [y/N] " answer
  [ "$answer" = y ] || { log INFO "aborted"; exit 0; }
}

choose() {
  local answer
  read -r -p "mode [single/cluster]: " answer
  echo "$answer"
}

start() {
  local target="$1" arg="$2" current
  current="$(current_mode)"
  if [ "$current" != none ] && [ "$current" != "$target" ]; then
    confirm_switch "$current" "$target"
    "$DIR/csfx-lima" destroy
  fi
  "$DIR/csfx-lima" up $arg
}

main() {
  local mode="${1:-}"
  [ -n "$mode" ] || mode="$(choose)"
  case "$mode" in
    single) start single "" ;;
    cluster) start ha ha ;;
    stop) "$DIR/csfx-lima" down ;;
    destroy) "$DIR/csfx-lima" destroy ;;
    *) log ERROR "usage: start.sh single | cluster | stop | destroy"; exit 1 ;;
  esac
}

main "$@"
