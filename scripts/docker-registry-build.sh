#!/bin/bash

set -e

export COMPOSE_DOCKER_CLI_BUILD=1
export DOCKER_BUILDKIT=1

echo "[INFO] Building with BuildKit optimizations enabled"

docker-compose -f docker-compose.registry.yml build \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  --parallel

echo "[INFO] Build complete. Starting services..."

docker-compose -f docker-compose.registry.yml up -d
