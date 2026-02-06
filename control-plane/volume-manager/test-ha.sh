#!/bin/bash

# Test Script für HA, Leader Election und Failover
# Verwendung: ./test-ha.sh

set -e

# Setze ETCDCTL API Version
export ETCDCTL_API=3

COLOR_RESET='\033[0m'
COLOR_GREEN='\033[0;32m'
COLOR_BLUE='\033[0;34m'
COLOR_YELLOW='\033[1;33m'
COLOR_RED='\033[0;31m'
COLOR_CYAN='\033[0;36m'

log() {
    echo -e "${COLOR_BLUE}[$(date +'%H:%M:%S')]${COLOR_RESET} $1"
}

success() {
    echo -e "${COLOR_GREEN}✅ $1${COLOR_RESET}"
}

info() {
    echo -e "${COLOR_CYAN}ℹ️  $1${COLOR_RESET}"
}

warning() {
    echo -e "${COLOR_YELLOW}⚠️  $1${COLOR_RESET}"
}

error() {
    echo -e "${COLOR_RED}❌ $1${COLOR_RESET}"
}

header() {
    echo -e "\n${COLOR_GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${COLOR_RESET}"
    echo -e "${COLOR_GREEN}  $1${COLOR_RESET}"
    echo -e "${COLOR_GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${COLOR_RESET}\n"
}

# Prüfe ob etcdctl installiert ist
check_etcdctl() {
    if ! command -v etcdctl &> /dev/null; then
        warning "etcdctl nicht gefunden. Installiere mit:"
        echo "  brew install etcd  # macOS"
        echo "  apt install etcd-client  # Ubuntu"
        return 1
    fi
    return 0
}

# Zeige etcd Cluster Status
show_etcd_status() {
    header "etcd Cluster Status"
    if check_etcdctl; then
        etcdctl --endpoints=localhost:2379 member list
        echo ""
        etcdctl --endpoints=localhost:2379 endpoint health
    fi
}

# Zeige alle Nodes in etcd
show_nodes() {
    header "Registrierte Nodes"
    if check_etcdctl; then
        echo "Nodes im Cluster:"
        etcdctl --endpoints=localhost:2379 get /csf/volume-manager/nodes/ --prefix --keys-only | grep -v "^$" || echo "Keine Nodes gefunden"
        echo ""
        echo "Node Details:"
        etcdctl --endpoints=localhost:2379 get /csf/volume-manager/nodes/ --prefix | grep -v "^$" | jq '.' 2>/dev/null || etcdctl --endpoints=localhost:2379 get /csf/volume-manager/nodes/ --prefix
    fi
}

# Zeige aktuellen Leader
show_leader() {
    header "Leader Election Status"
    if check_etcdctl; then
        echo "Aktueller Leader:"
        # Der Leader wird direkt als String unter /csf/volume-manager/election/leader gespeichert
        LEADER=$(etcdctl --endpoints=localhost:2379 get /csf/volume-manager/election/leader --print-value-only 2>/dev/null)
        if [ -n "$LEADER" ]; then
            echo -e "${COLOR_GREEN}👑 $LEADER${COLOR_RESET}"
            
            # Zeige zusätzliche Node-Details
            echo ""
            echo "Node Details:"
            NODE_DATA=$(etcdctl --endpoints=localhost:2379 get /csf/volume-manager/nodes/$LEADER --print-value-only 2>/dev/null)
            if [ -n "$NODE_DATA" ]; then
                echo "$NODE_DATA" | jq '.'
            else
                echo "  Node-Daten nicht verfügbar"
            fi
        else
            echo "Kein Leader gewählt"
        fi
    fi
}

# Zeige Volume States
show_volumes() {
    header "Volume States"
    if check_etcdctl; then
        echo "Volumes im Cluster:"
        etcdctl --endpoints=localhost:2379 get /csf/volume-manager/volumes/ --prefix --keys-only | grep -v "^$" || echo "Keine Volumes gefunden"
        echo ""
        echo "Volume Details:"
        etcdctl --endpoints=localhost:2379 get /csf/volume-manager/volumes/ --prefix | grep -v "^$" | jq '.' 2>/dev/null || echo "Keine Volumes"
    fi
}

