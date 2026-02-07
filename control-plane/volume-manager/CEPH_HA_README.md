# Ceph Storage HA mit PostgreSQL

## Überblick

Diese Implementierung bietet High Availability (HA) für PostgreSQL-Datenbanken mit Ceph Storage Backend. Alle Cluster- und Management-Daten werden redundant auf einem Ceph-Cluster gespeichert, der automatisches Failover und Datenreplikation bietet.

## Architektur

### Komponenten

1. **Ceph Cluster** (9 Container)
   - 3x Ceph Monitor (MON) - Cluster-Koordination
   - 3x Ceph OSD (Object Storage Daemon) - Datenspeicherung
   - 3x Ceph Manager (MGR) - Cluster-Management

2. **PostgreSQL HA** (3 Container + HAProxy)
   - 3x PostgreSQL 16 Instanzen
   - 1x HAProxy für Load Balancing
   - Automatisches Failover bei Ausfall einer Instanz

3. **etcd Cluster** (3 Container)
   - Distributed State Management
   - Leader Election für Volume Manager

4. **Volume Manager** (3 Container)
   - Ceph RBD Volume Management
   - Automatisches Failover
   - Leader Election via etcd

### Netzwerk-Topologie

```
172.20.0.0/16 CSF Test Network
├── 172.20.0.21-23  Ceph Monitors
├── 172.20.0.31-33  Ceph OSDs
├── 172.20.0.40     PostgreSQL HAProxy
├── 172.20.0.41-43  PostgreSQL Nodes
└── 172.20.0.11-13  Volume Managers
```

## Setup

### Voraussetzungen

- Docker 20.10+
- Docker Compose 2.0+
- Mindestens 8 GB RAM
- 20 GB freier Speicherplatz

### Installation

1. **Setup starten:**

   ```bash
   cd control-plane/volume-manager
   ./setup-ceph-ha.sh
   ```

   Das Script:
   - Startet alle Services (Ceph, PostgreSQL, etcd, Volume Manager)
   - Wartet auf Ceph-Cluster-Initialisierung
   - Erstellt Ceph Pools (csf-volumes, csf-postgres, csf-metadata)
   - Aktiviert RBD-Applikation auf Pools
   - Prüft PostgreSQL-Verfügbarkeit

2. **Status prüfen:**

   ```bash
   # Alle Services
   docker-compose -f docker-compose.ceph.yml ps

   # Ceph Health
   docker exec ceph-mon1 ceph status
   docker exec ceph-mon1 ceph osd tree

   # PostgreSQL
   docker exec postgres1 pg_isready -U csf -d csf_core
   ```

## Verwendung

### PostgreSQL-Verbindung

**Via HAProxy (empfohlen):**

```bash
psql -h localhost -p 5432 -U csf -d csf_core
Password: csfpassword
```

**Direkt zu einer Node:**

```bash
# Node 1
docker exec -it postgres1 psql -U csf -d csf_core

# Node 2
docker exec -it postgres2 psql -U csf -d csf_core

# Node 3
docker exec -it postgres3 psql -U csf -d csf_core
```

### Ceph Storage Management

**Cluster Status:**

```bash
docker exec ceph-mon1 ceph status
docker exec ceph-mon1 ceph health detail
```

**Pools anzeigen:**

```bash
docker exec ceph-mon1 ceph osd pool ls detail
```

**RBD Images (Volumes) anzeigen:**

```bash
docker exec ceph-mon1 rbd ls csf-volumes
docker exec ceph-mon1 rbd ls csf-postgres
docker exec ceph-mon1 rbd info csf-postgres/postgres-node-1
```

**Neues Volume erstellen:**

```bash
docker exec ceph-mon1 rbd create csf-volumes/my-volume --size 5G
```

### HAProxy Stats

Öffne im Browser: http://localhost:8000

Hier siehst du:

- Aktive PostgreSQL-Backends
- Health Check Status
- Connection Statistics

## Failover-Tests

### Interaktive Test-Suite

```bash
./test-ha-failover.sh
```

