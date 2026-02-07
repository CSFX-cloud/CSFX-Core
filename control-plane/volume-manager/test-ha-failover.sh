#!/usr/bin/env bash
# Failover-Test-Script für Ceph + PostgreSQL HA

set -euo pipefail

# Farben für Output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Prüfe ob Docker Compose läuft
check_services() {
    log_info "Checking service status..."
    docker-compose -f docker-compose.ceph.yml ps
}

# Ceph Cluster Health
check_ceph_health() {
    log_info "Checking Ceph cluster health..."
    docker exec ceph-mon1 ceph status || log_error "Ceph cluster not healthy"
    docker exec ceph-mon1 ceph osd tree || log_error "Cannot get OSD tree"
}

# Prüfe PostgreSQL Connections
check_postgres() {
    log_info "Checking PostgreSQL connections..."
    
    for i in 1 2 3; do
        if docker exec postgres${i} pg_isready -U csf -d csf_core > /dev/null 2>&1; then
            log_info "PostgreSQL Node ${i}: ${GREEN}READY${NC}"
        else
            log_warn "PostgreSQL Node ${i}: ${RED}NOT READY${NC}"
        fi
    done
    
    # Teste über HAProxy
    if docker exec postgres-haproxy nc -zv localhost 5432 > /dev/null 2>&1; then
        log_info "HAProxy PostgreSQL: ${GREEN}ACCESSIBLE${NC}"
    else
        log_error "HAProxy PostgreSQL: ${RED}NOT ACCESSIBLE${NC}"
    fi
}

# Volume Manager Status
check_volume_managers() {
    log_info "Checking Volume Manager nodes..."
    
    for i in 1 2 3; do
        if docker exec volume-manager-${i} echo "alive" > /dev/null 2>&1; then
            log_info "Volume Manager ${i}: ${GREEN}RUNNING${NC}"
        else
            log_warn "Volume Manager ${i}: ${RED}NOT RUNNING${NC}"
        fi
    done
}

# Simuliere Failover durch Stoppen eines Services
test_postgres_failover() {
    local node=$1
    log_info "Testing PostgreSQL failover by stopping postgres${node}..."
    
    # Status vor Failover
    log_info "Status before failover:"
    check_postgres
    
    # Stoppe Node
    log_warn "Stopping postgres${node}..."
    docker-compose -f docker-compose.ceph.yml stop postgres${node}
    
    # Warte 10 Sekunden
    log_info "Waiting 10 seconds for failover..."
    sleep 10
    
    # Status nach Failover
    log_info "Status after failover:"
    check_postgres
    
    # Teste Verbindung über HAProxy
    log_info "Testing connection through HAProxy..."
    docker exec postgres-haproxy nc -zv postgres2 5432 || log_error "Cannot connect to backup"
    
    # Starte Node wieder
    log_info "Restarting postgres${node}..."
    docker-compose -f docker-compose.ceph.yml start postgres${node}
    
    # Warte auf Recovery
    log_info "Waiting 10 seconds for recovery..."
    sleep 10
    
    # Final Status
    log_info "Final status:"
    check_postgres
}

# Simuliere Ceph OSD Failure
test_ceph_osd_failover() {
    local osd=$1
    log_info "Testing Ceph OSD failover by stopping ceph-osd${osd}..."
    
    # Status vor Failover
    log_info "Status before failover:"
    check_ceph_health
    
    # Stoppe OSD
    log_warn "Stopping ceph-osd${osd}..."
    docker-compose -f docker-compose.ceph.yml stop ceph-osd${osd}
    
    # Warte 15 Sekunden
    log_info "Waiting 15 seconds for OSD failover..."
    sleep 15
    
    # Status nach Failover
    log_info "Status after failover:"
    check_ceph_health
    
    # Starte OSD wieder
    log_info "Restarting ceph-osd${osd}..."
    docker-compose -f docker-compose.ceph.yml start ceph-osd${osd}
    
    # Warte auf Recovery
    log_info "Waiting 20 seconds for OSD recovery..."
    sleep 20
    
    # Final Status
    log_info "Final status:"
    check_ceph_health
}

# Volume Manager Leader Election Test
test_volume_manager_failover() {
    log_info "Testing Volume Manager leader failover..."
    
    # Finde aktuellen Leader
    log_info "Finding current leader..."
    
    # Stoppe Volume Manager 1 (könnte Leader sein)
    log_warn "Stopping volume-manager-1..."
    docker-compose -f docker-compose.ceph.yml stop volume-manager-1
    
    # Warte 10 Sekunden für Leader Election
    log_info "Waiting 10 seconds for leader election..."
    sleep 10
    
    # Status prüfen
    log_info "Checking remaining volume managers..."
    check_volume_managers
    
    # Starte wieder
    log_info "Restarting volume-manager-1..."
    docker-compose -f docker-compose.ceph.yml start volume-manager-1
    
    # Warte
    log_info "Waiting 10 seconds for recovery..."
    sleep 10
    
    # Final Status
    check_volume_managers
}

# Main Menu
show_menu() {
    echo ""
    log_info "=== CSF-Core HA Failover Test Suite ==="
    echo "1) Check all services"
    echo "2) Check Ceph health"
    echo "3) Check PostgreSQL"
    echo "4) Check Volume Managers"
    echo "5) Test PostgreSQL failover (node 1)"
    echo "6) Test Ceph OSD failover (OSD 1)"
    echo "7) Test Volume Manager failover"
    echo "8) Run all failover tests"
    echo "9) Exit"
    echo ""
}

run_all_tests() {
    log_info "Running all failover tests..."
    
    log_info "=== Test 1: PostgreSQL Failover ==="
    test_postgres_failover 1
    sleep 5
    
    log_info "=== Test 2: Ceph OSD Failover ==="
    test_ceph_osd_failover 1
    sleep 5
    
    log_info "=== Test 3: Volume Manager Failover ==="
    test_volume_manager_failover
    
    log_info "All tests completed!"
}

# Main loop
while true; do
    show_menu
    read -p "Select option: " choice
    
    case $choice in
        1) check_services ;;
        2) check_ceph_health ;;
        3) check_postgres ;;
        4) check_volume_managers ;;
        5) test_postgres_failover 1 ;;
        6) test_ceph_osd_failover 1 ;;
        7) test_volume_manager_failover ;;
        8) run_all_tests ;;
        9) 
            log_info "Exiting..."
            exit 0
            ;;
        *)
            log_error "Invalid option"
            ;;
    esac
done
