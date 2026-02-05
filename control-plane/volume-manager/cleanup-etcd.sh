#!/bin/bash

# Cleanup Script - Löscht alte etcd Daten

COLOR_GREEN='\033[0;32m'
COLOR_YELLOW='\033[1;33m'
COLOR_RESET='\033[0m'

echo -e "${COLOR_YELLOW}🧹 Cleaning etcd data...${COLOR_RESET}"

# Lösche alle alten Daten
echo "Deleting all keys under /csf/volume-manager/..."

# Nodes
ETCDCTL_API=3 etcdctl --endpoints=localhost:2379 del /csf/volume-manager/nodes/ --prefix

# Leader
ETCDCTL_API=3 etcdctl --endpoints=localhost:2379 del /csf/volume-manager/election/ --prefix

# Volumes
ETCDCTL_API=3 etcdctl --endpoints=localhost:2379 del /csf/volume-manager/volumes/ --prefix

# Snapshots
ETCDCTL_API=3 etcdctl --endpoints=localhost:2379 del /csf/volume-manager/snapshots/ --prefix

echo -e "${COLOR_GREEN}✅ etcd cleaned!${COLOR_RESET}"
echo ""
echo "Restart volume-manager containers:"
echo "  docker-compose -f docker-compose.test.yml restart"