# Zeige Container Logs
show_logs() {
    local NODE=$1
    header "Logs von $NODE"
    docker logs --tail 20 $NODE
}

# Stop einen Node (simuliert Failover)
stop_node() {
    local NODE=$1
    header "Stoppe Node: $NODE"
    docker stop $NODE
    success "Node $NODE gestoppt"
    info "Warte 5 Sekunden für Failover..."
    sleep 5
}

# Start einen Node
start_node() {
    local NODE=$1
    header "Starte Node: $NODE"
    docker start $NODE
    success "Node $NODE gestartet"
    info "Warte 5 Sekunden für Initialisierung..."
    sleep 5
}

# Zeige Container Status
show_container_status() {
    header "Docker Container Status"
    docker ps -a --filter "name=volume-manager" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
}

# Überwache Cluster in Echtzeit
monitor() {
    header "Cluster Monitoring (Strg+C zum Beenden)"
    while true; do
        clear
        echo -e "${COLOR_CYAN}═══════════════════════════════════════════════════════════════${COLOR_RESET}"
        echo -e "${COLOR_CYAN}        Volume Manager Cluster - Live Monitor${COLOR_RESET}"
        echo -e "${COLOR_CYAN}═══════════════════════════════════════════════════════════════${COLOR_RESET}"
        echo ""
        
        # Container Status
        echo -e "${COLOR_YELLOW}📦 Container Status:${COLOR_RESET}"
        docker ps --filter "name=volume-manager" --format "  {{.Names}}: {{.Status}}" | sed 's/Up /✅ /' | sed 's/Exited /❌ /'
        echo ""
        
        # Leader
        if check_etcdctl; then
            LEADER=$(etcdctl --endpoints=localhost:2379 get /csf/volume-manager/election/leader --print-value-only 2>/dev/null)
            if [ -n "$LEADER" ]; then
                echo -e "${COLOR_YELLOW}👑 Aktueller Leader:${COLOR_RESET} ${COLOR_GREEN}$LEADER${COLOR_RESET}"
            else
                echo -e "${COLOR_YELLOW}👑 Aktueller Leader:${COLOR_RESET} ${COLOR_RED}Kein Leader${COLOR_RESET}"
            fi
            echo ""
            
            # Nodes
            echo -e "${COLOR_YELLOW}🖥️  Registrierte Nodes:${COLOR_RESET}"
            etcdctl --endpoints=localhost:2379 get /csf/volume-manager/nodes/ --prefix 2>/dev/null | \
                jq -r 'select(.node_id != null) | "  \(.node_id): \(.status) (\(.role))"' 2>/dev/null || echo "  Keine Nodes"
            echo ""
        fi
        
        echo -e "${COLOR_CYAN}───────────────────────────────────────────────────────────────${COLOR_RESET}"
        echo "Aktualisiert: $(date +'%H:%M:%S') | Drücke Strg+C zum Beenden"
        
        sleep 3
    done
}

# Führe Failover-Test durch
test_failover() {
    header "Failover Test starten"
    
    info "1. Zeige initialen Cluster-Status"
    show_container_status
    sleep 2
    
    show_leader
    sleep 2
    
    info "2. Stoppe aktuellen Leader"
    if check_etcdctl; then
        LEADER=$(etcdctl --endpoints=localhost:2379 get /csf/volume-manager/election/leader --print-value-only 2>/dev/null)
        if [ -n "$LEADER" ]; then
            # Leader ID ist der Node-Name, aber Container Name könnte anders sein
            CONTAINER_NAME=$(docker ps --filter "name=$LEADER" --format "{{.Names}}" | head -n1)
            if [ -n "$CONTAINER_NAME" ]; then
                stop_node "$CONTAINER_NAME"
            else
                stop_node "$LEADER"
            fi
        else
            warning "Kein Leader gefunden, stoppe volume-manager-1"
            stop_node "volume-manager-1"
        fi
    else
        stop_node "volume-manager-1"
    fi
    
    info "3. Prüfe neuen Leader"
    show_leader
    sleep 2
    
    show_nodes
    sleep 2
    
    info "4. Starte gestoppten Node wieder"
    if [ -n "$LEADER" ] && [ "$LEADER" != "Kein Leader" ]; then
        start_node "$LEADER"
    else
        start_node "volume-manager-1"
    fi
    
    info "5. Finaler Cluster-Status"
    show_container_status
    sleep 2
    show_leader
    
    success "Failover Test abgeschlossen!"
}

