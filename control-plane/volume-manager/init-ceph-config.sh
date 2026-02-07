#!/usr/bin/env bash
# Initialisiert eine Ceph-Konfiguration ohne Authentifizierung für lokale Tests

set -euo pipefail

CEPH_CONFIG_DIR="./ceph-config"

echo "Creating Ceph configuration directory..."
mkdir -p "$CEPH_CONFIG_DIR"

echo "Generating ceph.conf without authentication..."
cat > "$CEPH_CONFIG_DIR/ceph.conf" << 'EOF'
[global]
fsid = $(uuidgen)
mon initial members = ceph-mon1
mon host = 172.20.0.21:6789
public network = 172.20.0.0/16
cluster network = 172.20.0.0/16

# Disable authentication for local testing
auth cluster required = none
auth service required = none
auth client required = none
auth supported = none

# OSD Settings
osd journal size = 100
osd pool default size = 3
osd pool default min size = 2
osd pool default pg num = 128
osd pool default pgp num = 128
osd crush chooseleaf type = 0

# Mon Settings
mon allow pool delete = true
mon max pg per osd = 500

# Performance
osd op threads = 2
osd max backfills = 1
osd recovery max active = 1

[mon]
mon allow pool delete = true

[osd]
osd mkfs type = xfs
osd mkfs options xfs = -f -i size=2048
osd mount options xfs = rw,noatime,nodiratime
EOF

# Generiere UUID für FSID
FSID=$(uuidgen | tr '[:upper:]' '[:lower:]')
sed -i.bak "s/fsid = .*/fsid = $FSID/" "$CEPH_CONFIG_DIR/ceph.conf"
rm -f "$CEPH_CONFIG_DIR/ceph.conf.bak"

echo "✅ Ceph configuration created at $CEPH_CONFIG_DIR/ceph.conf"
echo "FSID: $FSID"
cat "$CEPH_CONFIG_DIR/ceph.conf"
