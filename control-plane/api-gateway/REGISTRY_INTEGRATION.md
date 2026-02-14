# API Gateway & Registry Integration

## Architektur

Die CSF Control Plane verwendet eine Microservices-Architektur mit dem API Gateway als zentralem Entry Point:

```
┌─────────────────────────────────────────────────────────────────┐
│                         External Clients                         │
│                    (Agents, Frontend, CLI)                       │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         │ HTTP/HTTPS
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                        API Gateway :8000                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  - Authentication & Authorization                           │ │
│  │  - Request Routing & Proxy                                  │ │
│  │  - Central Entry Point                                      │ │
│  └────────────────────────────────────────────────────────────┘ │
└───────┬──────────────┬──────────────┬──────────────┬───────────┘
        │              │              │              │
        │              │              │              │
        ▼              ▼              ▼              ▼
   ┌────────┐    ┌──────────┐   ┌──────────┐  ┌──────────┐
   │Registry│    │Scheduler │   │ Failover │  │ Volume   │
   │  :8001 │    │          │   │Controller│  │ Manager  │
   └────┬───┘    └──────────┘   └──────────┘  └──────────┘
        │
        │
        ▼
   ┌──────────────┐
   │  PostgreSQL  │
   │    :5432     │
   └──────────────┘
```

## Komponenten

### 1. API Gateway (Port 8000)

- **Rolle**: Zentrale Eingangsschnittstelle für alle externen Anfragen
- **Funktionalität**:
  - Authentifizierung & Autorisierung (RBAC)
  - Request Routing zu Backend-Services
  - Reverse Proxy für Registry Service
  - Service-to-Service Kommunikation

### 2. Registry Service (Port 8001)

- **Rolle**: Agent Registration & Management Backend
- **Funktionalität**:
  - Token-basierte Agent-Registrierung
  - API Key Management für Agents
  - Agent Health Monitoring
  - Datenbank-Persistenz (PostgreSQL)

## API Endpoints

### Registry Endpoints (via API Gateway)

Alle Registry-Anfragen laufen durch das API Gateway unter `/api/registry/*`:

#### Admin Endpoints

```bash
# Token erstellen (Admin only)
POST http://localhost:8000/api/registry/admin/tokens
Content-Type: application/json
Authorization: Bearer <admin-jwt-token>

{
  "description": "Token für Production Agent",
  "created_by": "admin",
  "ttl_hours": 24
}

# Tokens auflisten (Admin only)
GET http://localhost:8000/api/registry/admin/tokens
Authorization: Bearer <admin-jwt-token>

# Agents auflisten (Admin only)
GET http://localhost:8000/api/registry/admin/agents
Authorization: Bearer <admin-jwt-token>

# Statistiken abrufen (Admin only)
GET http://localhost:8000/api/registry/admin/statistics
Authorization: Bearer <admin-jwt-token>
```

#### Agent Endpoints

```bash
# Agent registrieren
POST http://localhost:8000/api/registry/agents/register
Content-Type: application/json

{
  "registration_token": "reg_abc123...",
  "name": "production-node-1",
  "hostname": "prod-node-1.example.com",
  "os_type": "linux",
  "os_version": "Ubuntu 22.04",
  "architecture": "x86_64",
  "agent_version": "1.0.0"
}

# Heartbeat senden
POST http://localhost:8000/api/registry/agents/:id/heartbeat
X-API-Key: csf_agent_xyz789...
Content-Type: application/json

{
  "status": "online"
}
```

#### Health Check

```bash
# Registry Service Health
GET http://localhost:8000/api/registry/health
```

## Interne Service-Kommunikation

Andere Control-Plane Services (Scheduler, Failover Controller, etc.) können das API Gateway für interne Kommunikation nutzen:

```rust
use crate::service_client::ServiceClient;

// Im Service initialisieren
let client = ServiceClient::new();

// Registry Health Check
let is_healthy = client.check_registry_health().await;

// Custom Request forwarding
let (status, response) = client.forward_to_registry(
    reqwest::Method::GET,
    "/admin/agents",
    None,
    Some(headers)
).await?;
```

