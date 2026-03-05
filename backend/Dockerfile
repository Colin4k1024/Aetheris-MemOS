# Build stage (context is project root per docker-compose)
FROM rust:1-bookworm AS builder

WORKDIR /app

# Copy dependency manifests and source
COPY backend/Cargo.toml backend/Cargo.lock* ./
COPY backend/src ./src
# rinja.toml has dirs = ["views"]
COPY backend/views ./views
COPY backend/rinja.toml ./
COPY backend/assets ./assets
# SQLx offline mode: use cached query data so build does not need DATABASE_URL
COPY backend/.sqlx ./.sqlx

ENV SQLX_OFFLINE=true

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/backend /usr/local/bin/backend
# Config and static assets for runtime (binary may read config from /app)
COPY backend/docker.toml backend/config.toml* backend/rinja.toml ./
COPY backend/views ./views
COPY backend/assets ./assets
COPY backend/migrations ./migrations
COPY backend/docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh

ENV APP_CONFIG=docker.toml
EXPOSE 8008
ENTRYPOINT ["/docker-entrypoint.sh"]
