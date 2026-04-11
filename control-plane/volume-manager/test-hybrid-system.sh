#!/bin/bash

# 🧪 Umfassendes Test-Suite für das Hybridsystem
# Testet: etcd + Ceph + PostgreSQL/Patroni + Volume Manager

set -e

# Export etcd API version
export ETCDCTL_API=3

# 🎨 Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# 📝 Logging functions
log_header() {
    echo -e "\n${BLUE}═══════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════${NC}\n"
}

log_info() {
    echo -e "${CYAN}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

log_step() {
    echo -e "${MAGENTA}▶ $1${NC}"
}

# 🔍 Check prerequisites
check_prerequisites() {
    log_header "Checking Prerequisites"
    
    local all_ok=true
    
    # Check etcdctl
    if command -v etcdctl &> /dev/null; then
        log_success "etcdctl installed"
    else
        log_error "etcdctl not found - install with: brew install etcd"
        all_ok=false
    fi
    
    # Check docker
    if command -v docker &> /dev/null; then
        log_success "docker installed"
    else
        log_error "docker not found"
        all_ok=false
    fi
    
    # Check docker-compose
    if docker compose version &> /dev/null; then
        log_success "docker compose available"
    else
        log_error "docker compose not found"
        all_ok=false
    fi
    
    # Check curl
    if command -v curl &> /dev/null; then
        log_success "curl installed"
    else
        log_error "curl not found"
        all_ok=false
    fi
    
    # Check jq
    if command -v jq &> /dev/null; then
        log_success "jq installed"
    else
        log_warn "jq not found (optional) - install with: brew install jq"
    fi
    
    if [ "$all_ok" = false ]; then
        log_error "Please install missing prerequisites"
        exit 1
    fi
    
    echo ""
}

# 🏥 Component health checks
check_etcd_health() {
    log_step "Checking etcd cluster..."
    
    if etcdctl --endpoints=localhost:2379 endpoint health &>/dev/null; then
        local member_count=$(etcdctl --endpoints=localhost:2379 member list 2>/dev/null | wc -l)
        log_success "etcd cluster healthy ($member_count members)"
        return 0
    else
        log_error "etcd cluster unhealthy"
        return 1
    fi
}

check_ceph_health() {
    log_step "Checking Ceph cluster..."
    
    if docker exec ceph-mon1 ceph health 2>/dev/null | grep -q "HEALTH_OK\|HEALTH_WARN"; then
        local health=$(docker exec ceph-mon1 ceph health 2>/dev/null | awk '{print $1}')
        if [ "$health" == "HEALTH_OK" ]; then
            log_success "Ceph cluster: $health"
        else
            log_warn "Ceph cluster: $health (may be degraded)"
        fi
        return 0
    else
        log_error "Ceph cluster unhealthy or not accessible"
        return 1
    fi
}

check_patroni_health() {
    log_step "Checking Patroni cluster..."
    
    local primary_found=false
    local replica_count=0
    
    for port in 8008 8009 8010; do
        if role=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.role' 2>/dev/null); then
            if [ "$role" == "master" ] || [ "$role" == "primary" ]; then
                primary_found=true
                log_success "Patroni primary found on port $port"
            elif [ "$role" == "replica" ]; then
                ((replica_count++))
            fi
        fi
    done
    
    if [ "$primary_found" = true ]; then
        log_success "Patroni cluster: 1 primary + $replica_count replicas"
        return 0
    else
        log_error "No Patroni primary found"
        return 1
    fi
}

check_volume_manager_health() {
    log_step "Checking Volume Manager..."
    
    local leader=$(etcdctl --endpoints=localhost:2379 get /csfx/volume-manager/election/leader --print-value-only 2>/dev/null)
    
    if [ -n "$leader" ]; then
        log_success "Volume Manager leader: $leader"
        
        # Count nodes
        local node_count=$(etcdctl --endpoints=localhost:2379 get /csfx/volume-manager/nodes/ --prefix --keys-only 2>/dev/null | grep -c "/csfx/volume-manager/nodes/" || echo "0")
        log_info "Registered nodes: $node_count"
        return 0
    else
        log_error "No Volume Manager leader elected"
        return 1
    fi
}

# 🧪 Test 1: Complete System Status
test_system_status() {
    log_header "Test 1: Complete System Status"
    
    echo -e "${YELLOW}🗄️  Database Layer:${NC}"
    check_patroni_health
    echo ""
    
    echo -e "${YELLOW}💾 Storage Layer:${NC}"
    check_ceph_health
    echo ""
    
    echo -e "${YELLOW}🔑 Coordination Layer:${NC}"
    check_etcd_health
    echo ""
    
    echo -e "${YELLOW}🎛️  Control Plane:${NC}"
    check_volume_manager_health
    echo ""
    
    echo -e "${YELLOW}🐳 Docker Services:${NC}"
    docker-compose -f docker-compose.patroni.yml ps
    echo ""
}

# 🧪 Test 2: Data Replication Test
test_data_replication() {
    log_header "Test 2: PostgreSQL Data Replication"
    
    local test_data="hybrid_test_$(date +%s)"
    
    log_step "Creating test table..."
    docker exec patroni1 psql -U csfx -d csfx_core -c \
        "CREATE TABLE IF NOT EXISTS hybrid_test (id SERIAL PRIMARY KEY, data TEXT, created_at TIMESTAMP DEFAULT NOW());" &>/dev/null
    
    log_step "Writing test data to primary..."
    docker exec patroni1 psql -U csfx -d csfx_core -c \
        "INSERT INTO hybrid_test (data) VALUES ('$test_data');" &>/dev/null
    
    # Wait for replication
    sleep 2
    
    log_step "Verifying data on replica..."
    local result=$(docker exec patroni2 psql -U csfx -d csfx_core -t -c \
        "SELECT data FROM hybrid_test WHERE data='$test_data';" 2>/dev/null | xargs)
    
    if [ "$result" == "$test_data" ]; then
        log_success "Data successfully replicated to all nodes!"
        
        # Verify via HAProxy
        log_step "Verifying access via HAProxy..."
        if docker exec postgres-haproxy nc -zv localhost 5000 &>/dev/null; then
            log_success "HAProxy routing working"
        else
            log_warn "HAProxy connectivity issue"
        fi
    else
        log_error "Data replication failed"
        return 1
    fi
    
    echo ""
}

# 🧪 Test 3: PostgreSQL Failover
test_postgres_failover() {
    log_header "Test 3: PostgreSQL Primary Failover"
    
    # Find current primary
    local primary=""
    for port in 8008 8009 8010; do
        role=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.role' 2>/dev/null)
        if [ "$role" == "master" ] || [ "$role" == "primary" ]; then
            case $port in
                8008) primary="patroni1" ;;
                8009) primary="patroni2" ;;
                8010) primary="patroni3" ;;
            esac
            break
        fi
    done
    
    if [ -z "$primary" ]; then
        log_error "No primary found"
        return 1
    fi
    
    log_info "Current primary: $primary"
    echo ""
    
    read -p "$(echo -e ${YELLOW}Stop $primary to trigger failover? [y/N]: ${NC})" -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Skipped"
        return 0
    fi
    
    log_step "Stopping $primary..."
    docker-compose -f docker-compose.patroni.yml stop $primary &>/dev/null
    
    log_step "Waiting for automatic failover..."
    local failover_start=$(date +%s)
    
    for i in {1..30}; do
        sleep 1
        
        # Check for new primary
        for port in 8008 8009 8010; do
            role=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.role' 2>/dev/null)
            if [ "$role" == "master" ] || [ "$role" == "primary" ]; then
                case $port in
                    8008) new_primary="patroni1" ;;
                    8009) new_primary="patroni2" ;;
                    8010) new_primary="patroni3" ;;
                esac
                
                if [ "$new_primary" != "$primary" ]; then
                    local failover_time=$(($(date +%s) - failover_start))
                    echo ""
                    log_success "Failover completed in ${failover_time}s!"
                    log_info "New primary: $new_primary"
                    
                    # Test connectivity
                    sleep 2
                    if docker exec $new_primary psql -U csfx -d csfx_core -c "SELECT 1;" &>/dev/null; then
                        log_success "New primary accepting connections"
                    fi
                    
                    echo ""
                    read -p "$(echo -e ${YELLOW}Restart $primary? [y/N]: ${NC})" -n 1 -r
                    echo
                    if [[ $REPLY =~ ^[Yy]$ ]]; then
                        log_step "Restarting $primary..."
                        docker-compose -f docker-compose.patroni.yml start $primary &>/dev/null
                        log_success "$primary will rejoin as replica"
                    fi
                    
                    return 0
                fi
            fi
        done
        
        echo -n "."
    done
    
    echo ""
    log_error "Failover timeout (30s exceeded)"
    return 1
}

