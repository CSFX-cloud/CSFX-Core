# CSF-Core Development Guide

## 🏗️ Multi-Crate Workspace Structure

This project is organized as a Rust workspace with multiple crates:

```
crates/
├── agent/          # Resource monitoring and task execution agent
├── cli/            # Command-line interface tool
├── control-plane/  # Main backend API server
├── entity/         # Database entity definitions (SeaORM)
├── migration/      # Database migrations
└── shared/         # Shared utilities (logger, DB connections)
```

## 🚀 Quick Start

### Prerequisites

- Docker and Docker Compose
- Rust 1.75+ (for local development)
- PostgreSQL 16 (if running locally)

### Development with Docker Compose

1. **Clone and setup:**

   ```bash
   git clone https://github.com/CS-Foundry/CSF-Core.git
   cd CSF-Core
   cp .env.example .env
   ```

2. **Edit `.env` file** with your configuration

3. **Start all services:**

   ```bash
   docker-compose -f docker-compose.dev.yml up
   ```

   This will start:
   - PostgreSQL database (port 5432)
   - Control Plane API (port 8000)
   - Agent (background service)
   - Frontend (port 3000)

4. **Run CLI separately** (optional):
   ```bash
   docker-compose -f docker-compose.dev.yml --profile tools up cli
   ```

### Local Development (without Docker)

1. **Install dependencies:**

   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Install cargo-watch for auto-reload
   cargo install cargo-watch
   ```

2. **Setup PostgreSQL:**

   ```bash
   # Start PostgreSQL (example using Docker)
   docker run -d \
     --name csf-postgres \
     -e POSTGRES_USER=csf_user \
     -e POSTGRES_PASSWORD=csf_password \
     -e POSTGRES_DB=csf_core \
     -p 5432:5432 \
     postgres:16-alpine
   ```

3. **Configure environment:**

   ```bash
   cp .env.example .env
   # Edit .env with your settings
   ```

4. **Run migrations:**

   ```bash
   cargo run --bin control-plane
   # Migrations run automatically on startup
   ```

5. **Run individual services:**

   **Control Plane:**

   ```bash
   cargo watch -x "run --bin control-plane"
   ```

   **Agent:**

   ```bash
   cargo watch -x "run --bin agent"
   ```

   **CLI:**

   ```bash
   cargo run --bin csf
   ```

## 📦 Workspace Crates

### Control Plane

Main backend API server with:

- REST API endpoints
- JWT authentication
- Docker container management
- System monitoring
- Swagger UI documentation at `/swagger-ui`

### Agent

Resource monitoring agent that:

- Reports system metrics
- Executes tasks from control plane
- Manages local containers
- Provides heartbeat monitoring

### CLI

Command-line interface for:

- User management
- Resource configuration
- System administration
- Deployment tools

### Entity

SeaORM entity definitions for all database tables

### Migration

Database migration scripts managed by SeaORM

### Shared

Common utilities:

- Logger initialization (tracing)
- Database connection management
- Shared types and traits

## 🔧 Development Commands

### Build entire workspace:

```bash
cargo build --workspace
```

### Run tests:

```bash
cargo test --workspace
```

### Check code:

```bash
cargo check --workspace
```

### Format code:

```bash
cargo fmt --all
```

### Lint code:

```bash
cargo clippy --workspace -- -D warnings
```

### Run specific binary:

```bash
# Control Plane
cargo run --bin control-plane

# Agent
cargo run --bin agent

# CLI
cargo run --bin csf
```

## 🗄️ Database Management

### Run migrations:

Migrations run automatically when control-plane starts, or manually:

```bash
cd crates/migration
cargo run
```

### Create new migration:

```bash
cd crates/migration
sea-orm-cli migrate generate <migration_name>
```

### Generate entities from database:

```bash
sea-orm-cli generate entity \
  -u postgres://csf_user:csf_password@localhost:5432/csf_core \
  -o crates/entity/src/entities
```

## 📊 API Documentation

Once the control-plane is running, visit:

- Swagger UI: http://localhost:8000/swagger-ui
- OpenAPI JSON: http://localhost:8000/api-docs/openapi.json

## 🐛 Debugging

### View logs:

```bash
# All services
docker-compose -f docker-compose.dev.yml logs -f

# Specific service
docker-compose -f docker-compose.dev.yml logs -f control-plane
```

### Access database:

```bash
docker exec -it csf-postgres-dev psql -U csf_user -d csf_core
```

### Check service status:

```bash
docker-compose -f docker-compose.dev.yml ps
```

## 🔐 Environment Variables

| Variable            | Description                                 | Default                 |
| ------------------- | ------------------------------------------- | ----------------------- |
| `DATABASE_URL`      | PostgreSQL connection string                | Required                |
| `JWT_SECRET`        | Secret for JWT tokens                       | Required                |
| `RUST_LOG`          | Log level (error, warn, info, debug, trace) | `info`                  |
| `RSA_KEY_SIZE`      | RSA key size for encryption                 | `2048`                  |
| `CONTROL_PLANE_URL` | URL to control plane API                    | `http://localhost:8000` |

## 📝 Notes

- **Agent and CLI**: Currently have placeholder implementations with test logs
- **Hot reload**: All services support hot reload in development mode using `cargo-watch`
- **Database**: Automatically creates tables and runs migrations on startup
- **Docker volumes**: Data persists across container restarts

## 🤝 Contributing

1. Create a feature branch
2. Make your changes
3. Run tests and linting
4. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details
