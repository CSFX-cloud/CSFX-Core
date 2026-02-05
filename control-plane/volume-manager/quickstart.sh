#!/bin/bash

# Quick Start Script für Volume Manager HA Tests
set -e

COLOR_GREEN='\033[0;32m'
COLOR_BLUE='\033[0;34m'
COLOR_YELLOW='\033[1;33m'
COLOR_RED='\033[0;31m'
COLOR_RESET='\033[0m'

log() {
    echo -e "${COLOR_BLUE}▶${COLOR_RESET} $1"
}

success() {
    echo -e "${COLOR_GREEN}✅ $1${COLOR_RESET}"
}

error() {
    echo -e "${COLOR_RED}❌ $1${COLOR_RESET}"
    exit 1
}

warning() {
    echo -e "${COLOR_YELLOW}⚠️  $1${COLOR_RESET}"
}

header() {
    echo ""
    echo -e "${COLOR_GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${COLOR_RESET}"
    echo -e "${COLOR_GREEN}  $1${COLOR_RESET}"
    echo -e "${COLOR_GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${COLOR_RESET}"
    echo ""
}

# Prüfe Dependencies
check_dependencies() {
    header "Prüfe Dependencies"
    
    if ! command -v docker &> /dev/null; then
        error "Docker ist nicht installiert. Bitte installiere Docker: https://docs.docker.com/get-docker/"
    fi
    success "Docker gefunden"
    
    if ! command -v docker-compose &> /dev/null; then
        error "Docker Compose ist nicht installiert."
    fi
    success "Docker Compose gefunden"
    
    if ! command -v cargo &> /dev/null; then
        warning "Cargo nicht gefunden - wird für lokale Entwicklung benötigt"
    else
        success "Cargo gefunden"
    fi
    
    if ! command -v etcdctl &> /dev/null; then
        warning "etcdctl nicht gefunden - installiere für erweiterte Debugging-Möglichkeiten"
        echo "  macOS: brew install etcd"
        echo "  Ubuntu: sudo apt install etcd-client"
    else
        success "etcdctl gefunden"
    fi
}

# Build Docker Images
build_images() {
    header "Baue Docker Images"
    log "Dies kann einige Minuten dauern..."
    
    cd /Volumes/CedricExterne/Coding/CSF-Core
    docker-compose -f control-plane/volume-manager/docker-compose.test.yml build
    
    success "Docker Images gebaut"
}

# Start Cluster
start_cluster() {
    header "Starte HA Cluster"
    log "Starte 3x etcd + 3x volume-manager..."
    
    cd /Volumes/CedricExterne/Coding/CSF-Core
    docker-compose -f control-plane/volume-manager/docker-compose.test.yml up -d
    
    success "Cluster gestartet"
    log "Warte 15 Sekunden für Initialisierung..."
    
    for i in {15..1}; do
        echo -ne "  $i Sekunden...\r"
        sleep 1
    done
    echo ""
}

# Zeige Status
show_status() {
    header "Cluster Status"
    
    log "Docker Container:"
    docker ps --filter "name=volume-manager" --format "  {{.Names}}: {{.Status}}"
    echo ""
    docker ps --filter "name=etcd" --format "  {{.Names}}: {{.Status}}"
    
    if command -v etcdctl &> /dev/null; then
        echo ""
        log "etcd Cluster Health:"
        ETCDCTL_API=3 etcdctl --endpoints=localhost:2379 endpoint health 2>/dev/null | sed 's/^/  /'
        
        echo ""
        log "Aktueller Leader:"
        LEADER=$(ETCDCTL_API=3 etcdctl --endpoints=localhost:2379 get /csf/volume-manager/election/leader --print-value-only 2>/dev/null || echo "Noch kein Leader gewählt")
        echo "  👑 $LEADER"
    fi
}

# Öffne Monitoring
open_monitoring() {
    header "Starte Monitoring"
    log "Öffne interaktives Monitoring..."
    ./test-ha.sh monitor
}

# Main
main() {
    clear
    echo -e "${COLOR_GREEN}"
    cat << "EOF"
    ╔═══════════════════════════════════════════════════════════════╗
    ║                                                               ║
    ║        Volume Manager HA Test - Quick Start                  ║
    ║                                                               ║
    ║        CS-Foundry Control Plane                               ║
    ║                                                               ║
    ╚═══════════════════════════════════════════════════════════════╝
EOF
    echo -e "${COLOR_RESET}"
    
    check_dependencies
    
    echo ""
    log "Was möchtest du tun?"
    echo ""
    echo "  1) Kompletter Start (Build + Start + Monitor)"
    echo "  2) Nur Build"
    echo "  3) Nur Start"
    echo "  4) Test-Suite öffnen"
    echo "  5) Cluster stoppen"
    echo "  0) Abbrechen"
    echo ""
    echo -n "Auswahl: "
    read CHOICE
    
    case $CHOICE in
        1)
            build_images
            start_cluster
            show_status
            echo ""
            log "Drücke Enter um Monitoring zu starten..."
            read
            open_monitoring
            ;;
        2)
            build_images
            success "Build abgeschlossen"
            ;;
        3)
            start_cluster
            show_status
            success "Start abgeschlossen"
            ;;
        4)
            cd /Volumes/CedricExterne/Coding/CSF-Core/control-plane/volume-manager
            ./test-ha.sh
            ;;
        5)
            header "Stoppe Cluster"
            cd /Volumes/CedricExterne/Coding/CSF-Core
            docker-compose -f control-plane/volume-manager/docker-compose.test.yml down
            success "Cluster gestoppt"
            ;;
        0)
            log "Abgebrochen"
            exit 0
            ;;
        *)
            error "Ungültige Auswahl"
            ;;
    esac
}

main
