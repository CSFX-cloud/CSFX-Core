#!/usr/bin/env bash
# Quick Connect Script für PostgreSQL HA

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

show_menu() {
    echo ""
    echo -e "${BLUE}=== PostgreSQL HA Connection Tool ===${NC}"
    echo ""
    echo "1) Connect via HAProxy (localhost:5432)"
    echo "2) Connect to PostgreSQL Node 1"
    echo "3) Connect to PostgreSQL Node 2" 
    echo "4) Connect to PostgreSQL Node 3"
    echo "5) Show HAProxy Stats"
    echo "6) Test all connections"
    echo "7) Show database info"
    echo "8) Exit"
    echo ""
}

connect_haproxy() {
    log_info "Connecting to PostgreSQL via HAProxy..."
    docker exec -it postgres-haproxy nc -zv localhost 5432 && \
    psql -h localhost -p 5432 -U csf -d csf_core
}

connect_node() {
    local node=$1
    log_info "Connecting to PostgreSQL Node ${node}..."
    docker exec -it postgres${node} psql -U csf -d csf_core
}

show_haproxy_stats() {
    log_info "HAProxy Stats available at: http://localhost:7000"
    log_info "Opening in browser..."
    open http://localhost:7000 2>/dev/null || xdg-open http://localhost:7000 2>/dev/null || \
        echo "Please open http://localhost:7000 in your browser"
}

test_all_connections() {
    log_info "Testing all PostgreSQL connections..."
    echo ""
    
    # HAProxy
    if docker exec postgres-haproxy nc -zv localhost 5432 > /dev/null 2>&1; then
        echo -e "HAProxy (localhost:5432): ${GREEN}✓ OK${NC}"
    else
        echo -e "HAProxy (localhost:5432): ${RED}✗ FAILED${NC}"
    fi
    
    # Nodes
    for i in 1 2 3; do
        if docker exec postgres${i} pg_isready -U csf -d csf_core > /dev/null 2>&1; then
            echo -e "PostgreSQL Node ${i}: ${GREEN}✓ OK${NC}"
        else
            echo -e "PostgreSQL Node ${i}: ${YELLOW}⚠ NOT READY${NC}"
        fi
    done
    
    echo ""
    log_info "Connection test complete!"
}

show_db_info() {
    log_info "Fetching database information..."
    echo ""
    
    # Via HAProxy
    echo "=== Database Info (via HAProxy) ==="
    docker exec postgres1 psql -U csf -d csf_core -c "
        SELECT 
            version() as version,
            current_database() as database,
            current_user as user,
            inet_server_addr() as server_ip,
            inet_server_port() as server_port;
    " 2>/dev/null || log_info "Could not fetch info"
    
    echo ""
    echo "=== Database Size ==="
    docker exec postgres1 psql -U csf -d csf_core -c "
        SELECT 
            pg_database.datname, 
            pg_size_pretty(pg_database_size(pg_database.datname)) AS size
        FROM pg_database
        ORDER BY pg_database_size(pg_database.datname) DESC;
    " 2>/dev/null
    
    echo ""
    echo "=== Tables ==="
    docker exec postgres1 psql -U csf -d csf_core -c "\dt" 2>/dev/null || \
        log_info "No tables found (database might be empty)"
    
    echo ""
}

# Main loop
while true; do
    show_menu
    read -p "Select option: " choice
    
    case $choice in
        1) connect_haproxy ;;
        2) connect_node 1 ;;
        3) connect_node 2 ;;
        4) connect_node 3 ;;
        5) show_haproxy_stats ;;
        6) test_all_connections ;;
        7) show_db_info ;;
        8)
            log_info "Goodbye!"
            exit 0
            ;;
        *)
            echo -e "${YELLOW}Invalid option${NC}"
            ;;
    esac
    
    echo ""
    read -p "Press Enter to continue..."
done
