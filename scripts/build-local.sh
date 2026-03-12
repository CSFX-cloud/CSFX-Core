#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GHCR_ORG="${GHCR_ORG:-local}"
CSF_VERSION="${CSF_VERSION:-dev}"

SERVICES=(api-gateway registry scheduler volume-manager failover-controller sdn-controller)

cd "$REPO_ROOT"

for svc in "${SERVICES[@]}"; do
    echo "building ${svc}..."
    docker build \
        -f control-plane/Dockerfile.prod.shared \
        --build-arg SERVICE_BIN="${svc}" \
        --build-arg BUILD_JOBS="$(nproc 2>/dev/null || sysctl -n hw.logicalcpu)" \
        -t "ghcr.io/${GHCR_ORG}/csf-ce-${svc}:${CSF_VERSION}" \
        -t "ghcr.io/${GHCR_ORG}/csf-ce-${svc}:latest" \
        .
    echo "built ghcr.io/${GHCR_ORG}/csf-ce-${svc}:${CSF_VERSION}"
done

echo "all images built"
echo ""
echo "run with:"
echo "  GHCR_ORG=${GHCR_ORG} CSF_VERSION=${CSF_VERSION} docker compose -f docker-compose.prod.yml up -d"
