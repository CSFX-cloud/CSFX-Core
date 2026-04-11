# CSFX-Core NixOS Deployment Guide

## Voraussetzungen

1. **NixOS auf dem Zielserver installiert** (z.B. mit der ISO aus diesem Projekt)
2. **SSH-Zugriff als root** mit Key-basierter Authentifizierung
3. **Nix mit Flakes** auf deinem Mac installiert

## Schritt 1: SSH-Key einrichten

Falls noch nicht vorhanden, generiere einen SSH-Key:

```bash
ssh-keygen -t ed25519 -C "csfx-deployment@mac"
```

Kopiere den Public Key auf den Server:

```bash
ssh-copy-id root@dein-server
# oder:
cat ~/.ssh/id_ed25519.pub | ssh root@dein-server "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys"
```

## Schritt 2: Konfiguration anpassen

Bearbeite [`modules/server-configuration.nix`](modules/server-configuration.nix):

1. **SSH Public Key eintragen** (Zeile 51):

   ```nix
   users.users.root = {
     openssh.authorizedKeys.keys = [
       "ssh-ed25519 AAAAC3NzaC1... dein-key-hier"
     ];
   };
   ```

2. **Netzwerk-Konfiguration anpassen** (Zeile 21-35):
   - Interface-Name (`eth0` → dein Interface)
   - IP-Adresse (falls statisch gewünscht)
   - Gateway und DNS

3. **Boot-Loader anpassen** (Zeile 8-13):
   - BIOS: `device = "/dev/sda";`
   - UEFI: Kommentare entfernen und anpassen

## Schritt 3: Deployment

### Remote-Deployment von deinem Mac:

```bash
cd /Volumes/CedricExterne/Coding/CSFX-Core/nixos-node

# Deployment auf den Server
nixos-rebuild switch --flake .#csfx-server --target-host root@dein-server --use-remote-sudo
```

**Optionen:**

- `--flake .#csfx-server`: Verwendet die `csfx-server`-Konfiguration aus der flake.nix
- `--target-host root@dein-server`: Deployment-Ziel (ersetze `dein-server` mit IP oder Hostname)
- `--use-remote-sudo`: Verwendet sudo auf dem Remote-Server (falls benötigt)
- `--build-host localhost`: Build lokal auf dem Mac (statt auf dem Server)

### Lokales Build + Deployment (schneller):

```bash
# 1. Build lokal
nix build .#nixosConfigurations.csfx-server.config.system.build.toplevel

# 2. Auf Server kopieren und aktivieren
nixos-rebuild switch --flake .#csfx-server --target-host root@dein-server --build-host localhost
```

## Schritt 4: Verifizierung

Nach dem Deployment, teste die Installation:

```bash
# SSH auf den Server
ssh root@dein-server

# Test-Script ausführen
./test-docker.sh

# Oder manuell testen
systemctl status docker
docker ps
curl http://localhost:8080
```

Von deinem Mac aus:

```bash
# Nginx testen (ersetze IP)
curl http://dein-server:8080
curl http://dein-server:8080/health
```

## Schritt 5: Kontinuierliche Updates

Nach Änderungen an der Konfiguration:

```bash
# Änderungen committen (optional, aber empfohlen)
git add modules/server-configuration.nix
git commit -m "Update server config"

# Deployment
nixos-rebuild switch --flake .#csfx-server --target-host root@dein-server
```

## Troubleshooting

### SSH-Verbindung schlägt fehl

```bash
# Test SSH-Verbindung
ssh -v root@dein-server

# Prüfe authorized_keys auf dem Server
ssh root@dein-server "cat ~/.ssh/authorized_keys"
```

### Build schlägt fehl

```bash
# Lokaler Test-Build
nix build .#nixosConfigurations.csfx-server.config.system.build.toplevel --show-trace
```

### Konfiguration validieren

```bash
# Syntax-Check ohne Deployment
nixos-rebuild dry-build --flake .#csfx-server --target-host root@dein-server
```

### Rollback bei Problemen

```bash
# Auf dem Server: Zur vorherigen Generation zurückkehren
ssh root@dein-server
nixos-rebuild switch --rollback
```

## Alternative: Lokales Deployment

Falls du direkten Zugriff auf den Server hast:

```bash
# 1. Repo auf den Server klonen
ssh root@dein-server
git clone https://github.com/CS-Foundry/CSFX-Core.git /etc/nixos/csfx-core

# 2. Lokal auf dem Server bauen und aktivieren
cd /etc/nixos/csfx-core/nixos-node
nixos-rebuild switch --flake .#csfx-server
```

## Architektur-Wechsel

Für ARM-Server (Raspberry Pi, etc.), ändere in `flake.nix`:

```nix
nixosConfigurations.csfx-server = nixpkgs.lib.nixosSystem {
  system = "aarch64-linux"; # statt "x86_64-linux"
  modules = [ ./modules/server-configuration.nix ];
};
```

Dann deployment:

```bash
nixos-rebuild switch --flake .#csfx-server --target-host root@raspberry-pi --build-host localhost
```
