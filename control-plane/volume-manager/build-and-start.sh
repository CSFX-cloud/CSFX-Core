#!/bin/bash

# 🚀 Build und starte das Hybridsystem
# Baut alle erforderlichen Images und startet den Stack

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  🚀 CSF Hybrid System Builder             ║${NC}"
echo -e "${BLUE}║  etcd + Ceph + PostgreSQL/Patroni         ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════╝${NC}"
echo ""

# Schritt 1: Erstelle Ceph Config
echo -e "${YELLOW}📁 Creating Ceph configuration...${NC}"
mkdir -p ceph-config

if [ ! -f ceph-config/ceph.conf ]; then
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
    echo -e "${GREEN}✅ Ceph config created${NC}"
else
    echo -e "${GREEN}✅ Ceph config exists${NC}"
fi
echo ""

# Schritt 2: Pull offizielle Images
echo -e "${YELLOW}📥 Pulling official Docker images...${NC}"
echo ""

echo -e "${BLUE}Pulling Spilo (Patroni) image from Zalando...${NC}"
docker pull ghcr.io/zalando/spilo-15:3.0-p1
echo -e "${GREEN}✅ Spilo image ready${NC}"
echo ""

echo -e "${BLUE}Building Volume Manager image...${NC}"
docker build -f Dockerfile.test -t volume-manager:patroni ../..
echo -e "${GREEN}✅ Volume Manager image built${NC}"
echo ""

# Schritt 3: Optional cleanup
read -p "$(echo -e ${YELLOW}Clean up old containers and volumes? [y/N]: ${NC})" -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}⚠️  Stopping and removing old containers...${NC}"
    docker-compose -f docker-compose.patroni.yml down -v
    echo -e "${GREEN}✅ Cleanup complete${NC}"
fi
echo ""

# Schritt 4: Starte Services
echo -e "${YELLOW}🚀 Starting all services...${NC}"
docker-compose -f docker-compose.patroni.yml up -d
echo -e "${GREEN}✅ Services started${NC}"
echo ""

# Schritt 5: Health Checks
echo -e "${YELLOW}⏳ Waiting for services to initialize...${NC}"
echo -e "${BLUE}This may take 60-90 seconds...${NC}"
echo ""

# Warte auf etcd
echo -n "Waiting for etcd..."
for i in {1..30}; do
    if docker exec etcd1 etcdctl endpoint health &>/dev/null; then
        echo -e " ${GREEN}✅${NC}"
        break
    fi
    echo -n "."
    sleep 2
done
echo ""

# Warte auf Ceph
echo -n "Waiting for Ceph cluster..."
for i in {1..60}; do
    if docker exec ceph-mon1 ceph health &>/dev/null; then
        echo -e " ${GREEN}✅${NC}"
        break
    fi
    echo -n "."
    sleep 2
done
echo ""

# Zeige Ceph Status
echo ""
echo -e "${BLUE}📊 Ceph Cluster Status:${NC}"
docker exec ceph-mon1 ceph -s 2>/dev/null || echo -e "${YELLOW}⚠️  Ceph still initializing...${NC}"
echo ""

# Warte auf Patroni
echo -n "Waiting for Patroni cluster..."
for i in {1..60}; do
    if curl -s http://localhost:8008/health &>/dev/null; then
        echo -e " ${GREEN}✅${NC}"
        break
    fi
    echo -n "."
    sleep 2
done
echo ""

# Zeige Patroni Status
echo ""
echo -e "${BLUE}🗄️  PostgreSQL Cluster Status:${NC}"
for port in 8008 8009 8010; do
    if curl -s http://localhost:$port/health &>/dev/null; then
        role=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.role' 2>/dev/null)
        state=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.state' 2>/dev/null)
        
        case $port in
            8008) node="patroni1" ;;
            8009) node="patroni2" ;;
            8010) node="patroni3" ;;
        esac
        
        if [ "$role" == "master" ] || [ "$role" == "primary" ]; then
            echo -e "  ${GREEN}👑 $node: $role ($state)${NC}"
        else
            echo -e "  ${BLUE}🔄 $node: $role ($state)${NC}"
        fi
    fi
done
echo ""

# Schritt 6: Erfolgsmeldung
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ✅ System erfolgreich gestartet!         ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}🎯 Nächste Schritte:${NC}"
echo ""
echo -e "  ${YELLOW}1.${NC} System testen:"
echo -e "     ${BLUE}./test-hybrid-system.sh${NC}"
echo ""
echo -e "  ${YELLOW}2.${NC} PostgreSQL verbinden:"
echo -e "     ${BLUE}./connect-postgres.sh${NC}"
echo ""
echo -e "  ${YELLOW}3.${NC} Logs anzeigen:"
echo -e "     ${BLUE}docker-compose -f docker-compose.patroni.yml logs -f${NC}"
echo ""
echo -e "  ${YELLOW}4.${NC} Status prüfen:"
echo -e "     ${BLUE}docker-compose -f docker-compose.patroni.yml ps${NC}"
echo ""
echo -e "${BLUE}📚 Dokumentation:${NC}"
echo -e "  - ${BLUE}HYBRID_SYSTEM_TESTING.md${NC} - Umfassende Test-Dokumentation"
echo -e "  - ${BLUE}PATRONI_HA_ARCHITECTURE.md${NC} - Patroni Architektur"
echo -e "  - ${BLUE}CEPH_HA_README.md${NC} - Ceph Setup"
echo ""
