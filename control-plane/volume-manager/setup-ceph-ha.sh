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

# Initialize Ceph configuration without auth
log_info "Initializing Ceph configuration..."
chmod +x ./init-ceph-config.sh
./init-ceph-config.sh

# Clean up old containers if any
log_info "Cleaning up old containers..."
docker-compose -f docker-compose.ceph.yml down -v 2>/dev/null || true

# Start etcd first
log_info "Starting etcd cluster..."
docker-compose -f docker-compose.ceph.yml up -d etcd1 etcd2 etcd3

log_info "Waiting for etcd to be ready (10s)..."
sleep 10

# Start Ceph Monitors
log_info "Starting Ceph monitors..."
docker-compose -f docker-compose.ceph.yml up -d ceph-mon1 ceph-mon2 ceph-mon3

# Wait for Monitors to create keyrings
log_info "Waiting for Ceph monitors to initialize and create keyrings (40s)..."
sleep 40

# Check if monitors are ready
log_info "Checking Ceph monitor status..."
docker exec ceph-mon1 ceph mon stat || log_warn "Monitors not fully ready yet"

# Now start OSDs (they will retry until keyrings are available)
log_info "Starting Ceph OSDs..."
docker-compose -f docker-compose.ceph.yml up -d ceph-osd1 ceph-osd2 ceph-osd3

# Wait for OSDs to join
log_info "Waiting for OSDs to join the cluster (30s)..."
sleep 30

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

# Start Volume Managers
log_info "Starting Volume Managers..."
docker-compose -f docker-compose.ceph.yml up -d volume-manager-1 volume-manager-2 volume-manager-3

log_info "Waiting for Volume Managers to initialize (10s)..."
sleep 10

# Start PostgreSQL instances
log_info "Starting PostgreSQL instances..."
docker-compose -f docker-compose.ceph.yml up -d postgres1 postgres2 postgres3

# Wait for PostgreSQL
log_info "Waiting for PostgreSQL instances (20s)..."
sleep 20

# Start HAProxy
log_info "Starting HAProxy..."
docker-compose -f docker-compose.ceph.yml up -d postgres-haproxy

log_info "Waiting for HAProxy to be ready (5s)..."
sleep 5

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
log_info "2. Access HAProxy stats: http://localhost:8000"
log_info "3. Connect to PostgreSQL: psql -h localhost -p 5432 -U csf -d csf_core"
log_info "4. Check Ceph: docker exec ceph-mon1 ceph status"