## Environment Variablen

### API Gateway

```env
DATABASE_URL=postgres://user:pass@localhost:5432/csf_core
REGISTRY_SERVICE_URL=http://localhost:8001
JWT_SECRET=your_secret_key
RSA_KEY_SIZE=2048
RUST_LOG=debug
```

### Registry Service

```env
DATABASE_URL=postgres://user:pass@localhost:5432/csf_core
REGISTRY_PORT=8001
RUST_LOG=debug
```

## Datenbank-Schema

### Tabelle: `registry_tokens`

Speichert einmalige Registrierungs-Tokens für neue Agents.

| Spalte           | Typ       | Beschreibung                   |
| ---------------- | --------- | ------------------------------ |
| id               | UUID      | Primary Key                    |
| token            | String    | Einmaliger Token (reg\_...)    |
| description      | String?   | Optionale Beschreibung         |
| created_by       | String    | Ersteller (Admin)              |
| created_at       | DateTime  | Erstellungszeitpunkt           |
| expires_at       | DateTime  | Ablaufzeitpunkt                |
| used_at          | DateTime? | Verwendungszeitpunkt           |
| used_by_agent_id | UUID?     | Agent, der Token verwendet hat |
| is_used          | Boolean   | Wurde Token bereits verwendet? |

### Tabelle: `agent_api_keys`

Speichert permanente API Keys für authentifizierte Agents.

| Spalte       | Typ       | Beschreibung                 |
| ------------ | --------- | ---------------------------- |
| id           | UUID      | Primary Key                  |
| agent_id     | UUID      | Foreign Key zu agents        |
| api_key      | String    | API Key (csf*agent*...)      |
| created_at   | DateTime  | Erstellungszeitpunkt         |
| last_used_at | DateTime? | Letzter Verwendungszeitpunkt |
| is_active    | Boolean   | Ist Key aktiv?               |

## Deployment

### Development

```bash
# Alle Services starten (inkl. Gateway + Registry)
docker-compose -f docker-compose.dev.yml up

# Nur Gateway + Registry + Datenbank
docker-compose -f docker-compose.dev.yml up postgres api-gateway registry
```

### Migration ausführen

```bash
# Im api-gateway Container
cargo run --bin migration

# Oder über SeaORM CLI
sea-orm-cli migrate up
```

## Vorteile dieser Architektur

1. **Single Entry Point**: Alle externen Anfragen gehen durch ein Gateway
2. **Service Isolation**: Registry kann unabhängig skaliert und deployed werden
3. **Zentrale Authentifizierung**: RBAC & JWT im Gateway
4. **Service Discovery**: Interne Services nutzen Gateway für Kommunikation
5. **Datenbank-Persistenz**: Keine Datenverluste bei Service-Neustarts
6. **Einfache Erweiterbarkeit**: Neue Backend-Services einfach hinzufügen

## Security Features

- ✅ Token-basierte Registrierung (nur mit gültigem Token)
- ✅ API Key Authentication für Agents
- ✅ JWT Authentication für Admin-Endpoints
- ✅ RBAC für feingranulare Berechtigungen
- ✅ Automatische Token-Ablauf-Prüfung
- ✅ Health Checks für Service-Überwachung
- ✅ Zero Trust Architektur

## Troubleshooting

### Registry Service nicht erreichbar

```bash
# Health Check
curl http://localhost:8000/api/registry/health

# Registry direkt testen
curl http://localhost:8001/health

# Logs prüfen
docker logs csf-registry-dev
docker logs csf-api-gateway-dev
```

### Datenbank-Probleme

```bash
# PostgreSQL Status
docker exec csf-postgres-dev pg_isready -U csf_user

# Migrations prüfen
docker exec csf-api-gateway-dev cargo run --bin migration status

# Migration neu ausführen
docker exec csf-api-gateway-dev cargo run --bin migration fresh
```

## Weitere Informationen

- [Registry README](../registry/README.md)
- [API Gateway Documentation](../../docs/api-gateway.md)
- [Agent Integration Guide](../../docs/agent-integration.md)
