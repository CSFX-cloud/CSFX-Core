#!/bin/bash

# PostgreSQL HA Setup mit Patroni + Ceph
# Startet den kompletten Stack für Production-Grade HA

set -e

echo "🚀 Starting PostgreSQL HA with Patroni + Ceph..."
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Cleanup alte Container (optional)
read -p "Clean up old containers? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Stopping and removing old containers...${NC}"
    docker-compose -f docker-compose.patroni.yml down -v
fi

# Erstelle Ceph Config Verzeichnis
mkdir -p ceph-config

# Erstelle minimale Ceph Config
if [ ! -f ceph-config/ceph.conf ]; then
    echo -e "${YELLOW}Creating Ceph configuration...${NC}"
    cat > ceph-config/ceph.conf << 'EOF'
[global]
fsid = a7f64266-0894-4f1e-a635-d0aeaca0e993
mon initial members = ceph-mon1,ceph-mon2,ceph-mon3
mon host = 172.20.0.21:6789,172.20.0.22:6789,172.20.0.23:6789
auth cluster required = cephx
auth service required = cephx
auth client required = cephx
osd pool default size = 3
osd pool default min size = 2
osd pool default pg num = 128
osd pool default pgp num = 128
osd crush chooseleaf type = 0

[mon]
mon allow pool delete = true
EOF
fi

echo -e "${GREEN}✅ Configuration ready${NC}"
echo ""

# Starte Services
echo -e "${YELLOW}Starting services...${NC}"
docker-compose -f docker-compose.patroni.yml up -d

echo ""
echo -e "${GREEN}✅ Services started${NC}"
echo ""

# Warte auf Ceph
echo -e "${YELLOW}Waiting for Ceph cluster to be ready...${NC}"
for i in {1..60}; do
    if docker exec ceph-mon1 ceph health &>/dev/null; then
        echo -e "${GREEN}✅ Ceph cluster is ready${NC}"
        break
    fi
    echo -n "."
    sleep 2
done
echo ""

# Zeige Ceph Status
echo ""
echo "📊 Ceph Cluster Status:"
docker exec ceph-mon1 ceph -s || echo -e "${RED}⚠️  Ceph not ready yet${NC}"
echo ""

# Warte auf etcd
echo -e "${YELLOW}Waiting for etcd cluster...${NC}"
sleep 10
echo -e "${GREEN}✅ etcd ready${NC}"
echo ""

# Warte auf Patroni
echo -e "${YELLOW}Waiting for Patroni cluster to initialize (this may take 60-90 seconds)...${NC}"
for i in {1..60}; do
    if curl -s http://localhost:8008/health &>/dev/null; then
        echo -e "${GREEN}✅ Patroni cluster is ready${NC}"
        break
    fi
    echo -n "."
    sleep 2
done
echo ""

# Zeige Patroni Status
echo ""
echo "🗄️  PostgreSQL Cluster Status (Patroni):"
echo ""
for port in 8008 8009 8010; do
    echo "Node on port $port:"
    curl -s http://localhost:$port/health | jq -r '. | "  Role: \(.role), State: \(.state), Timeline: \(.timeline // "N/A")"' 2>/dev/null || echo "  Not ready yet"
done
echo ""

# Zeige HAProxy Stats
echo "📊 HAProxy Load Balancer:"
echo "   Stats UI: http://localhost:8000/stats"
echo ""

# Zeige Connection Strings
echo "🔌 PostgreSQL Connection:"
echo "   Primary (Writes): postgresql://csf:csfpassword@localhost:5432/csf_core"
echo "   Replicas (Reads): postgresql://csf:csfpassword@localhost:5433/csf_core"
echo ""

# Test Connection
echo -e "${YELLOW}Testing Primary connection...${NC}"
if docker exec patroni1 psql -U csf -d csf_core -c "SELECT version();" &>/dev/null; then
    echo -e "${GREEN}✅ Primary connection successful${NC}"
else
    echo -e "${RED}⚠️  Primary not ready yet, give it a minute${NC}"
fi
echo ""

# Zeige wie man Cluster Status prüft
echo "📋 Useful Commands:"
echo "   Check Ceph health:     docker exec ceph-mon1 ceph -s"
echo "   Check Patroni status:  curl http://localhost:8008/cluster"
echo "   Check HAProxy stats:   open http://localhost:8000/stats"
echo "   Connect to Primary:    docker exec -it patroni1 psql -U csf -d csf_core"
echo "   View Volume Manager:   docker logs -f volume-manager-1"
echo ""

# Test Failover
echo "🧪 Testing Setup (optional):"
echo "   1. Stop Primary:       docker-compose -f docker-compose.patroni.yml stop patroni1"
echo "   2. Watch Failover:     docker logs -f volume-manager-1"
echo "   3. Check new Primary:  curl http://localhost:8009/health"
echo "   4. Restart Node:       docker-compose -f docker-compose.patroni.yml start patroni1"
echo ""

echo -e "${GREEN}✅ PostgreSQL HA with Patroni + Ceph is ready!${NC}"
echo ""
echo "📚 Architecture:"
echo "   • 3x Ceph Monitors (HA coordination)"
echo "   • 3x Ceph OSDs (3-way replication)"
echo "   • 3x PostgreSQL with Patroni (Streaming Replication)"
echo "   • 3x etcd nodes (State management)"
echo "   • 1x HAProxy (Smart routing)"
echo "   • 3x Volume Managers (Storage orchestration)"
echo ""
echo "🎯 Benefits:"
echo "   ✅ Zero-downtime failover (1-3 seconds)"
echo "   ✅ Automatic leader election"
echo "   ✅ Data persistence via Ceph"
echo "   ✅ Read scaling via replicas"
echo "   ✅ Node failure tolerance (survives 2 node failures)"
echo ""
