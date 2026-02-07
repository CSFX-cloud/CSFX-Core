# Ceph Module Structure

## Übersicht

Das Ceph-Modul wurde in eine klare, modulare Struktur organisiert, ähnlich wie das etcd-Modul. Dies verbessert die Wartbarkeit und macht den Code übersichtlicher.

## Verzeichnisstruktur

```
src/ceph/
├── mod.rs                 # Haupt-Modul mit Re-Exports
│
├── core/                  # Kern-Komponenten
│   ├── mod.rs            # Core module exports
│   ├── client.rs         # Ceph Client Implementation
│   ├── config.rs         # Konfiguration aus ENV
│   └── error.rs          # Error-Typen (CephError, Result)
│
├── storage/              # Storage Management
│   ├── mod.rs           # Storage module exports
│   ├── types.rs         # Datentypen (CephVolume, CephPool, etc.)
│   ├── pool.rs          # Pool Management (PoolManager)
│   └── rbd.rs           # RBD Volume Operations (RbdManager)
│
└── ops/                  # High-Level Operationen
    ├── mod.rs           # Ops module exports
    └── init.rs          # Initialisierung & Setup
```

## Module

### core/

**Zweck:** Basis-Komponenten für Ceph-Interaktion

- **client.rs** - `CephClient`
  - Führt Ceph-Kommandos aus
  - Health Monitoring
  - Cluster-Verfügbarkeit prüfen

- **config.rs** - `CephConfig`
  - Lädt Konfiguration aus Umgebungsvariablen
  - Monitor Hosts, Keyring, Client Name
  - Pool & Replikations-Einstellungen

- **error.rs** - `CephError`, `Result`
  - Einheitliche Error-Handling
  - Verschiedene Fehlertypen (Command, Parse, Pool, RBD, etc.)

### storage/

**Zweck:** Storage-Verwaltung (Pools & Volumes)

- **types.rs** - Datenstrukturen
  - `CephVolume` - RBD Volume Definition
  - `CephPool` - Pool-Konfiguration
  - `CephClusterHealth` - Cluster Health Status
  - `RbdImage` - RBD Image Info
  - `CephCommand` - Command Builder

- **pool.rs** - `PoolManager`
  - Pool erstellen/löschen
  - Pools auflisten
  - Pool-Existenz prüfen
  - Replikation konfigurieren

- **rbd.rs** - `RbdManager`
  - RBD Images erstellen/löschen
  - Images auflisten
  - Snapshots verwalten
  - Image resize
  - Device mapping (map/unmap)
  - Verschlüsselung

### ops/

**Zweck:** High-Level Operationen & Initialisierung

- **init.rs** - Setup & Initialisierung
  - `init_ceph()` - Initialisiert Ceph-Cluster
  - `create_postgres_volumes()` - Erstellt PostgreSQL Volumes
  - `CephManager` - Zentrale Manager-Struktur

## Verwendung

### Import-Beispiele

**Direkt aus Submodulen:**

```rust
use crate::ceph::core::{CephClient, CephConfig, CephError};
use crate::ceph::storage::{PoolManager, RbdManager};
use crate::ceph::storage::types::{CephVolume, CephPool};
use crate::ceph::ops::{init_ceph, CephManager};
```

**Via Re-Exports (empfohlen):**

```rust
use crate::ceph::{
    CephClient, CephConfig, CephError,
    PoolManager, RbdManager,
    init_ceph, CephManager
};
```

### Code-Beispiel

```rust
// Initialisierung
let ceph_manager = ceph::ops::init_ceph().await?;

// Pool-Operation
let pool = CephPool {
    name: "my-pool".to_string(),
    pg_num: 128,
    pgp_num: 128,
    size: 3,
    min_size: 2,
};
ceph_manager.pool_manager.create_pool(&pool).await?;

// Volume erstellen
let volume = CephVolume {
    name: "my-volume".to_string(),
    pool: "my-pool".to_string(),
    size_mb: 10240,
    features: vec!["layering".to_string()],
    encrypted: false,
};
ceph_manager.rbd_manager.create_image(&volume).await?;
```

## Vergleich mit etcd-Struktur

Beide Module folgen dem gleichen Organisationsprinzip:

```
etcd/                          ceph/
├── core/                     ├── core/
│   ├── client                │   ├── client
│   ├── config                │   ├── config
│   └── error                 │   └── error
├── ha/                       ├── storage/
│   ├── health                │   ├── pool
│   └── leader_election       │   ├── rbd
├── state/                    │   └── types
│   ├── manager               └── ops/
│   ├── storage                   └── init
│   └── types
└── sync/
    ├── lock
    └── watcher
```

## Vorteile der neuen Struktur

1. **Klare Trennung der Verantwortlichkeiten**
   - Core: Basis-Funktionalität
   - Storage: Spezifische Storage-Operationen
   - Ops: High-Level Orchestrierung

2. **Bessere Wartbarkeit**
   - Leichter zu finden, wo Code hingehört
   - Kleinere, fokussierte Dateien
   - Klare Module-Boundaries

3. **Konsistenz mit anderem Code**
   - Gleiche Struktur wie etcd-Modul
   - Einheitliches Muster im ganzen Projekt

4. **Einfachere Tests**
   - Module können einzeln getestet werden
   - Mock-Implementierungen leichter

5. **Bessere IDE-Unterstützung**
   - Auto-Complete funktioniert besser
   - Schnellere Code-Navigation
   - Klarere Import-Pfade

## Migration von altem Code

Falls alter Code noch die alten Pfade verwendet:

**Alt:**

```rust
use crate::ceph::client::CephClient;
use crate::ceph::pool::PoolManager;
use crate::ceph::init::init_ceph;
```

**Neu:**

```rust
use crate::ceph::core::CephClient;
use crate::ceph::storage::PoolManager;
use crate::ceph::ops::init_ceph;
```

Oder einfach:

```rust
use crate::ceph::{CephClient, PoolManager, init_ceph};
```