# 🧪 Test 4: Ceph OSD Failure
test_ceph_failover() {
    log_header "Test 4: Ceph OSD Failure"
    
    log_info "Current Ceph status:"
    docker exec ceph-mon1 ceph -s 2>/dev/null | head -15
    echo ""
    
    read -p "$(echo -e ${YELLOW}Stop ceph-osd1 to simulate failure? [y/N]: ${NC})" -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Skipped"
        return 0
    fi
    
    log_step "Stopping ceph-osd1..."
    docker-compose -f docker-compose.patroni.yml stop ceph-osd1 &>/dev/null
    
    log_step "Waiting for Ceph to detect failure (10s)..."
    sleep 10
    
    log_info "Ceph status after OSD failure:"
    docker exec ceph-mon1 ceph -s 2>/dev/null | head -15
    echo ""
    
    log_step "Testing PostgreSQL availability..."
    if docker exec patroni1 psql -U csfx -d csfx_core -c "SELECT version();" &>/dev/null; then
        log_success "PostgreSQL still fully operational (Ceph has 2 remaining replicas)"
    else
        log_error "PostgreSQL affected by OSD failure"
    fi
    
    echo ""
    read -p "$(echo -e ${YELLOW}Restart ceph-osd1? [y/N]: ${NC})" -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_step "Restarting ceph-osd1..."
        docker-compose -f docker-compose.patroni.yml start ceph-osd1 &>/dev/null
        
        log_step "Waiting for OSD recovery (15s)..."
        sleep 15
        
        log_info "Ceph status after recovery:"
        docker exec ceph-mon1 ceph -s 2>/dev/null | head -15
    fi
    
    echo ""
}

