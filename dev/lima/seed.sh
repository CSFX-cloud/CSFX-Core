#!/usr/bin/env bash
set -euo pipefail

API="${CSFX_API:?CSFX_API}"
TOKEN=""

log() {
  printf '[%s] %s\n' "$1" "$2" >&2
}

die() {
  log ERROR "$1"
  exit 1
}

request() {
  local method="$1" path="$2" body="${3:-}"
  local args=(-sk -X "$method" "$API$path" -H 'Content-Type: application/json' -w '\n%{http_code}')
  [ -n "$TOKEN" ] && args+=(-H "Authorization: Bearer $TOKEN")
  [ -n "${CSFX_HOST:-}" ] && args+=(-H "Host: $CSFX_HOST")
  [ -n "$body" ] && args+=(-d "$body")
  curl "${args[@]}"
}

response_body() {
  sed '$d'
}

response_status() {
  tail -n 1
}

encrypt() {
  local key_file
  key_file="$(mktemp)"
  until curl -skf -m 5 "$API/api/public-key" | jq -er .public_key > "$key_file"; do
    sleep 3
  done
  printf '%s' "$1" | openssl pkeyutl -encrypt -pubin -inkey "$key_file" \
    -pkeyopt rsa_padding_mode:oaep -pkeyopt rsa_oaep_md:sha256 -pkeyopt rsa_mgf1_md:sha256 | base64 | tr -d '\n'
  rm -f "$key_file"
}

login() {
  local reply
  reply="$(request POST /api/login "$(jq -n --arg user "$CSFX_ADMIN_EMAIL" --arg password "$(encrypt "$CSFX_ADMIN_PASSWORD")" \
    '{username: $user, encrypted_password: $password}')")"
  [ "$(echo "$reply" | response_status)" = 200 ] || die "login failed user=$CSFX_ADMIN_EMAIL"
  TOKEN="$(echo "$reply" | response_body | jq -r .token)"
}

setup() {
  local reply status
  reply="$(request POST /api/setup "$(jq -n \
    --arg code "$CSFX_SETUP_CODE" \
    --arg password "$(encrypt "$CSFX_ADMIN_PASSWORD")" \
    --arg pin "$(encrypt "$CSFX_CONSOLE_PIN")" \
    --arg email "$CSFX_ADMIN_EMAIL" \
    '{setup_code: $code, organization_name: "CSFX Dev", admin_email: $email, encrypted_password: $password, encrypted_console_pin: $pin}')")"
  status="$(echo "$reply" | response_status)"
  case "$status" in
    201) log INFO "setup completed email=$CSFX_ADMIN_EMAIL" ;;
    410) log INFO "setup already completed" ;;
    *) die "setup failed status=$status" ;;
  esac
}

join_string() {
  local reply
  login
  reply="$(request POST /api/cluster/join-tokens "$(jq -n --arg role "$1" '{role: $role}')")"
  [ "$(echo "$reply" | response_status)" = 200 ] || die "join token failed role=$1"
  echo "$reply" | response_body | jq -r .join_string
}

exists() {
  request GET "$1" | response_body | jq -e --arg name "$2" '[.. | objects | select(.name? == $name)] | length > 0' > /dev/null
}

ensure() {
  local path="$1" name="$2" body="$3" status
  if exists "$path" "$name"; then
    log INFO "exists path=$path name=$name"
    return 0
  fi
  status="$(request POST "$path" "$body" | response_status)"
  case "$status" in
    2*) log INFO "created path=$path name=$name" ;;
    *) log WARN "create failed path=$path name=$name status=$status" ;;
  esac
}

resource_group_id() {
  request GET /api/resource-groups | response_body | jq -r --arg name "$1" '[.. | objects | select(.name? == $name)][0].id // empty'
}

seed_resource_groups() {
  ensure /api/resource-groups web-prod '{"name":"web-prod","description":"Production web tier","internal_cidr":"10.10.0.0/24"}'
  ensure /api/resource-groups data-staging '{"name":"data-staging","description":"Staging data services","internal_cidr":"10.20.0.0/24"}'
}

seed_workloads() {
  local group
  group="$(resource_group_id web-prod)"
  local name image
  for entry in "nginx-frontend nginx:alpine" "redis-cache redis:7-alpine" "api-backend traefik/whoami"; do
    name="${entry% *}"
    image="${entry#* }"
    ensure /api/workloads "$name" "$(jq -n --arg name "$name" --arg image "$image" --arg group "$group" \
      '{name: $name, image: $image, cpu_millicores: 250, memory_bytes: 268435456, disk_bytes: 1073741824, resource_group_id: ($group | select(. != ""))}')"
  done
}

seed_volumes() {
  local group
  group="$(resource_group_id web-prod)"
  for name in web-assets db-data; do
    ensure /api/volumes "$name" "$(jq -n --arg name "$name" --arg group "$group" \
      '{name: $name, size_gb: 5, resource_group_id: ($group | select(. != ""))}')"
  done
}

seed_networks() {
  local group
  group="$(resource_group_id web-prod)"
  ensure /api/networks frontend-net "$(jq -n --arg group "$group" \
    '{name: "frontend-net", cidr: "10.30.0.0/24", overlay_type: "wireguard", resource_group_id: ($group | select(. != ""))}')"
  ensure /api/networks backend-net '{"name":"backend-net","cidr":"10.31.0.0/24","overlay_type":"wireguard"}'
}

seed_ssh_keys() {
  ensure /api/ssh-keys dev-laptop '{"name":"dev-laptop","public_key":"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDevSeedKeyDevSeedKeyDevSeedKeyDevSeedKeyDevS dev@csfx"}'
}

seed_users() {
  local role_id
  role_id="$(request GET /api/organization/roles | response_body | jq -r '[.[] | select(.name == "Viewer")][0].id // empty')"
  [ -n "$role_id" ] || { log WARN "viewer role missing"; return 0; }
  ensure /api/organization/users viewer "$(jq -n --arg role "$role_id" \
    '{username: "viewer", email: "viewer@csfx.dev", password: "csfx-dev-password-1", role_id: $role, force_password_change: false}')"
}

data() {
  login
  seed_resource_groups
  seed_users
  seed_ssh_keys
  seed_networks
  seed_volumes
  seed_workloads
  log INFO "seed completed"
}

case "${1:-}" in
  setup) setup ;;
  data) data ;;
  join-string) join_string "${2:?role}" ;;
  *) die "usage: seed.sh setup | data | join-string <role>" ;;
esac