Das Script bietet:

1. Service-Status prüfen
2. Ceph Health prüfen
3. PostgreSQL-Status prüfen
4. Volume Manager-Status prüfen
5. PostgreSQL Failover testen
6. Ceph OSD Failover testen
7. Volume Manager Failover testen
8. Alle Tests nacheinander ausführen

### Manuelle Failover-Tests

**PostgreSQL Node ausfall simulieren:**

```bash
# Node stoppen
docker-compose -f docker-compose.ceph.yml stop postgres1

# Verbindung testen (sollte weiter funktionieren)
psql -h localhost -p 5432 -U csf -d csf_core -c "SELECT version();"

# Node wieder starten
docker-compose -f docker-compose.ceph.yml start postgres1
```

**Ceph OSD Ausfall:**

```bash
# OSD stoppen
docker-compose -f docker-compose.ceph.yml stop ceph-osd1

# Cluster Status (sollte degraded sein, aber funktionieren)
docker exec ceph-mon1 ceph status

# Warte auf Rebalancing
sleep 30

# OSD wieder starten
docker-compose -f docker-compose.ceph.yml start ceph-osd1

# Warte auf Recovery
docker exec ceph-mon1 ceph -w  # Ctrl+C zum Beenden
```

**Volume Manager Failover:**

```bash
# Leader finden und stoppen
docker-compose -f docker-compose.ceph.yml stop volume-manager-1

# Logs der anderen Nodes prüfen (Leader Election)
docker logs -f volume-manager-2

# Node wieder starten
docker-compose -f docker-compose.ceph.yml start volume-manager-1
```

## Konfiguration

### Ceph Replikation

Standardmäßig werden Daten 3-fach repliziert. Zum Ändern:

```bash
# Replikation auf 2 ändern
docker exec ceph-mon1 ceph osd pool set csf-postgres size 2
docker exec ceph-mon1 ceph osd pool set csf-postgres min_size 1
```

### PostgreSQL in Produktivumgebung

Für Produktion solltest du:

1. **Passwörter ändern** in [docker-compose.ceph.yml](docker-compose.ceph.yml:160):

   ```yaml
   environment:
     - POSTGRES_PASSWORD=SECURE_PASSWORD_HERE
   ```

2. **Streaming Replication aktivieren:**
   - PostgreSQL Primary/Standby Setup
   - pg_basebackup für Initiale Kopie
   - Automatisches Promotion bei Failover

3. **Backup-Strategie:**
   - Ceph RBD Snapshots
   - pg_dump regelmäßig
   - WAL Archivierung

### Volume Manager Konfiguration

In [docker-compose.ceph.yml](docker-compose.ceph.yml:485) anpassen:

```yaml
environment:
  - CEPH_MON_HOSTS=ceph-mon1:6789,ceph-mon2:6789,ceph-mon3:6789
  - CEPH_DEFAULT_POOL=csf-volumes
  - CEPH_PG_NUM=128 # Placement Groups
  - CEPH_REPLICATION=3 # Replikationsfaktor
```

## Troubleshooting

### Ceph Cluster startet nicht

```bash
# Logs prüfen
docker logs ceph-mon1
docker logs ceph-osd1

# Volumes löschen und neu starten
docker-compose -f docker-compose.ceph.yml down -v
./setup-ceph-ha.sh
```

### PostgreSQL kann nicht verbinden

```bash
# Logs prüfen
docker logs postgres1
docker logs postgres-haproxy

# Health Checks
docker exec postgres1 pg_isready -U csf

# HAProxy Config testen
docker exec postgres-haproxy cat /usr/local/etc/haproxy/haproxy.cfg
```

### Volume Manager Fehler

```bash
# Logs
docker logs volume-manager-1

# etcd Status
docker exec etcd1 etcdctl --endpoints=http://etcd1:2379,http://etcd2:2379,http://etcd3:2379 member list

# Leader Election prüfen
docker exec etcd1 etcdctl --endpoints=http://etcd1:2379 get "" --prefix --keys-only | grep leader
```