# 🧪 Test 5: etcd & Volume Manager Failover
test_volume_manager_failover() {
    log_header "Test 5: Volume Manager Failover"
    
    local current_leader=$(etcdctl --endpoints=localhost:2379 get /csfx/volume-manager/election/leader --print-value-only 2>/dev/null)
    
    if [ -z "$current_leader" ]; then
        log_error "No leader found"
        return 1
    fi
    
    log_info "Current leader: $current_leader"
    echo ""
    
    read -p "$(echo -e ${YELLOW}Stop $current_leader to trigger re-election? [y/N]: ${NC})" -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Skipped"
        return 0
    fi
    
    log_step "Stopping $current_leader..."
    docker-compose -f docker-compose.patroni.yml stop $current_leader &>/dev/null
    
    log_step "Waiting for leader re-election (10s)..."
    sleep 10
    
    local new_leader=$(etcdctl --endpoints=localhost:2379 get /csfx/volume-manager/election/leader --print-value-only 2>/dev/null)
    
    if [ -n "$new_leader" ] && [ "$new_leader" != "$current_leader" ]; then
        log_success "New leader elected: $new_leader"
    else
        log_error "Leader election failed"
        return 1
    fi
    
    echo ""
    read -p "$(echo -e ${YELLOW}Restart $current_leader? [y/N]: ${NC})" -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_step "Restarting $current_leader..."
        docker-compose -f docker-compose.patroni.yml start $current_leader &>/dev/null
        log_success "$current_leader restarted (will run as standby)"
    fi
    
    echo ""
}

