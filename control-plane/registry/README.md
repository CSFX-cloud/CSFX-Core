# CSFX Registry Service

Sicherer Agent Registry Service mit Token-basierter Registrierung und API Key Management.

> **⚠️ WICHTIG**: Der Registry Service ist ein interner Backend-Service. Alle externen Anfragen müssen durch das **API Gateway** (Port 8000) geleitet werden. Siehe [API Gateway Integration](../api-gateway/REGISTRY_INTEGRATION.md) für Details.

## Features

- 🔐 **Token-basierte Registrierung**: Admins erstellen einmalige Registrierungs-Tokens
- 🔑 **API Key Management**: Permanente API Keys für authentifizierte Agent-Kommunikation
- 💓 **Agent Heartbeat**: Überwachung des Agent-Status
- 📊 **Statistiken**: Echtzeit-Übersicht über alle registrierten Agents
- 🧹 **Auto-Cleanup**: Automatisches Bereinigen abgelaufener Tokens und inaktiver Agents
- 💾 **Datenbank-Persistenz**: PostgreSQL für sichere Datenspeicherung

## Architektur

```
┌──────────────┐           ┌─────────────────┐           ┌──────────────┐
│   External   │──────────▶│   API Gateway   │──────────▶│   Registry   │
│   Clients    │  :8000    │  (Reverse Proxy)│  :8001    │   Service    │
│ (Agents/CLI) │           │                 │           │  (Backend)   │
└──────────────┘           └─────────────────┘           └──────┬───────┘
                                                                 │
                                                                 ▼
                                                          ┌──────────────┐
                                                          │  PostgreSQL  │
                                                          │   Database   │
                                                          └──────────────┘

Flow:
     1. Admin erstellt Token über Gateway → Registry
     2. Agent registriert sich über Gateway → Registry (in DB gespeichert)
     3. Agent erhält API Key (in DB gespeichert)
     4. Agent sendet Heartbeats über Gateway → Registry
```

## API Endpoints

**ALLE Anfragen laufen über das API Gateway:**

### Via API Gateway (empfohlen)

```bash
# Admin: Token erstellen
POST http://localhost:8000/api/registry/admin/tokens

# Agent: Registrieren
POST http://localhost:8000/api/registry/agents/register

# Agent: Heartbeat
POST http://localhost:8000/api/registry/agents/:id/heartbeat

# Admin: Agents auflisten
GET http://localhost:8000/api/registry/admin/agents
```

### Direkt (nur für Entwicklung/Debugging)

```bash
# Nur lokal verfügbar, nicht extern exponiert
POST http://localhost:8001/admin/tokens
POST http://localhost:8001/agents/register
POST http://localhost:8001/agents/:id/heartbeat
```

## Schnellstart

### 1. System starten

```bash
# Gesamtes System (Gateway + Registry + DB)
docker-compose -f docker-compose.dev.yml up

# Oder nur notwendige Services
docker-compose -f docker-compose.dev.yml up postgres api-gateway registry
```

Die Services laufen auf:

- API Gateway: `http://localhost:8000`
- Registry (intern): `http://localhost:8001`

### 2. Admin: Token erstellen

**Alle Anfragen gehen durch das API Gateway:**

```bash
curl -X POST http://localhost:8000/api/registry/admin/tokens \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Token für Production Agent",
    "created_by": "admin",
    "ttl_hours": 24
  }'
```

Response:

```json
{
  "token_id": "550e8400-e29b-41d4-a716-446655440000",
  "token": "reg_abc123...",
  "expires_at": "2024-01-15T12:00:00Z"
}
```

### 3. Agent: Registrierung

```bash
curl -X POST http://localhost:8000/api/registry/agents/register \
  -H "Content-Type: application/json" \
  -d '{
    "registration_token": "reg_abc123...",
    "name": "production-node-1",
    "hostname": "prod-node-1.example.com",
    "os_type": "linux",
    "os_version": "Ubuntu 22.04",
    "architecture": "x86_64",
    "agent_version": "1.0.0"
  }'
```

Response:

