# Quick Start: API Gateway + Registry

Vereinfachtes Setup nur für API Gateway und Registry Service mit PostgreSQL.

## Services

- **PostgreSQL**: Port 5432
- **API Gateway**: Port 8000 (Haupteingang)
- **Registry Service**: Port 8001 (interner Service)

## Starten

```bash
# Aus dem Root-Verzeichnis
docker-compose -f docker-compose.registry.yml up

# Im Hintergrund
docker-compose -f docker-compose.registry.yml up -d

# Mit rebuild
docker-compose -f docker-compose.registry.yml up --build
```

## Stoppen

```bash
docker-compose -f docker-compose.registry.yml down

# Mit Volume-Cleanup
docker-compose -f docker-compose.registry.yml down -v
```

## Logs anzeigen

```bash
# Alle Services
docker-compose -f docker-compose.registry.yml logs -f

# Nur API Gateway
docker-compose -f docker-compose.registry.yml logs -f api-gateway

# Nur Registry
docker-compose -f docker-compose.registry.yml logs -f registry
```

## Erste Schritte

Nach dem Start:

1. **Migration ausführen** (einmalig):
```bash
docker exec csf-api-gateway-registry cargo run --bin migration
```

2. **Health Checks**:
```bash
# API Gateway
curl http://localhost:8000/health

# Registry via Gateway
curl http://localhost:8000/api/registry/health

# Registry direkt (nur intern/debugging)
curl http://localhost:8001/health
```

3. **Token erstellen**:
```bash
curl -X POST http://localhost:8000/api/registry/admin/tokens \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Test Token",
    "created_by": "admin",
    "ttl_hours": 24
  }'
```

4. **Agent registrieren**:
```bash
curl -X POST http://localhost:8000/api/registry/agents/register \
  -H "Content-Type: application/json" \
  -d '{
    "registration_token": "reg_YOUR_TOKEN_HERE",
    "name": "test-agent",
    "hostname": "localhost",
    "os_type": "linux",
    "os_version": "Ubuntu 22.04",
    "architecture": "x86_64",
    "agent_version": "1.0.0"
  }'
```

## Troubleshooting

### Services neu bauen
```bash
docker-compose -f docker-compose.registry.yml build --no-cache
docker-compose -f docker-compose.registry.yml up
```

### In Container einsteigen
```bash
# API Gateway
docker exec -it csf-api-gateway-registry /bin/sh

# Registry
docker exec -it csf-registry-service /bin/sh

# PostgreSQL
docker exec -it csf-postgres-registry psql -U csf_user -d csf_core
```

### Datenbank zurücksetzen
```bash
docker-compose -f docker-compose.registry.yml down -v
docker-compose -f docker-compose.registry.yml up
```

## Environment Variablen

Optional `.env` Datei im Root:

```env
JWT_SECRET=your_super_secret_jwt_key
RUST_LOG=debug
```

## Ports

| Service      | Port | Zugriff                      |
|--------------|------|------------------------------|
| PostgreSQL   | 5432 | `localhost:5432`             |
| API Gateway  | 8000 | `http://localhost:8000`      |
| Registry     | 8001 | `http://localhost:8001` (intern) |

## Weitere Infos

- [API Gateway Integration](control-plane/api-gateway/REGISTRY_INTEGRATION.md)
- [Registry Service](control-plane/registry/README.md)