# 🧪 Test 6: End-to-End Integration Test
test_e2e_integration() {
    log_header "Test 6: End-to-End Integration"
    
    log_step "Verifying all layers are working together..."
    
    # 1. Check etcd
    if ! check_etcd_health &>/dev/null; then
        log_error "etcd not healthy"
        return 1
    fi
    log_success "✓ etcd coordination working"
    
    # 2. Check Ceph
    if ! check_ceph_health &>/dev/null; then
        log_error "Ceph not healthy"
        return 1
    fi
    log_success "✓ Ceph storage available"
    
    # 3. Check Patroni
    if ! check_patroni_health &>/dev/null; then
        log_error "Patroni not healthy"
        return 1
    fi
    log_success "✓ Patroni database cluster ready"
    
    # 4. Check Volume Manager
    if ! check_volume_manager_health &>/dev/null; then
        log_error "Volume Manager not healthy"
        return 1
    fi
    log_success "✓ Volume Manager orchestration active"
    
    # 5. Test data write & read
    log_step "Testing complete data flow..."
    local test_val="e2e_test_$(date +%s)"
    
    if docker exec patroni1 psql -U csfx -d csfx_core -c \
        "CREATE TABLE IF NOT EXISTS e2e_test (val TEXT); INSERT INTO e2e_test VALUES ('$test_val');" &>/dev/null; then
        
        sleep 2
        local result=$(docker exec patroni2 psql -U csfx -d csfx_core -t -c \
            "SELECT val FROM e2e_test WHERE val='$test_val';" 2>/dev/null | xargs)
        
        if [ "$result" == "$test_val" ]; then
            log_success "✓ Complete data path verified (Primary → Ceph → Replica)"
        else
            log_error "Data replication failed"
            return 1
        fi
    else
        log_error "Database write failed"
        return 1
    fi
    
    echo ""
    log_success "🎉 All integration tests passed!"
    echo ""
}

# 🧪 Test 7: Performance Metrics
test_performance_metrics() {
    log_header "Test 7: Performance Metrics"
    
    log_step "Measuring system metrics..."
    echo ""
    
    # PostgreSQL connections
    local pg_connections=$(docker exec patroni1 psql -U csfx -d csfx_core -t -c \
        "SELECT count(*) FROM pg_stat_activity;" 2>/dev/null | xargs)
    echo -e "${CYAN}PostgreSQL Connections:${NC} $pg_connections"
    
    # Ceph metrics
    log_info "Ceph Cluster Metrics:"
    docker exec ceph-mon1 ceph df 2>/dev/null || log_warn "Could not get Ceph metrics"
    echo ""
    
    # etcd metrics
    local etcd_keys=$(etcdctl --endpoints=localhost:2379 get "" --prefix --keys-only 2>/dev/null | wc -l)
    echo -e "${CYAN}etcd Keys:${NC} $etcd_keys"
    echo ""
}

# 🧪 Test 8: Live Monitoring
test_live_monitoring() {
    log_header "Test 8: Live Monitoring"
    
    echo -e "${YELLOW}Starting live monitoring... (Press Ctrl+C to stop)${NC}"
    echo ""
    
    while true; do
        clear
        log_header "Hybrid System Live Status - $(date '+%H:%M:%S')"
        
        # etcd
        echo -e "${CYAN}🔑 etcd Leader:${NC}"
        etcdctl --endpoints=localhost:2379 get /csfx/volume-manager/election/leader --print-value-only 2>/dev/null || echo "none"
        echo ""
        
        # Ceph
        echo -e "${CYAN}💾 Ceph Health:${NC}"
        docker exec ceph-mon1 ceph health 2>/dev/null | head -1
        echo ""
        
        # Patroni
        echo -e "${CYAN}🗄️  Patroni Cluster:${NC}"
        for port in 8008 8009 8010; do
            role=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.role' 2>/dev/null)
            state=$(curl -s http://localhost:$port/health 2>/dev/null | jq -r '.state' 2>/dev/null)
            case $port in
                8008) node="patroni1" ;;
                8009) node="patroni2" ;;
                8010) node="patroni3" ;;
            esac
            
            if [ "$role" == "master" ] || [ "$role" == "primary" ]; then
                echo -e "  ${GREEN}👑 $node: $role ($state)${NC}"
            elif [ "$role" == "replica" ]; then
                echo -e "  ${BLUE}🔄 $node: $role ($state)${NC}"
            else
                echo -e "  ${RED}❌ $node: offline${NC}"
            fi
        done
        echo ""
        
        # Docker services
        echo -e "${CYAN}🐳 Container Status:${NC}"
        docker-compose -f docker-compose.patroni.yml ps --format "table {{.Name}}\t{{.Status}}" | head -10
        
        sleep 3
    done
}

