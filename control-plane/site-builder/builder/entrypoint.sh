#!/bin/bash
set -uo pipefail

WORKDIR=/workspace/source
LOG_FILE=/tmp/csfx-build.log
OFFSET_FILE=/tmp/csfx-build.offset
LOCK_FILE=/tmp/csfx-build.lock
ARTIFACT=/tmp/csfx-artifact.tar.gz
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
    log ERROR "$1"
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

build_container() {
    local dockerfile=Dockerfile
    if [ ! -f "$PROJECT_DIR/Dockerfile" ]; then
        node_dockerfile || fail "no Dockerfile found and the project cannot be built automatically"
        dockerfile=Dockerfile.csfx
    fi
    local arguments=()
    mapfile -t arguments < <(build_arguments)
    local insecure=()
    [ "${CSFX_REGISTRY_INSECURE:-}" = "1" ] && insecure=(--insecure --skip-tls-verify)
    [ -n "${CSFX_REGISTRY_MIRROR:-}" ] && insecure+=(--registry-mirror "$CSFX_REGISTRY_MIRROR" --insecure-registry "$CSFX_REGISTRY_MIRROR")
    log INFO "building image reference=$CSFX_IMAGE_REF"
    kaniko-executor --context "$PROJECT_DIR" --dockerfile "$PROJECT_DIR/$dockerfile" \
        --destination "$CSFX_IMAGE_REF" "${insecure[@]}" "${arguments[@]}" >>"$LOG_FILE" 2>&1 \
        || fail "image build failed"
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
