#!/bin/bash

# Test-Suite für PostgreSQL HA mit Patroni + Ceph
# Testet verschiedene Failover-Szenarien

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo -e "${BLUE}  PostgreSQL HA Failover Tests (Patroni)${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo ""

# Function: Check if service is running
check_service() {
    local service=$1
    if docker ps | grep -q $service; then
        echo -e "${GREEN}✅${NC} $service"
        return 0
    else
        echo -e "${RED}❌${NC} $service"
        return 1
    fi
}

# Function: Get Patroni Primary
get_primary() {
    for port in 8008 8009 8010; do
        role=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.role' 2>/dev/null)
        if [ "$role" == "master" ] || [ "$role" == "primary" ]; then
            case $port in
                8008) echo "patroni1" ;;
                8009) echo "patroni2" ;;
                8010) echo "patroni3" ;;
            esac
            return 0
        fi
    done
    echo "none"
}

# Function: Count healthy replicas
count_replicas() {
    local count=0
    for port in 8008 8009 8010; do
        role=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.role' 2>/dev/null)
        if [ "$role" == "replica" ]; then
            ((count++))
        fi
    done
    echo $count
}

# Function: Test database write
test_write() {
    local port=$1
    local test_data="failover_test_$(date +%s)"
    
    # Ermittle aktuellen Primary
    local primary=$(get_primary)
    echo "Writing to primary: $primary"
    
    if [ "$primary" == "none" ]; then
         echo -e "${RED}❌ No primary found!${NC}"
         return 1
    fi

    # Bestimme Replica
    local replica=""
    if [ "$primary" == "patroni1" ]; then replica="patroni2"; 
    else replica="patroni1"; fi

    docker exec $primary psql -U csfx -d csfx_core -c \
        "CREATE TABLE IF NOT EXISTS failover_test (id SERIAL PRIMARY KEY, data TEXT, created_at TIMESTAMP DEFAULT NOW());" &>/dev/null
    
    docker exec $primary psql -U csfx -d csfx_core -c \
        "INSERT INTO failover_test (data) VALUES ('$test_data');" &>/dev/null
    
    # Verify on replica
    sleep 2
    local result=$(docker exec $replica psql -U csfx -d csfx_core -t -c \
        "SELECT data FROM failover_test WHERE data='$test_data';" 2>/dev/null | xargs)
    
    if [ "$result" == "$test_data" ]; then
        echo -e "${GREEN}✅ Data replicated successfully${NC}"
        return 0
    else
        echo -e "${RED}❌ Replication failed${NC}"
        return 1
    fi
}

# Menu
show_menu() {
    echo ""
    echo "Choose a test:"
    echo "  1) Check Cluster Status"
    echo "  2) Test Database Replication"
    echo "  3) Test PostgreSQL Primary Failover"
    echo "  4) Test Ceph OSD Failure"
    echo "  5) Test Volume Manager Failover"
    echo "  6) Full HA Test (All scenarios)"
    echo "  7) Monitor Cluster (Live)"
    echo "  0) Exit"
    echo ""
    read -p "Select option: " choice
}

# Test 1: Cluster Status
test_cluster_status() {
    echo ""
    echo -e "${YELLOW}📊 Checking Cluster Status...${NC}"
    echo ""
    
    echo "🗄️  PostgreSQL Nodes:"
    check_service "patroni1"
    check_service "patroni2"
    check_service "patroni3"
    echo ""
    
    primary=$(get_primary)
    replicas=$(count_replicas)
    
    echo -e "👑 Primary: ${GREEN}$primary${NC}"
    echo -e "🔄 Replicas: ${GREEN}$replicas${NC}"
    echo ""
    
    echo "💾 Ceph Storage:"
    check_service "ceph-mon1"
    check_service "ceph-osd1"
    check_service "ceph-osd2"
    echo ""
    
    echo "🎛️  Control Plane:"
    check_service "etcd1"
    check_service "volume-manager-1"
    check_service "postgres-haproxy"
    echo ""
    
    echo "Ceph Health:"
    docker exec ceph-mon1 ceph health 2>/dev/null || echo "Ceph not ready"
    echo ""
}

# Test 2: Database Replication
test_replication() {
    echo ""
    echo -e "${YELLOW}🧪 Testing Database Replication...${NC}"
    echo ""
    
    primary=$(get_primary)
    if [ "$primary" == "none" ]; then
        echo -e "${RED}❌ No primary found!${NC}"
        return 1
    fi
    
    echo "Primary is: $primary"
    echo "Writing test data..."
    
    if test_write; then
        echo -e "${GREEN}✅ Replication test passed${NC}"
    else
        echo -e "${RED}❌ Replication test failed${NC}"
    fi
    echo ""
}

# Test 3: PostgreSQL Failover
test_postgres_failover() {
    echo ""
    echo -e "${YELLOW}🧪 Testing PostgreSQL Primary Failover...${NC}"
    echo ""
    
    primary=$(get_primary)
    if [ "$primary" == "none" ]; then
        echo -e "${RED}❌ No primary found!${NC}"
        return 1
    fi
    
    echo "Current Primary: $primary"
    echo ""
    
    read -p "Stop $primary to trigger failover? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        return 0
    fi
    
    echo "Stopping $primary..."
    docker-compose -f docker-compose.patroni.yml stop $primary
    
    echo "Waiting for failover (max 30 seconds)..."
    for i in {1..30}; do
        sleep 1
        new_primary=$(get_primary)
        if [ "$new_primary" != "none" ] && [ "$new_primary" != "$primary" ]; then
            echo ""
            echo -e "${GREEN}✅ Failover successful!${NC}"
            echo "New Primary: $new_primary (took ${i}s)"
            break
        fi
        echo -n "."
    done
    echo ""
    
    # Test connection to new primary
    echo "Testing connection to new primary..."
    sleep 3
    if docker exec $new_primary psql -U csfx -d csfx_core -c "SELECT 1;" &>/dev/null; then
        echo -e "${GREEN}✅ New primary is accepting connections${NC}"
    else
        echo -e "${RED}❌ New primary not ready${NC}"
    fi
    echo ""
    
    read -p "Restart $primary? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Restarting $primary..."
        docker-compose -f docker-compose.patroni.yml start $primary
        echo -e "${GREEN}✅ $primary restarted (will join as replica)${NC}"
    fi
    echo ""
}

