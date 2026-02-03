#!/usr/bin/env bash
# CSF-Core Remote Deployment Script
# Deployt die NixOS-Konfiguration auf einen Remote-Server

set -euo pipefail

# Konfiguration
TARGET_HOST="${1:-rootcsf@192.168.1.36}"
FLAKE_NAME="csf-server"

echo "🚀 CSF-Core NixOS Deployment"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Target: $TARGET_HOST"
echo "Flake: $FLAKE_NAME"
echo ""

# Prüfe ob SSH-Verbindung funktioniert
echo "📡 Testing SSH connection..."
if ! ssh -o ConnectTimeout=5 "$TARGET_HOST" "echo 'Connection OK'" > /dev/null 2>&1; then
    echo "❌ Cannot connect to $TARGET_HOST"
    exit 1
fi
echo "✅ SSH connection successful"
echo ""

# Kopiere Flake zum Server
echo "📦 Copying flake to remote server..."
REMOTE_DIR="/tmp/csf-nixos-deploy-$$"
ssh "$TARGET_HOST" "mkdir -p $REMOTE_DIR"
rsync -az --delete \
    --exclude='.git' \
    --exclude='*.qcow2' \
    --exclude='result' \
    ./ "$TARGET_HOST:$REMOTE_DIR/"
echo "✅ Flake copied"
echo ""

# Führe nixos-rebuild auf dem Server aus
echo "🔨 Building and activating configuration on remote server..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ssh -t "$TARGET_HOST" "cd $REMOTE_DIR && sudo nixos-rebuild switch --flake .#$FLAKE_NAME"

# Cleanup
echo ""
echo "🧹 Cleaning up..."
ssh "$TARGET_HOST" "rm -rf $REMOTE_DIR"

echo ""
echo "✅ Deployment complete!"
echo ""
echo "📝 Next steps:"
echo "  - Test Docker: ssh $TARGET_HOST 'docker --version'"
echo "  - Run test script: ssh $TARGET_HOST 'sudo /root/test-csf-backend.sh'"
echo "  - Check backend: curl http://192.168.1.36:8000/health"