```json
{
  "agent_id": "660e8400-e29b-41d4-a716-446655440000",
  "api_key": "csfx_agent_xyz789...",
  "message": "Agent successfully registered"
}
```

### 4. Agent: Heartbeat senden

```bash
curl -X POST http://localhost:8000/api/registry/agents/660e8400-e29b-41d4-a716-446655440000/heartbeat \
  -H "Content-Type: application/json" \
  -H "X-API-Key: csfx_agent_xyz789..." \
  -d '{
    "status": "online"
  }'
```

## API Endpoints

### Admin Endpoints

| Methode | Endpoint            | Beschreibung                      |
| ------- | ------------------- | --------------------------------- |
| POST    | `/admin/tokens`     | Erstelle neuen Registration Token |
| GET     | `/admin/tokens`     | Liste alle Tokens                 |
| GET     | `/admin/agents`     | Liste alle Agents                 |
| GET     | `/admin/agents/:id` | Agent Details                     |
| POST    | `/admin/agents/:id` | Agent deregistrieren              |
| GET     | `/admin/statistics` | Agent Statistiken                 |

### Agent Endpoints

| Methode | Endpoint                | Beschreibung                        |
| ------- | ----------------------- | ----------------------------------- |
| POST    | `/agents/register`      | Registriere neuen Agent (mit Token) |
| POST    | `/agents/:id/heartbeat` | Sende Heartbeat (mit API Key)       |

### Health Check

| Methode | Endpoint  | Beschreibung         |
| ------- | --------- | -------------------- |
| GET     | `/health` | Service Health Check |

## Entwicklung

### Dependencies aktualisieren

```bash
cargo update -p registry
```

### Tests ausführen

```bash
cargo test -p registry
```

### Mit Watch Mode

```bash
cargo watch -x 'run -p registry'
```

## Konfiguration

Environment Variablen (`.env`):

```bash
# Server Port
REGISTRY_PORT=8001

# Logging Level
RUST_LOG=debug

# Agent Health Check Timeout (Sekunden)
AGENT_TIMEOUT=300

# Token Cleanup Interval (Sekunden)
TOKEN_CLEANUP_INTERVAL=3600
```

## Sicherheit

### Token Security

- ✅ Einmalige Verwendung pro Token
- ✅ Zeitlich begrenzte Gültigkeit (TTL)
- ✅ Automatisches Cleanup abgelaufener Tokens

### API Key Security

- ✅ Unique pro Agent
- ✅ Kann revoked/rotated werden
- ✅ Validierung bei jedem Request

### Best Practices

1. **Kurze Token TTL**: Verwende kurze TTLs (z.B. 1-24h) für Registration Tokens
2. **Token Rotation**: Lösche alte/ungenutzte Tokens regelmäßig
3. **API Key Rotation**: Implementiere regelmäßige API Key Rotation
4. **Monitoring**: Überwache failed authentication attempts
5. **TLS**: Verwende HTTPS in Production

## Troubleshooting

### Registry startet nicht

```bash
# Check logs
docker-compose -f docker-compose.dev.yml logs registry

# Check port conflicts
lsof -i :8001
```

### Agent kann sich nicht registrieren

1. **Token abgelaufen?**

   ```bash
   curl http://localhost:8001/admin/tokens
   ```

2. **Token bereits verwendet?**
   - Ein Token kann nur einmal verwendet werden
   - Erstelle einen neuen Token

### Heartbeat fehlgeschlagen

1. **API Key korrekt?**
   - Verwende den API Key aus der Registrierungs-Response
   - Format: `X-API-Key: csfx_agent_...`

2. **Agent ID korrekt?**
   - URL muss die korrekte Agent ID enthalten

## Integration mit Agent

Siehe [agent/README.md](../../agent/README.md) für Details zur Agent-Integration.

## Produktion

Für Production Deployment:

1. Verwende Production Dockerfile (ohne cargo-watch)
2. Enable TLS/HTTPS
3. Implementiere Admin Authentication
4. Setup Logging/Monitoring
5. Database Persistence (aktuell: In-Memory)

## License

See [LICENSE](../../LICENSE)
