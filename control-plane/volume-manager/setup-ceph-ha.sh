#!/usr/bin/env bash
# Setup-Script für Ceph + PostgreSQL HA

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_info "Starting CSF-Core HA setup with Ceph storage..."

# Start Services
log_info "Starting services..."
docker-compose -f docker-compose.ceph.yml up -d

# Wait for Ceph Monitors
log_info "Waiting for Ceph monitors to start (30s)..."
sleep 30

# Wait for Ceph OSDs
log_info "Waiting for Ceph OSDs to start (20s)..."
sleep 20

# Check Ceph Health
log_info "Checking Ceph health..."
docker exec ceph-mon1 ceph -s || log_warn "Ceph not fully ready yet"

# Wait for Ceph to be healthy
log_info "Waiting for Ceph cluster to become healthy..."
for i in {1..12}; do
    if docker exec ceph-mon1 ceph health | grep -q "HEALTH_OK\|HEALTH_WARN"; then
        log_info "Ceph cluster is healthy!"
        break
    fi
    log_info "Attempt $i/12: Waiting 10s..."
    sleep 10
done

# Show Ceph status
log_info "Ceph Status:"
docker exec ceph-mon1 ceph status

# Create Ceph pools (if not exists)
log_info "Creating Ceph pools..."
docker exec ceph-mon1 ceph osd pool create csf-volumes 128 || log_warn "Pool csf-volumes already exists"
docker exec ceph-mon1 ceph osd pool create csf-postgres 64 || log_warn "Pool csf-postgres already exists"
docker exec ceph-mon1 ceph osd pool create csf-metadata 32 || log_warn "Pool csf-metadata already exists"

# Enable RBD application
log_info "Enabling RBD application on pools..."
docker exec ceph-mon1 ceph osd pool application enable csf-volumes rbd || true
docker exec ceph-mon1 ceph osd pool application enable csf-postgres rbd || true
docker exec ceph-mon1 ceph osd pool application enable csf-metadata rbd || true

# Show pools
log_info "Ceph Pools:"
docker exec ceph-mon1 ceph osd pool ls

# Wait for PostgreSQL
log_info "Waiting for PostgreSQL instances (20s)..."
sleep 20

# Check PostgreSQL
log_info "Checking PostgreSQL instances..."
for i in 1 2 3; do
    if docker exec postgres${i} pg_isready -U csf -d csf_core > /dev/null 2>&1; then
        log_info "PostgreSQL ${i}: Ready"
    else
        log_warn "PostgreSQL ${i}: Not ready yet"
    fi
done

# Show running containers
log_info "Running containers:"
docker-compose -f docker-compose.ceph.yml ps

log_info "Setup complete!"
log_info ""
log_info "Next steps:"
log_info "1. Run './test-ha-failover.sh' to test failover scenarios"
log_info "2. Access HAProxy stats: http://localhost:7000"
log_info "3. Connect to PostgreSQL: psql -h localhost -p 5432 -U csf -d csf_core"
log_info "4. Check Ceph: docker exec ceph-mon1 ceph status"