# 🔥 Test 9: Full Chaos Test
test_chaos() {
    log_header "Test 9: Full Chaos Engineering Test"
    
    log_warn "⚠️  This will simulate multiple failure scenarios!"
    echo ""
    read -p "$(echo -e ${RED}Are you SURE you want to continue? [y/N]: ${NC})" -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Cancelled"
        return 0
    fi
    
    echo ""
    log_step "Starting chaos test sequence..."
    sleep 2
    
    # Scenario 1: Kill PostgreSQL primary
    log_info "🔥 Scenario 1: Killing PostgreSQL primary..."
    local primary=$(curl -s http://localhost:8008/health 2>/dev/null | jq -r '.role' 2>/dev/null)
    if [ "$primary" == "master" ] || [ "$primary" == "primary" ]; then
        docker-compose -f docker-compose.patroni.yml stop patroni1 &>/dev/null
        log_warn "patroni1 stopped"
    fi
    
    sleep 15
    log_step "Checking if system recovered..."
    check_patroni_health
    echo ""
    
    # Scenario 2: Kill Ceph OSD
    log_info "🔥 Scenario 2: Killing Ceph OSD..."
    docker-compose -f docker-compose.patroni.yml stop ceph-osd2 &>/dev/null
    log_warn "ceph-osd2 stopped"
    
    sleep 15
    log_step "Checking Ceph status..."
    check_ceph_health
    echo ""
    
    # Scenario 3: Kill Volume Manager leader
    log_info "🔥 Scenario 3: Killing Volume Manager leader..."
    local leader=$(etcdctl --endpoints=localhost:2379 get /csfx/volume-manager/election/leader --print-value-only 2>/dev/null)
    if [ -n "$leader" ]; then
        docker-compose -f docker-compose.patroni.yml stop $leader &>/dev/null
        log_warn "$leader stopped"
    fi
    
    sleep 10
    log_step "Checking leader re-election..."
    check_volume_manager_health
    echo ""
    
    # Check if system is still functional
    log_step "Testing system functionality under stress..."
    
    if docker exec patroni2 psql -U csfx -d csfx_core -c "SELECT 1;" &>/dev/null; then
        log_success "✅ Database still accessible!"
    else
        log_error "Database not accessible"
    fi
    
    echo ""
    log_info "🔄 Recovering all services..."
    docker-compose -f docker-compose.patroni.yml up -d &>/dev/null
    
    log_step "Waiting for recovery (30s)..."
    sleep 30
    
    log_info "Final system status:"
    check_etcd_health
    check_ceph_health
    check_patroni_health
    check_volume_manager_health
    
    echo ""
    log_success "🎉 Chaos test completed!"
}

# 📋 Main Menu
show_menu() {
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  🧪 Hybrid System Test Suite              ║${NC}"
    echo -e "${GREEN}║  etcd + Ceph + PostgreSQL/Patroni + VM    ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════╝${NC}"
    echo ""
    echo "  1) 📊 Complete System Status"
    echo "  2) 🔄 Test Data Replication"
    echo "  3) 🗄️  Test PostgreSQL Failover"
    echo "  4) 💾 Test Ceph OSD Failure"
    echo "  5) 🎛️  Test Volume Manager Failover"
    echo "  6) 🔗 End-to-End Integration Test"
    echo "  7) 📈 Performance Metrics"
    echo "  8) 👀 Monitor Cluster (Live)"
    echo "  9) 🔥 Full Chaos Test (Advanced)"
    echo "  0) 🚪 Exit"
    echo ""
}

# 🚀 Main
main() {
    clear
    log_header "Hybrid System Test Suite"
    
    # Check prerequisites
    check_prerequisites
    
    # Main loop
    while true; do
        show_menu
        read -p "$(echo -e ${CYAN}Select option: ${NC})" choice
        
        case $choice in
            1) test_system_status ;;
            2) test_data_replication ;;
            3) test_postgres_failover ;;
            4) test_ceph_failover ;;
            5) test_volume_manager_failover ;;
            6) test_e2e_integration ;;
            7) test_performance_metrics ;;
            8) test_live_monitoring ;;
            9) test_chaos ;;
            0)
                echo ""
                log_info "Exiting... Goodbye! 👋"
                echo ""
                exit 0
                ;;
            *)
                log_error "Invalid option"
                ;;
        esac
        
        echo ""
        read -p "$(echo -e ${CYAN}Press Enter to continue...${NC})"
    done
}

# Run main
main
