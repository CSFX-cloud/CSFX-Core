#!/usr/bin/env bash
set -euo pipefail

ETCD_ENDPOINT="${ETCD_ENDPOINT:-http://localhost:2379}"
COMPOSE_FILE="${COMPOSE_FILE:-/opt/csf/docker-compose.prod.yml}"
GHCR_ORG="${GHCR_ORG:-csfx-cloud}"
POLL_INTERVAL="${POLL_INTERVAL:-30}"

ETCD_DESIRED_KEY="/csf/config/desired_cp_version"
ETCD_RESULT_KEY="/csf/config/last_update_result"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"
}

etcd_get() {
    curl -sf "${ETCD_ENDPOINT}/v3/kv/range" \
        -X POST \
        -H "Content-Type: application/json" \
        -d "{\"key\": \"$(printf '%s' "$1" | base64 -w0)\"}" \
        | jq -r '.kvs[0].value // empty' \
        | base64 -d 2>/dev/null || true
}

etcd_put() {
    curl -sf "${ETCD_ENDPOINT}/v3/kv/put" \
        -X POST \
        -H "Content-Type: application/json" \
        -d "{\"key\": \"$(printf '%s' "$1" | base64 -w0)\", \"value\": \"$(printf '%s' "$2" | base64 -w0)\"}" \
        > /dev/null
}

current_version() {
    docker compose -f "$COMPOSE_FILE" \
        --env-file "$(dirname "$COMPOSE_FILE")/.env" \
        images --format json 2>/dev/null \
        | jq -r '.[0].Tag // empty' \
        | head -1 || true
}

run_update() {
    local version="$1"
    log "starting update to ${version}"

    etcd_put "$ETCD_RESULT_KEY" "in_progress"

    log "pulling images"
    if ! GHCR_ORG="$GHCR_ORG" CSF_VERSION="$version" \
        docker compose -f "$COMPOSE_FILE" pull; then
        log "pull failed"
        etcd_put "$ETCD_RESULT_KEY" "failed"
        return 1
    fi

    log "restarting services"
    if ! GHCR_ORG="$GHCR_ORG" CSF_VERSION="$version" \
        docker compose -f "$COMPOSE_FILE" up -d; then
        log "up failed"
        etcd_put "$ETCD_RESULT_KEY" "failed"
        return 1
    fi

    log "waiting for health checks"
    sleep 15
    if ! GHCR_ORG="$GHCR_ORG" CSF_VERSION="$version" \
        docker compose -f "$COMPOSE_FILE" ps --format json \
        | jq -e '[.[] | select(.Health == "unhealthy")] | length == 0' > /dev/null 2>&1; then
        log "health check failed"
        etcd_put "$ETCD_RESULT_KEY" "failed"
        return 1
    fi

    etcd_put "$ETCD_RESULT_KEY" "success"
    log "update to ${version} complete"
}

log "csf-updater started, polling etcd every ${POLL_INTERVAL}s"

last_applied=""

while true; do
    desired="$(etcd_get "$ETCD_DESIRED_KEY")"

    if [[ -n "$desired" && "$desired" != "$last_applied" ]]; then
        log "desired version: ${desired}, last applied: ${last_applied:-none}"
        if run_update "$desired"; then
            last_applied="$desired"
        fi
    fi

    sleep "$POLL_INTERVAL"
done
