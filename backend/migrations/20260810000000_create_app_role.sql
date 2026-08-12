-- P1: Create non-BYPASSRLS application role for RLS enforcement.
--
-- The default `memory` user created by docker-compose is a superuser,
-- which means RLS policies are bypassed (NO-OP). This migration creates a
-- restricted application role that RLS policies actually enforce against.
--
-- This is the AUTHORITATIVE source for the aetheris_app role. The companion
-- file docker/initdb/01-create-app-role.sql is retained only for fresh-volume
-- convenience (docker-entrypoint-initdb.d scripts run only on empty data
-- directories) and carries identical logic.
--
-- Usage:
--   - Application connections: use `aetheris_app` (RLS enforced)
--   - Admin/maintenance: use `memory` (superuser, RLS bypassed)
--   - Set DATABASE_URL=postgres://aetheris_app:<PASSWORD>@localhost:5432/memory
--
-- ⚠️  PASSWORD MUST BE SET OUT OF BAND BEFORE ANY NON-LOCAL DEPLOYMENT:
--     After this migration runs, the role exists WITHOUT a password. No
--     connection can authenticate as aetheris_app until a password is set:
--
--       ALTER ROLE aetheris_app WITH LOGIN PASSWORD '<managed-secret>';
--
--     In production/staging, the password MUST come from a secrets manager
--     (Vault, AWS Secrets Manager, k8s secrets, etc.) and MUST be rotated
--     regularly. Never hardcode a production password in migration files.
--
--     For local development only, if you need a convenience default:
--       ALTER ROLE aetheris_app WITH LOGIN PASSWORD 'aetheris_app';
--     This is intentionally NOT included in this migration to prevent
--     accidental deployment of insecure defaults to production.

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'aetheris_app') THEN
        CREATE ROLE aetheris_app WITH LOGIN NOSUPERUSER NOBYPASSRLS;
    END IF;
END
$$;

-- Grant connect to the current database (works regardless of database name).
-- Must use EXECUTE because GRANT ON DATABASE requires a literal identifier,
-- not a function call.
DO $$
BEGIN
    EXECUTE format('GRANT CONNECT ON DATABASE %I TO aetheris_app', current_database());
END
$$;

-- Grant schema access.
GRANT USAGE ON SCHEMA public TO aetheris_app;

-- Grant table-level CRUD (RLS policies will further restrict per-tenant access).
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO aetheris_app;

-- Grant usage on sequences (needed for auto-increment IDs).
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO aetheris_app;

-- Future tables will also be accessible.
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO aetheris_app;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE ON SEQUENCES TO aetheris_app;