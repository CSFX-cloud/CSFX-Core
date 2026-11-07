DIR="${CSFX_RBD_SIM_DIR:-/var/lib/csfx-agent/rbd-sim}"
SIZE="${CSFX_RBD_SIM_SIZE:-5G}"

log() {
  printf '[%s] %s\n' "$1" "$2" >&2
}

fail() {
  log ERROR "$1"
  exit 1
}

mapped_device() {
  local inode
  inode="$(stat -c %i "$1")"
  losetup --list --noheadings --output NAME,BACK-INO | awk -v inode="$inode" '$2 == inode {print $1; exit}'
}

mapped_images() {
  local inode relative device
  find "$DIR" -name '*.img' -printf '%i %P\n' | while read -r inode relative; do
    device="$(losetup --list --noheadings --output NAME,BACK-INO | awk -v inode="$inode" '$2 == inode {print $1; exit}')"
    [ -z "$device" ] || echo "$device $relative"
  done
}

cmd_map() {
  local spec="$1" file device
  file="$DIR/$spec.img"
  mkdir -p "$(dirname "$file")"
  [ -f "$file" ] || truncate -s "$SIZE" "$file"
  [ -z "$(mapped_device "$file")" ] || fail "image already mapped image=$spec"
  device="$(losetup --find --show "$file")"
  log INFO "mapped image=$spec device=$device"
  echo "$device"
}

cmd_unmap() {
  losetup --detach "$1"
  log INFO "unmapped device=$1"
}

cmd_showmapped() {
  mapped_images \
    | jq -R -s -c 'split("\n") | map(select(length > 0) | split(" ")) | to_entries | map({
        id: (.key | tostring),
        pool: (.value[1] | split("/")[0]),
        namespace: "",
        name: (.value[1] | split("/")[1:] | join("/") | sub("\\.img$"; "")),
        snap: "-",
        device: .value[0]
      })'
}

cmd_resize() {
  local spec="$1" size_mb="$2" file device
  file="$DIR/$spec.img"
  [ -f "$file" ] || fail "image not found image=$spec"
  truncate -s "${size_mb}M" "$file"
  device="$(mapped_device "$file")"
  [ -z "$device" ] || losetup --set-capacity "$device"
  log INFO "resized image=$spec size_mb=$size_mb"
}

case "${1:-}" in
  resize) cmd_resize "${2:?image}" "${4:?size}" ;;
  map) cmd_map "${2:?image}" ;;
  unmap) cmd_unmap "${2:?device}" ;;
  showmapped) cmd_showmapped ;;
  *) fail "unsupported command=${1:-}" ;;
esac
