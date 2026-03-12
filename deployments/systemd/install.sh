#!/usr/bin/env bash
set -euo pipefail

CSF_DIR="/opt/csf"

if [[ "$EUID" -ne 0 ]]; then
    echo "run as root"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if ! id csf-updater &>/dev/null; then
    useradd --system --no-create-home --shell /usr/sbin/nologin csf-updater
    usermod -aG docker csf-updater
    echo "created csf-updater system user"
fi

mkdir -p "$CSF_DIR"
chown csf-updater:docker "$CSF_DIR"

cp "${REPO_ROOT}/docker-compose.prod.yml" "${CSF_DIR}/docker-compose.prod.yml"
cp "${SCRIPT_DIR}/csf-updater.sh" "${CSF_DIR}/csf-updater.sh"
chmod 750 "${CSF_DIR}/csf-updater.sh"
chown csf-updater:docker "${CSF_DIR}/csf-updater.sh"

if [[ ! -f "${CSF_DIR}/.env" ]]; then
    cp "${REPO_ROOT}/.env.example" "${CSF_DIR}/.env"
    chmod 640 "${CSF_DIR}/.env"
    chown csf-updater:docker "${CSF_DIR}/.env"
    echo "created ${CSF_DIR}/.env — fill in values before starting"
fi

cp "${SCRIPT_DIR}/csf-updater.service" /etc/systemd/system/csf-updater.service

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
systemctl enable csf-updater
systemctl start csf-updater

echo "csf-updater installed and started"
echo "logs: journalctl -u csf-updater -f"
