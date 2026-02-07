# Quick Start Guide - Ceph Storage HA mit PostgreSQL

## 🚀 In 5 Minuten starten

### 1. System hochfahren

```bash
cd control-plane/volume-manager
./setup-ceph-ha.sh
```

**Das dauert ca. 2-3 Minuten.** Das Script startet:

- 3x Ceph Monitors
- 3x Ceph OSDs (Storage)
- 3x PostgreSQL Nodes
- 1x HAProxy (Load Balancer)
- 3x etcd Nodes
- 3x Volume Manager Nodes

### 2. Status prüfen

```bash
# Alle Services sollten "Up" sein
docker-compose -f docker-compose.ceph.yml ps

# Ceph sollte HEALTH_OK oder HEALTH_WARN zeigen
docker exec ceph-mon1 ceph status
```

### 3. Mit PostgreSQL verbinden

**Option A: Interaktives Script (empfohlen)**

```bash
./connect-postgres.sh
```

**Option B: Direkt**

```bash
psql -h localhost -p 5432 -U csf -d csf_core
# Passwort: csfpassword
```

### 4. Failover testen

```bash
./test-ha-failover.sh
```

Wähle Option 8 für alle Tests automatisch.

## 📊 Wichtige Endpoints

| Service       | URL/Command                         | Beschreibung                  |
| ------------- | ----------------------------------- | ----------------------------- |
| PostgreSQL    | `localhost:5432`                    | Haupt-Datenbank (via HAProxy) |
| HAProxy Stats | `http://localhost:7000`             | Load Balancer Dashboard       |
| Ceph Status   | `docker exec ceph-mon1 ceph status` | Storage Cluster Info          |

## 🧪 Failover Demo

### PostgreSQL Node ausschalten

```bash
# Node 1 stoppen
docker-compose -f docker-compose.ceph.yml stop postgres1

# Verbindung testen (funktioniert weiter!)
psql -h localhost -p 5432 -U csf -d csf_core -c "SELECT version();"

# Node wieder starten
docker-compose -f docker-compose.ceph.yml start postgres1
```

### Ceph OSD Failure

```bash
# OSD stoppen
docker-compose -f docker-compose.ceph.yml stop ceph-osd1

# Status prüfen (degraded, aber funktioniert)
docker exec ceph-mon1 ceph status

# OSD wieder starten
docker-compose -f docker-compose.ceph.yml start ceph-osd1
```

## 📝 Häufige Befehle

### Ceph

```bash
# Cluster Health
docker exec ceph-mon1 ceph health

# OSD Status
docker exec ceph-mon1 ceph osd tree

# Pool Info
docker exec ceph-mon1 ceph osd pool ls detail

# RBD Images
docker exec ceph-mon1 rbd ls csf-postgres
```

### PostgreSQL

```bash
# Alle Nodes prüfen
for i in 1 2 3; do
  docker exec postgres${i} pg_isready -U csf -d csf_core
done

# Datenbank-Größe
docker exec postgres1 psql -U csf -d csf_core -c "
  SELECT pg_size_pretty(pg_database_size('csf_core'));"

# Aktive Verbindungen
docker exec postgres1 psql -U csf -d csf_core -c "
  SELECT count(*) FROM pg_stat_activity;"
```

### Volume Manager

```bash
# Logs anschauen
docker logs -f volume-manager-1

# Leader finden
docker logs volume-manager-1 | grep -i leader
docker logs volume-manager-2 | grep -i leader
docker logs volume-manager-3 | grep -i leader
```

## 🛠️ Troubleshooting

### "Connection refused" bei PostgreSQL

```bash
# Prüfe ob Container laufen
docker ps | grep postgres

# Prüfe Logs
docker logs postgres1
docker logs postgres-haproxy

# Starte neu
docker-compose -f docker-compose.ceph.yml restart postgres1
```

### Ceph HEALTH_ERR

```bash
# Details
docker exec ceph-mon1 ceph health detail

# OSDs prüfen (alle sollten "up" und "in" sein)
docker exec ceph-mon1 ceph osd tree

# Neustart falls nötig
docker-compose -f docker-compose.ceph.yml restart ceph-osd1 ceph-osd2 ceph-osd3
```

### Volume Manager startet nicht

```bash
# Logs
docker logs volume-manager-1

# etcd prüfen
docker exec etcd1 etcdctl endpoint health

# Neu bauen und starten
docker-compose -f docker-compose.ceph.yml build volume-manager-1
docker-compose -f docker-compose.ceph.yml up -d volume-manager-1
```

## 🧹 Cleanup

### Services stoppen (Daten behalten)

```bash
docker-compose -f docker-compose.ceph.yml down
```

### Alles löschen (inkl. Daten)

```bash
docker-compose -f docker-compose.ceph.yml down -v
```

### Nur PostgreSQL neu starten

```bash
docker-compose -f docker-compose.ceph.yml restart postgres1 postgres2 postgres3 postgres-haproxy
```

## 📚 Weitere Infos

Siehe [CEPH_HA_README.md](CEPH_HA_README.md) für:

- Detaillierte Architektur
- Performance Tuning
- Produktions-Setup
- Security Hardening
- Monitoring & Backup

## 💡 Tipps

1. **HAProxy Stats** unter http://localhost:7000 zeigt Live-Status
2. **Ceph Dashboard** kann mit `ceph mgr module enable dashboard` aktiviert werden
3. **PostgreSQL Replikation** ist derzeit standalone - für Produktion Streaming Replication aktivieren
4. **Backups** über `docker exec ceph-mon1 rbd snap create csf-postgres/postgres-node-1@backup1`
5. **Monitoring** mit Prometheus/Grafana für Produktion empfohlen

## 🎯 Nächste Schritte

1. ✅ Setup verstanden
2. ✅ Failover erfolgreich getestet
3. → Eigene Daten in PostgreSQL importieren
4. → Monitoring aufsetzen
5. → Backup-Strategie implementieren
6. → Für Produktion härten (Passwörter, TLS, etc.)
