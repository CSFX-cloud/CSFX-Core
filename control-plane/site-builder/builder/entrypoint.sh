#!/bin/bash
set -uo pipefail

WORKDIR=/workspace/source
LOG_FILE=/tmp/csfx-build.log
OFFSET_FILE=/tmp/csfx-build.offset
LOCK_FILE=/tmp/csfx-build.lock
ARTIFACT=/tmp/csfx-artifact.tar.gz
KANIKO_ROOT=/kaniko-root
EMPTY_JSON='{}'
FLUSH_SECONDS=2

: >"$LOG_FILE"
echo 0 >"$OFFSET_FILE"

callback() {
    local method=$1 path=$2
    shift 2
    curl -fsS -X "$method" -H "Authorization: Bearer $CSFX_CALLBACK_TOKEN" "$@" \
        "$CSFX_CALLBACK_URL/callbacks/$CSFX_DEPLOYMENT_ID/$path"
}

log() {
    printf '[%s] %s\n' "$1" "$2" >>"$LOG_FILE"
}

send_unflushed() {
    local size flushed
    size=$(stat -c %s "$LOG_FILE")
    flushed=$(cat "$OFFSET_FILE")
    [ "$size" -gt "$flushed" ] || return 0
    tail -c +"$((flushed + 1))" "$LOG_FILE" | head -c "$((size - flushed))" \
        | callback POST log -H 'Content-Type: text/plain' --data-binary @- > /dev/null \
        && echo "$size" > "$OFFSET_FILE"
}

flush_logs() {
    (
        flock 9
        send_unflushed
    ) 9> "$LOCK_FILE"
}

stream_logs() {
    while true; do
        sleep "$FLUSH_SECONDS"
        flush_logs
    done
}

report() {
    local payload=$1
    flush_logs
    callback POST complete -H 'Content-Type: application/json' -d "$payload"
}

fail() {
    report "$(jq -n --arg error "$1" '{success: false, error: $error}')"
    exit 1
}

export_environment() {
    local exports
    exports=$(printf '%s' "${CSFX_BUILD_ENV:-$EMPTY_JSON}" | jq -r 'to_entries[] | "export \(.key)=\(.value | @sh)"') || return 1
    eval "$exports"
}

clone_repository() {
    log INFO "cloning repository branch=$CSFX_BRANCH"
    if [ -n "${CSFX_GIT_TOKEN:-}" ]; then
        export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=http.extraHeader
        GIT_CONFIG_VALUE_0="Authorization: Basic $(printf 'x-access-token:%s' "$CSFX_GIT_TOKEN" | base64 -w0)"
        export GIT_CONFIG_VALUE_0
    fi
    GIT_TERMINAL_PROMPT=0 git clone --depth 1 --single-branch --branch "$CSFX_BRANCH" -- "$CSFX_REPO_URL" "$WORKDIR" >>"$LOG_FILE" 2>&1
    local status=$?
    unset GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0 CSFX_GIT_TOKEN
    return $status
}

run_step() {
    local name=$1 command=$2
    [ -z "$command" ] && return 0
    log INFO "running $name command=$command"
    (cd "$PROJECT_DIR" && bash -c "$command") >>"$LOG_FILE" 2>&1
}

commit_details() {
    COMMIT_SHA=$(git -C "$WORKDIR" rev-parse HEAD)
    COMMIT_MESSAGE=$(git -C "$WORKDIR" log -1 --pretty=%s)
}

build_static() {
    local output="$PROJECT_DIR/${CSFX_OUTPUT_DIR:-.}"
    [ -d "$output" ] || fail "output directory ${CSFX_OUTPUT_DIR:-.} does not exist"
    tar -czf "$ARTIFACT" --exclude=.git -C "$output" . || fail "could not package output"
    log INFO "uploading artifact bytes=$(stat -c %s "$ARTIFACT")"
    flush_logs
    callback PUT artifact -H 'Content-Type: application/gzip' --data-binary @"$ARTIFACT" >>"$LOG_FILE" 2>&1 \
        || fail "artifact upload failed"
    report "$(jq -n --arg sha "$COMMIT_SHA" --arg message "$COMMIT_MESSAGE" \
        '{success: true, commit_sha: $sha, commit_message: $message}')"
}

node_dockerfile() {
    [ -f "$PROJECT_DIR/package.json" ] || return 1
    [ -n "${CSFX_START_COMMAND:-}" ] || return 1
    {
        echo "FROM node:22-slim"
        echo "WORKDIR /app"
        printf '%s' "${CSFX_BUILD_ENV:-$EMPTY_JSON}" | jq -r 'keys[] | "ARG \(.)\nENV \(.)=$\(.)"'
        echo "COPY . ."
        [ -n "${CSFX_INSTALL_COMMAND:-}" ] && echo "RUN corepack enable && $CSFX_INSTALL_COMMAND"
        [ -n "${CSFX_BUILD_COMMAND:-}" ] && echo "RUN $CSFX_BUILD_COMMAND"
        echo "ENV NODE_ENV=production"
        echo "EXPOSE ${CSFX_CONTAINER_PORT:-3000}"
        jq -n --arg command "$CSFX_START_COMMAND" '"CMD [\"sh\", \"-c\", \($command | tojson)]"' -r
    } >"$PROJECT_DIR/Dockerfile.csfx"
}