### Ceph Health WARN/ERR

```bash
# Details anzeigen
docker exec ceph-mon1 ceph health detail

# Häufige Probleme:
# 1. Zu wenig OSDs: Mindestens 3 sollten "up" und "in" sein
docker exec ceph-mon1 ceph osd tree

# 2. Clock Skew: Zeit-Synchronisation prüfen
docker exec ceph-mon1 ceph time-sync-status

# 3. PGs nicht aktiv
docker exec ceph-mon1 ceph pg stat
```

## Performance-Tuning

### Ceph

```bash
# Mehr Placement Groups für große Pools
docker exec ceph-mon1 ceph osd pool set csf-volumes pg_num 256
docker exec ceph-mon1 ceph osd pool set csf-volumes pgp_num 256

# Compression aktivieren
docker exec ceph-mon1 ceph osd pool set csf-volumes compression_mode aggressive
```

### PostgreSQL

Füge zu docker-compose.ceph.yml hinzu:

```yaml
postgres1:
  command:
    - postgres
    - -c
    - max_connections=200
    - -c
    - shared_buffers=256MB
    - -c
    - effective_cache_size=1GB
```

## Cleanup

### Services stoppen

```bash
docker-compose -f docker-compose.ceph.yml down
```

### Alles löschen (inkl. Daten)

```bash
docker-compose -f docker-compose.ceph.yml down -v
```

## Architektur-Diagramm

```
┌─────────────────────────────────────────────────────────┐
│                    CSF-Core HA Stack                     │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  PostgreSQL  │  │  PostgreSQL  │  │  PostgreSQL  │  │
│  │   Node 1     │  │   Node 2     │  │   Node 3     │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                  │                  │           │
│         └──────────┬───────┴──────────────────┘          │
│                    │                                      │
│         ┌──────────▼──────────┐                          │
│         │   HAProxy (5432)    │                          │
│         └─────────────────────┘                          │
│                    │                                      │
├────────────────────┼──────────────────────────────────────┤
│                    │                                      │
│         ┌──────────▼──────────┐                          │
│         │   Ceph RBD Volumes  │                          │
│         └──────────┬──────────┘                          │
│                    │                                      │
│  ┌─────────────────┼─────────────────┐                   │
│  │    Ceph Storage Cluster           │                   │
│  ├───────────────────────────────────┤                   │
│  │  MON1   MON2   MON3  (Monitors)   │                   │
│  │  OSD1   OSD2   OSD3  (Storage)    │                   │
│  └───────────────────────────────────┘                   │
│                    │                                      │
├────────────────────┼──────────────────────────────────────┤
│                    │                                      │
│  ┌─────────────────▼─────────────────┐                   │
│  │    Volume Manager Cluster         │                   │
│  ├───────────────────────────────────┤                   │
│  │  VM1   VM2   VM3  (Leader Elect)  │                   │
│  └──────────┬────────────────────────┘                   │
│             │                                             │
│  ┌──────────▼──────────┐                                 │
│  │   etcd Cluster      │                                 │
│  │  etcd1 etcd2 etcd3  │                                 │
│  └─────────────────────┘                                 │
└─────────────────────────────────────────────────────────┘
```

## Nächste Schritte

1. **Monitoring hinzufügen:**
   - Prometheus Exporters für Ceph
   - PostgreSQL Metrics
   - Grafana Dashboards

2. **Backup-Automation:**
   - Cron-Jobs für RBD Snapshots
   - PostgreSQL WAL Archivierung
   - Automatisches Backup-Testing

3. **Security Hardening:**
   - TLS für PostgreSQL
   - Ceph CephX Authentication
   - Network Policies

4. **Produktions-Deployment:**
   - Kubernetes Manifests
   - Helm Charts
   - Terraform IaC

## Weitere Ressourcen

- [Ceph Dokumentation](https://docs.ceph.com/)
- [PostgreSQL HA Best Practices](https://www.postgresql.org/docs/current/high-availability.html)
- [etcd Operations Guide](https://etcd.io/docs/latest/op-guide/)
