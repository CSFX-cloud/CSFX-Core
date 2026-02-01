# Multi-stage build for API Gateway

###################
# Chef Stage - Prepare Recipe
###################
FROM rust:latest AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/app

###################
# Planner Stage - Analyze Dependencies
###################
FROM chef AS planner
COPY Cargo.toml ./
COPY migration/Cargo.toml ./migration/
COPY entity/Cargo.toml ./entity/
COPY src ./src
COPY migration/src ./migration/src
COPY entity/src ./entity/src
RUN cargo chef prepare --recipe-path recipe.json

###################
# API Gateway Build Stage
###################
FROM chef AS api-gateway-builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo.toml files for workspace structure
COPY Cargo.toml ./
COPY migration/Cargo.toml ./migration/
COPY entity/Cargo.toml ./entity/

# Create dummy source files for dependency caching
RUN mkdir -p src migration/src entity/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > migration/src/lib.rs && \
    echo "pub fn dummy() {}" > entity/src/lib.rs

# Build dependencies first (cached layer)
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code and build application
COPY src ./src
COPY migration/src ./migration/src
COPY entity/src ./entity/src
# Fix: Update timestamps to force rebuild, otherwise cargo might skip building if host files are older than dummy files
RUN touch src/main.rs migration/src/lib.rs entity/src/lib.rs
RUN cargo build --release --bin api-gateway

###################
# Final Runtime Stage
###################
FROM debian:trixie-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    sqlite3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create data directory for SQLite
RUN mkdir -p /data

# Copy api-gateway binary from builder
COPY --from=api-gateway-builder /usr/src/app/target/release/api-gateway /app/api-gateway

# Expose api-gateway port
EXPOSE 8000

# Set environment variables
ENV DATABASE_URL=sqlite:/data/finance.db
ENV RUST_LOG=info

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8000/api/system/health || exit 1

# Start API Gateway
CMD ["./api-gateway"]