# Hauptmenü
show_menu() {
    echo -e "\n${COLOR_CYAN}═══════════════════════════════════════════════════════════════${COLOR_RESET}"
    echo -e "${COLOR_CYAN}    Volume Manager HA Test Suite${COLOR_RESET}"
    echo -e "${COLOR_CYAN}═══════════════════════════════════════════════════════════════${COLOR_RESET}\n"
    echo "  1) Start Cluster (docker-compose up)"
    echo "  2) Stop Cluster (docker-compose down)"
    echo "  3) Zeige Container Status"
    echo "  4) Zeige etcd Cluster Status"
    echo "  5) Zeige registrierte Nodes"
    echo "  6) Zeige aktuellen Leader"
    echo "  7) Zeige Volumes"
    echo "  8) Zeige Logs (Node auswählen)"
    echo "  9) Stop Node (Failover simulieren)"
    echo " 10) Start Node"
    echo " 11) Failover Test automatisch"
    echo " 12) Live Monitor starten"
    echo " 13) Cleanup etcd Daten"
    echo " 14) Restart Container"
    echo "  0) Beenden"
    echo ""
    echo -n "Wähle eine Option: "
}

# Start Cluster
start_cluster() {
    header "Starte Cluster"
    docker-compose -f docker-compose.test.yml up -d
    success "Cluster gestartet"
    info "Warte 10 Sekunden für Initialisierung..."
    sleep 10
}

# Stop Cluster
stop_cluster() {
    header "Stoppe Cluster"
    docker-compose -f docker-compose.test.yml down
    success "Cluster gestoppt"
}

# Clean etcd data
clean_etcd() {
    header "Cleanup etcd Daten"
    if check_etcdctl; then
        log "Lösche alle Keys unter /csf/volume-manager/..."
        etcdctl --endpoints=localhost:2379 del /csf/volume-manager/ --prefix 2>/dev/null || true
        success "etcd Daten gelöscht"
        
        warning "Bitte starte die Volume Manager Container neu:"
        echo "  docker-compose -f docker-compose.test.yml restart"
    else
        error "etcdctl nicht verfügbar"
    fi
}

# Node auswählen
select_node() {
    echo ""
    echo "Verfügbare Nodes:"
    echo "  1) volume-manager-1"
    echo "  2) volume-manager-2"
    echo "  3) volume-manager-3"
    echo -n "Wähle Node: "
    read NODE_NUM
    case $NODE_NUM in
        1) echo "volume-manager-1" ;;
        2) echo "volume-manager-2" ;;
        3) echo "volume-manager-3" ;;
        *) echo "" ;;
    esac
}

# Hauptprogramm
main() {
    if [ "$1" == "monitor" ]; then
        monitor
        exit 0
    fi
    
    if [ "$1" == "test" ]; then
        test_failover
        exit 0
    fi
    
    while true; do
        show_menu
        read OPTION
        
        case $OPTION in
            1) start_cluster ;;
            2) stop_cluster ;;
            3) show_container_status ;;
            4) show_etcd_status ;;
            5) show_nodes ;;
            6) show_leader ;;
            7) show_volumes ;;
            8)
                NODE=$(select_node)
                if [ -n "$NODE" ]; then
                    show_logs "$NODE"
                fi
                ;;
            9)
                NODE=$(select_node)
                if [ -n "$NODE" ]; then
                    stop_node "$NODE"
                fi
                ;;
            10)
                NODE=$(select_node)
                if [ -n "$NODE" ]; then
                    start_node "$NODE"
                fi
                ;;
            13) clean_etcd ;;
            14)
                header "Restart Container"
                docker-compose -f docker-compose.test.yml restart
                success "Container neu gestartet"
                info "Warte 10 Sekunden..."
                sleep 10
                ;;
            11) test_failover ;;
            12) monitor ;;
            0)
                log "Auf Wiedersehen!"
                exit 0
                ;;
            *)
                error "Ungültige Option"
                ;;
        esac
        
        echo ""
        echo -n "Drücke Enter um fortzufahren..."
        read
    done
}

# Starte
main "$@"