# Test 4: Ceph OSD Failure
test_ceph_failure() {
    echo ""
    echo -e "${YELLOW}🧪 Testing Ceph OSD Failure...${NC}"
    echo ""
    
    echo "Current Ceph Status:"
    docker exec ceph-mon1 ceph -s
    echo ""
    
    read -p "Stop ceph-osd1? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        return 0
    fi
    
    echo "Stopping ceph-osd1..."
    docker-compose -f docker-compose.patroni.yml stop ceph-osd1
    
    echo "Waiting for Ceph to detect failure..."
    sleep 10
    
    echo ""
    echo "Ceph Status (should be HEALTH_WARN with degraded PGs):"
    docker exec ceph-mon1 ceph -s
    echo ""
    
    echo -e "${YELLOW}Testing if PostgreSQL still works...${NC}"
    if docker exec patroni1 psql -U csfx -d csfx_core -c "SELECT version();" &>/dev/null; then
        echo -e "${GREEN}✅ PostgreSQL still working (Ceph has 2 replicas)${NC}"
    else
        echo -e "${RED}❌ PostgreSQL affected${NC}"
    fi
    echo ""
    
    read -p "Restart ceph-osd1? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Restarting ceph-osd1..."
        docker-compose -f docker-compose.patroni.yml start ceph-osd1
        echo "Waiting for recovery..."
        sleep 10
        echo ""
        docker exec ceph-mon1 ceph -s
    fi
    echo ""
}

# Test 5: Volume Manager Failover
test_volume_manager_failover() {
    echo ""
    echo -e "${YELLOW}🧪 Testing Volume Manager Leader Election...${NC}"
    echo ""
    
    echo "Current Volume Manager nodes:"
    check_service "volume-manager-1"
    check_service "volume-manager-2"
    check_service "volume-manager-3"
    echo ""
    
    echo "Checking logs for current leader..."
    for i in {1..3}; do
        if docker logs volume-manager-$i 2>&1 | tail -20 | grep -q "LEADER"; then
            echo -e "volume-manager-$i: ${GREEN}LEADER${NC}"
        else
            echo -e "volume-manager-$i: FOLLOWER"
        fi
    done
    echo ""
    
    read -p "Stop volume-manager-1? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        return 0
    fi
    
    docker-compose -f docker-compose.patroni.yml stop volume-manager-1
    echo "Waiting for new leader election..."
    sleep 10
    
    echo "New leader should be elected:"
    for i in {2..3}; do
        if docker logs volume-manager-$i 2>&1 | tail -20 | grep -q "LEADER"; then
            echo -e "volume-manager-$i: ${GREEN}NEW LEADER${NC}"
        else
            echo -e "volume-manager-$i: FOLLOWER"
        fi
    done
    echo ""
}

# Test 6: Full HA Test
test_full_ha() {
    echo ""
    echo -e "${YELLOW}🧪 Running Full HA Test Suite...${NC}"
    echo ""
    
    test_cluster_status
    read -p "Press Enter to continue..." dummy
    
    test_replication
    read -p "Press Enter to continue..." dummy
    
    test_postgres_failover
    read -p "Press Enter to continue..." dummy
    
    test_ceph_failure
    
    echo ""
    echo -e "${GREEN}✅ Full HA test completed${NC}"
    echo ""
}

# Test 7: Monitor
monitor_cluster() {
    echo ""
    echo -e "${YELLOW}📊 Monitoring Cluster (Ctrl+C to stop)...${NC}"
    echo ""
    
    while true; do
        clear
        echo "=== PostgreSQL HA Cluster Monitor ==="
        echo ""
        echo "Time: $(date)"
        echo ""
        
        primary=$(get_primary)
        replicas=$(count_replicas)
        
        echo -e "Primary: ${GREEN}$primary${NC}"
        echo -e "Replicas: ${GREEN}$replicas${NC}"
        echo ""
        
        echo "PostgreSQL Nodes:"
        for port in 8008 8009 8010; do
            node="patroni$((port-8007))"
            health=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '"\(.role) - \(.state)"' 2>/dev/null || echo "offline")
            echo "  $node: $health"
        done
        echo ""
        
        echo "Ceph Health:"
        docker exec ceph-mon1 ceph health 2>/dev/null || echo "  offline"
        echo ""
        
        sleep 5
    done
}

# Main loop
while true; do
    show_menu
    
    case $choice in
        1) test_cluster_status ;;
        2) test_replication ;;
        3) test_postgres_failover ;;
        4) test_ceph_failure ;;
        5) test_volume_manager_failover ;;
        6) test_full_ha ;;
        7) monitor_cluster ;;
        0) echo "Goodbye!"; exit 0 ;;
        *) echo "Invalid option" ;;
    esac
done