build_arguments() {
    printf '%s' "${CSFX_BUILD_ENV:-$EMPTY_JSON}" | jq -r 'to_entries[] | "--build-arg\n\(.key)=\(.value)"'
}

write_registry_credentials() {
    local registry="${CSFX_IMAGE_REF%%/*}" auth
    auth=$(printf 'd-%s:%s' "$CSFX_DEPLOYMENT_ID" "$CSFX_CALLBACK_TOKEN" | base64 -w0)
    mkdir -p "$KANIKO_ROOT/kaniko/.docker"
    jq -n --arg registry "$registry" --arg auth "$auth" '{auths: {($registry): {auth: $auth}}}' \
        > "$KANIKO_ROOT/kaniko/.docker/config.json"
    chmod 0600 "$KANIKO_ROOT/kaniko/.docker/config.json"
}

create_devices() {
    local spec
    mkdir -p "$KANIKO_ROOT/dev"
    for spec in "null 3" "zero 5" "random 8" "urandom 9"; do
        set -- $spec
        mknod -m 666 "$KANIKO_ROOT/dev/$1" c 1 "$2" 2> /dev/null || : > "$KANIKO_ROOT/dev/$1"
    done
}

create_mount_table() {
    mkdir -p "$KANIKO_ROOT/proc/self"
    printf '%s\n' \
        '1 0 0:1 / / rw - rootfs rootfs rw' \
        '2 1 0:2 / /proc rw - proc proc rw' \
        '3 1 0:3 / /dev rw - tmpfs tmpfs rw' \
        '4 1 0:4 / /sys rw - sysfs sysfs rw' > "$KANIKO_ROOT/proc/self/mountinfo"
}

prepare_kaniko_root() {
    local dockerfile=$1
    mkdir -p "$KANIKO_ROOT"/{kaniko,workspace,etc/ssl/certs,tmp,var/run,sys}
    write_registry_credentials
    cp /usr/local/bin/kaniko-executor "$KANIKO_ROOT/kaniko/executor"
    cp /etc/ssl/certs/ca-certificates.crt "$KANIKO_ROOT/etc/ssl/certs/ca-certificates.crt"
    cp /etc/resolv.conf "$KANIKO_ROOT/etc/resolv.conf"
    rm -rf "$PROJECT_DIR/.git"
    mv "$PROJECT_DIR" "$KANIKO_ROOT/workspace/context"
    create_devices
    create_mount_table
    echo "/workspace/context/$dockerfile"
}

run_kaniko() {
    local dockerfile_path=$1
    DOCKER_CONFIG=/kaniko/.docker HOME=/root chroot "$KANIKO_ROOT" /kaniko/executor --force --context /workspace/context \
        --dockerfile "$dockerfile_path" --destination "$CSFX_IMAGE_REF" "${KANIKO_ARGS[@]}" >> "$LOG_FILE" 2>&1
}

collect_kaniko_arguments() {
    local line
    KANIKO_ARGS=()
    if [ "${CSFX_REGISTRY_INSECURE:-}" = "1" ]; then
        KANIKO_ARGS+=(--insecure --skip-tls-verify)
    fi
    if [ -n "${CSFX_REGISTRY_MIRROR:-}" ]; then
        KANIKO_ARGS+=(--registry-mirror "$CSFX_REGISTRY_MIRROR" --insecure-registry "$CSFX_REGISTRY_MIRROR")
    fi
    while IFS= read -r line; do
        KANIKO_ARGS+=("$line")
    done < <(build_arguments)
}

build_container() {
    local dockerfile=Dockerfile dockerfile_path
    if [ ! -f "$PROJECT_DIR/Dockerfile" ]; then
        node_dockerfile || fail "no Dockerfile found and the project cannot be built automatically"
        dockerfile=Dockerfile.csfx
    fi
    collect_kaniko_arguments
    log INFO "building image reference=$CSFX_IMAGE_REF"
    dockerfile_path=$(prepare_kaniko_root "$dockerfile")
    run_kaniko "$dockerfile_path" || fail "image build failed"
    report "$(jq -n --arg sha "$COMMIT_SHA" --arg message "$COMMIT_MESSAGE" --arg image "$CSFX_IMAGE_REF" \
        '{success: true, commit_sha: $sha, commit_message: $message, image_ref: $image}')"
}

main() {
    export_environment || fail "invalid build environment"
    clone_repository || fail "git clone failed"
    commit_details
    PROJECT_DIR="$WORKDIR/${CSFX_ROOT_DIR:-}"
    [ -d "$PROJECT_DIR" ] || fail "root directory ${CSFX_ROOT_DIR:-} does not exist"
    if [ "$CSFX_MODE" = "static" ]; then
        run_step install "${CSFX_INSTALL_COMMAND:-}" || fail "install command failed"
        run_step build "${CSFX_BUILD_COMMAND:-}" || fail "build command failed"
        build_static
    else
        build_container
    fi
}

stream_logs &
STREAMER=$!
main
STATUS=$?
kill "$STREAMER" 2>/dev/null
exit $STATUS
