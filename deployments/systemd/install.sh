#!/usr/bin/env bash
set -euo pipefail

CSFX_DIR="/opt/csfxx"

if [[ "$EUID" -ne 0 ]]; then
    echo "run as root"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if ! id csfx-updater &>/dev/null; then
    useradd --system --no-create-home --shell /usr/sbin/nologin csfx-updater
    usermod -aG docker csfx-updater
    echo "created csfx-updater system user"
fi

mkdir -p "$CSFX_DIR"
chown csfx-updater:docker "$CSFX_DIR"

cp "${REPO_ROOT}/docker-compose.prod.yml" "${CSFX_DIR}/docker-compose.prod.yml"
cp "${SCRIPT_DIR}/csfx-updater.sh" "${CSFX_DIR}/csfx-updater.sh"
chmod 750 "${CSFX_DIR}/csfx-updater.sh"
chown csfx-updater:docker "${CSFX_DIR}/csfx-updater.sh"

if [[ ! -f "${CSFX_DIR}/.env" ]]; then
    cp "${REPO_ROOT}/.env.example" "${CSFX_DIR}/.env"
    chmod 640 "${CSFX_DIR}/.env"
    chown csfx-updater:docker "${CSFX_DIR}/.env"
    echo "created ${CSFX_DIR}/.env — fill in values before starting"
fi

cp "${SCRIPT_DIR}/csfx-updater.service" /etc/systemd/system/csfx-updater.service

if command -v ufw &>/dev/null; then
    ufw deny in 2379/tcp comment "etcd - internal only"
    ufw deny in 2380/tcp comment "etcd peer - internal only"
    echo "ufw rules added: etcd ports 2379/2380 blocked from external access"
elif command -v firewall-cmd &>/dev/null; then
    firewall-cmd --permanent --add-rich-rule='rule port port="2379" protocol="tcp" reject'
    firewall-cmd --permanent --add-rich-rule='rule port port="2380" protocol="tcp" reject'
    firewall-cmd --reload
    echo "firewalld rules added: etcd ports 2379/2380 blocked from external access"
fi

systemctl daemon-reload
systemctl enable csfx-updater
systemctl start csfx-updater

echo "csfx-updater installed and started"
echo "logs: journalctl -u csfx-updater -f"
