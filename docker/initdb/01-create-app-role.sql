-- ⚠️  RETAINED FOR FRESH-VOLUME CONVENIENCE ONLY.
-- This script is idempotent and mirrors the logic in the authoritative
-- sqlx migration: backend/migrations/20260810000000_create_app_role.sql.
-- Docker only executes initdb scripts when the data directory is empty,
-- so this script NEVER runs on an existing database. The migration is the
-- authoritative source that ensures the role is created in every database.
--
-- P1: Create non-BYPASSRLS application role for RLS enforcement.
--
-- The default `memory` user created by docker-compose is a superuser,
-- which means RLS policies are bypassed (NO-OP). This script creates a
-- restricted application role that RLS policies actually enforce against.
--
-- Usage:
--   - Application connections: use `aetheris_app` (RLS enforced)
--   - Admin/maintenance: use `memory` (superuser, RLS bypassed)
--   - Set DATABASE_URL=postgres://aetheris_app:aetheris_app@localhost:5432/memory
--
-- When deploying to staging/production, replace the default password with
-- a strong secret managed via environment variables.

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'aetheris_app') THEN
        CREATE ROLE aetheris_app WITH LOGIN PASSWORD 'aetheris_app' NOSUPERUSER NOBYPASSRLS;
    END IF;
END
$$;

-- Grant connect and schema access
GRANT CONNECT ON DATABASE memory TO aetheris_app;
GRANT USAGE ON SCHEMA public TO aetheris_app;

-- Grant table-level CRUD (RLS policies will further restrict per-tenant access)
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO aetheris_app;

-- Grant usage on sequences (needed for auto-increment IDs)
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO aetheris_app;

-- Future tables will also be accessible
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO aetheris_app;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE ON SEQUENCES TO aetheris_app;