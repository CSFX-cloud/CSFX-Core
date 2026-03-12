#!/usr/bin/env bash
set -euo pipefail

ETCD_ENDPOINT="${ETCD_ENDPOINT:-http://localhost:2379}"
ETCD_USERNAME="${ETCD_USERNAME:-csf}"
ETCD_PASSWORD="${ETCD_PASSWORD:?ETCD_PASSWORD must be set}"
COMPOSE_FILE="${COMPOSE_FILE:-/opt/csf/docker-compose.prod.yml}"
GHCR_ORG="${GHCR_ORG:-csfx-cloud}"
POLL_INTERVAL="${POLL_INTERVAL:-30}"

GHCR_TOKEN="${GHCR_TOKEN:?GHCR_TOKEN must be set}"
ETCD_DESIRED_KEY="/csf/config/desired_cp_version"
ETCD_RESULT_KEY="/csf/config/last_update_result"

SERVICES=(api-gateway registry scheduler volume-manager failover-controller sdn-controller)

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"
}

etcd_auth_token() {
    curl -sf "${ETCD_ENDPOINT}/v3/auth/authenticate" \
        -X POST \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"${ETCD_USERNAME}\", \"password\": \"${ETCD_PASSWORD}\"}" \
        | jq -r '.token // empty'
}

etcd_get() {
    local token
    token="$(etcd_auth_token)"
    curl -sf "${ETCD_ENDPOINT}/v3/kv/range" \
        -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: ${token}" \
        -d "{\"key\": \"$(printf '%s' "$1" | base64 -w0)\"}" \
        | jq -r '.kvs[0].value // empty' \
        | base64 -d 2>/dev/null || true
}

etcd_put() {
    local token
    token="$(etcd_auth_token)"
    curl -sf "${ETCD_ENDPOINT}/v3/kv/put" \
        -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: ${token}" \
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

ghcr_digest() {
    local image="$1" tag="$2"
    curl -sf \
        -H "Authorization: Bearer ${GHCR_TOKEN}" \
        -H "Accept: application/vnd.docker.distribution.manifest.v2+json" \
        "https://ghcr.io/v2/${image}/manifests/${tag}" \
        -I | grep -i "^docker-content-digest:" | tr -d '[:space:]' | cut -d: -f2-
}

local_digest() {
    docker inspect --format='{{index .RepoDigests 0}}' "$1" 2>/dev/null \
        | cut -d@ -f2 || true
}

verify_images() {
    local version="$1"
    log "verifying image digests against GHCR"
    for svc in "${SERVICES[@]}"; do
        local image="ghcr.io/${GHCR_ORG}/csf-ce-${svc}"
        local remote_digest local_dig
        remote_digest="$(ghcr_digest "${GHCR_ORG}/csf-ce-${svc}" "${version}")"
        local_dig="$(local_digest "${image}:${version}")"

        if [[ -z "$remote_digest" ]]; then
            log "failed to fetch remote digest for ${svc}"
            return 1
        fi
        if [[ "$remote_digest" != "$local_dig" ]]; then
            log "digest mismatch for ${svc}: remote=${remote_digest} local=${local_dig}"
            return 1
        fi
        log "verified ${svc}: ${remote_digest}"
    done
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

    if ! verify_images "$version"; then
        log "image verification failed, aborting update"
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

is_valid_version() {
    [[ "$1" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9._-]+)?$ ]]
}

log "csf-updater started, polling etcd every ${POLL_INTERVAL}s"

last_applied=""

while true; do
    desired="$(etcd_get "$ETCD_DESIRED_KEY")"

    if [[ -n "$desired" && "$desired" != "$last_applied" ]]; then
        if ! is_valid_version "$desired"; then
            log "rejected invalid version string: ${desired}"
            etcd_put "$ETCD_RESULT_KEY" "failed"
            last_applied="$desired"
            sleep "$POLL_INTERVAL"
            continue
        fi

        log "desired version: ${desired}, last applied: ${last_applied:-none}"
        if run_update "$desired"; then
            last_applied="$desired"
        else
            last_applied="$desired"
        fi
    fi

    sleep "$POLL_INTERVAL"
done
