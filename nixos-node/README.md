# CSFX-Core Docker Test ISO

Diese Konfiguration erstellt ein bootfähiges NixOS ISO-Image mit Docker und Docker Compose für einfache Container-Tests.

## 🚀 ISO bauen

### Mit Flakes (empfohlen)

```bash
cd nixos-node/
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Das ISO-Image wird unter `./result/iso/` erstellt.

## 📦 Was ist enthalten?

- **Docker & Docker Compose** - Container-Management und Orchestrierung
- **Nginx Test Container** - Über docker-compose automatisch gestartet auf Port 8080
- **Test-Script** - Automatische Tests für Docker-Funktionalität

## 🔧 Konfiguration

### Ports

- `8080` - Nginx Test Container

### Automatisch gestartete Services

- Docker Daemon
- Docker Compose mit nginx Container

## 🎯 Verwendung

### 1. ISO auf USB-Stick schreiben

```bash
sudo dd if=result/iso/*.iso of=/dev/sdX bs=4M status=progress
```

### 2. System booten

Boote von dem USB-Stick. Das System startet automatisch als Root und Docker Compose startet den nginx Container.

### 3. Container testen

```bash
# Webseite öffnen
curl http://localhost:8080

# Container-Status prüfen
docker ps -a

# Docker Compose Status
docker-compose ps
```

### 4. Vollständiger Test

```bash
# Führe das bereitgestellte Test-Script aus
./test-docker.sh
```

## 🐳 Docker Tests

### Container verwalten

```bash
# Container stoppen
docker-compose down

# Container neu starten
docker-compose up -d

# Logs ansehen
docker-compose logs
```

### Eigene Container testen

```bash
# Eigenes docker-compose.yml erstellen
cat > test-compose.yml <<EOF
version: '3.8'
services:
  my-app:
    image: hello-world
EOF

# Starten
docker-compose -f test-compose.yml up -d
```

## 📝 Troubleshooting

### Container startet nicht

```bash
# Docker Status prüfen
sudo systemctl status docker

# Docker Compose Logs
cd /etc/docker-test && docker-compose logs
```

### Port 8080 nicht erreichbar

```bash
# Firewall prüfen
sudo nft list ruleset

# Container direkt testen
docker exec -it nginx-test curl localhost
```

## 🔄 Updates

Um das ISO mit Updates zu bauen:

1. Aktualisiere Flake-Inputs:

```bash
nix flake update
```

2. Baue das ISO neu:

```bash
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```
