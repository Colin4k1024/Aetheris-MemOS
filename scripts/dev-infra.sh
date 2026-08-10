#!/usr/bin/env bash
#
# dev-infra.sh — Aetheris-MemOS local infrastructure bring-up for P1 execution.
#
# Brings up the data services (PostgreSQL+pgvector / Qdrant / Neo4j / Redis) via
# docker compose, waits for health, applies migrations, and regenerates the sqlx
# offline query cache (.sqlx) so `SQLX_OFFLINE=true cargo check` keeps working.
#
# Requirements (this cannot run in an air-gapped sandbox):
#   - Docker daemon running
#   - network access on first run (pulls images: pgvector/pgvector:pg16, qdrant, neo4j:5, redis:7-alpine)
#   - Rust toolchain (for sqlx-cli install + cargo sqlx prepare)
#
# Usage:
#   bash scripts/dev-infra.sh            # bring up + migrate + prepare
#   bash scripts/dev-infra.sh --down     # stop data services (keep volumes)
#   bash scripts/dev-infra.sh --reset    # stop + delete volumes (DESTRUCTIVE)
#
set -euo pipefail
cd "$(dirname "$0")/.."

DB_URL_DEFAULT="postgres://memory:memory@localhost:5432/memory"
SERVICES=(postgres qdrant neo4j redis)

case "${1:-up}" in
  --down)
    echo "==> Stopping data services (volumes kept)…"
    docker compose stop "${SERVICES[@]}"
    exit 0
    ;;
  --reset)
    echo "==> Stopping + removing data services and volumes (DESTRUCTIVE)…"
    docker compose down -v
    exit 0
    ;;
esac

if ! docker info >/dev/null 2>&1; then
  echo "ERROR: Docker daemon is not running. Start Docker Desktop / dockerd first." >&2
  exit 1
fi

echo "==> Starting data services: ${SERVICES[*]} …"
docker compose up -d "${SERVICES[@]}"

echo "==> Waiting for PostgreSQL to accept connections…"
for i in $(seq 1 30); do
  if docker compose exec -T postgres pg_isready -U memory -d memory >/dev/null 2>&1; then
    echo "    postgres ready."
    break
  fi
  sleep 2
  [ "$i" = "30" ] && { echo "ERROR: postgres did not become ready in time." >&2; exit 1; }
done

export DATABASE_URL="${DATABASE_URL:-$DB_URL_DEFAULT}"
echo "==> DATABASE_URL=$DATABASE_URL"

if ! command -v sqlx >/dev/null 2>&1; then
  echo "==> Installing sqlx-cli (rustls, postgres, sqlite)…"
  cargo install sqlx-cli --no-default-features --features rustls,postgres,sqlite
fi

echo "==> Applying migrations (backend/migrations)…"
( cd backend && sqlx migrate run --source migrations )

echo "==> Regenerating offline query cache (backend/.sqlx)…"
( cd backend && cargo sqlx prepare -- --tests ) || {
  echo "WARN: 'cargo sqlx prepare' failed — regenerate manually after fixing queries." >&2
}

cat <<'EOF'

✅ Data infrastructure is ready.

Next steps:
  export APP_JWT_SECRET="$(openssl rand -hex 32)"     # required: auth is on by default
  cd backend && cargo run                              # serves http://127.0.0.1:8008
                                                       # (migrations are applied by this script;
                                                       #  the app only verifies they are up to date)

P1 execution (see docs/artifacts/2026-07-16-enterprise-productionization/p1-execution-runbook.md):
  - Start with PR-1 (tenant_scope executor), then PR-2 (audit), PR-3 (RLS).
  - After changing any sqlx::query! SQL, re-run: (cd backend && cargo sqlx prepare -- --tests)
  - Run PG-backed integration tests (Testcontainers) as the CI gate.

Qdrant:  http://localhost:6333/dashboard   |  gRPC :6334
Neo4j:   http://localhost:7474  (neo4j/password)
EOF
