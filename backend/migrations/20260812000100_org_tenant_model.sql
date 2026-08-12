-- Org-level tenant model (ADR-0009 方案 A, backlog C-3) — schema + backfill.
--
-- Decouples `tenant_id` from `user_id`. Until now `RequestTenantContext::new`
-- set `tenant_id = user_id`, so every authenticated caller was the sole member —
-- and therefore auto-granted Owner — of their own single-user tenant. That made
-- the role check constant-true and left four completed pieces of work (A-1 MCP
-- capability derivation, C-1 admin-route permission gates, P0-6/P0-7 handler
-- identity binding, C-5 role assignment) correct but unobservable.
--
-- This migration introduces the FIRST persistent role storage in the schema.
-- There is no prior table to migrate from: roles lived only in a process-memory
-- HashMap (`services/rbac.rs`), which meant `assign_role` was lost on restart.
--
-- ⚠️ DEPLOYMENT PRECONDITION (same as every RLS slice here): RLS — even FORCE —
-- is bypassed by superusers and BYPASSRLS roles. The stock dev image connects as
-- `memory` (owner + superuser), under which these policies are a NO-OP. The
-- penetration tests provision a dedicated restricted role to prove isolation.
--
-- ⚠️ SEQUENCING: `users.primary_tenant_id` is left NULLABLE here on purpose.
-- Making it NOT NULL now would break `routers/auth.rs::register`, which does not
-- yet create an org for a new user — that wiring lands with the `org` JWT claim
-- (PR-3), and the SET NOT NULL goes with it. The assertion below still guarantees
-- every row that exists *at this point* is backfilled; the open window is only
-- for users registered between this migration and PR-3.

-- ── M1: tables ──

CREATE TABLE tenants (
    tenant_id  TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- `role` values are the lowercase `services::rbac::Role` Display forms. Kept as a
-- CHECK rather than a PG enum so the allowed set can be read back out of this
-- file by an anti-drift test (the pattern established in models/memory_enums.rs)
-- and so widening it later does not need an ALTER TYPE.
CREATE TABLE tenant_members (
    tenant_id   TEXT NOT NULL REFERENCES tenants (tenant_id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'reader')),
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    assigned_by TEXT,
    PRIMARY KEY (tenant_id, user_id)
);

-- Supports the "which orgs does this user belong to" lookup behind switch-org,
-- which filters on user_id alone and so cannot use the (tenant_id, user_id) PK.
CREATE INDEX tenant_members_user_idx ON tenant_members (user_id);

ALTER TABLE users
    ADD COLUMN primary_tenant_id TEXT REFERENCES tenants (tenant_id);

-- ── M2: backfill one personal org per existing user ──
--
-- This is what makes the change backward-compatible. All existing tenant-scoped
-- rows are stored with `tenant_id = user_id`, so giving each user an org whose id
-- IS their user id leaves every row reachable by exactly the same key. A token
-- issued before the `org` claim exists falls back to `org = uid` (PR-3), resolves
-- to this same personal org, and finds the same Owner membership — so old tokens
-- keep working with unchanged behaviour rather than degrading.

INSERT INTO tenants (tenant_id, name)
SELECT u.id, u.username
FROM users u
ON CONFLICT (tenant_id) DO NOTHING;

INSERT INTO tenant_members (tenant_id, user_id, role, assigned_by)
SELECT u.id, u.id, 'owner', 'migration:20260812000100'
FROM users u
ON CONFLICT (tenant_id, user_id) DO NOTHING;

UPDATE users
SET primary_tenant_id = id
WHERE primary_tenant_id IS NULL;

-- Completeness assertion. An incomplete backfill does not fail loudly on its own:
-- the user simply resolves to an org that has no membership row and is denied
-- everything, which reads like a permissions bug rather than a bad migration.
-- Fail the whole transaction instead.
DO $$
DECLARE
    unbackfilled bigint;
    memberless   bigint;
BEGIN
    SELECT count(*) INTO unbackfilled FROM users WHERE primary_tenant_id IS NULL;
    IF unbackfilled > 0 THEN
        RAISE EXCEPTION
            'org backfill incomplete: % user(s) have no primary_tenant_id', unbackfilled;
    END IF;

    SELECT count(*) INTO memberless
    FROM users u
    WHERE NOT EXISTS (
        SELECT 1 FROM tenant_members m
        WHERE m.tenant_id = u.primary_tenant_id AND m.user_id = u.id
    );
    IF memberless > 0 THEN
        RAISE EXCEPTION
            'org backfill incomplete: % user(s) have no membership in their primary org',
            memberless;
    END IF;
END $$;

-- ── M3: Row-Level Security ──
--
-- Both tables get the standard ENABLE + FORCE + fail-closed
-- `current_setting(..., true) IS NOT NULL` shape used by the eight existing RLS
-- tables. What is new here is a SECOND permissive policy per table keyed on a
-- second GUC, `aetheris.user_id`, set by `db::tenant_scope::begin_user_tx`.
--
-- Why a second GUC is necessary: switch-org has to answer "which orgs does this
-- user belong to", which is a query ACROSS tenants for one user. A single
-- tenant-keyed policy makes that unanswerable — you would have to know the org
-- before you could look up which orgs you have. PostgreSQL ORs permissive
-- policies together per command, so the two coexist without weakening each other:
-- with only the tenant GUC set you see that org; with only the user GUC set you
-- see your own memberships; with neither set you see nothing.
--
-- The self-membership policies are deliberately `FOR SELECT` only, i.e. **no
-- WITH CHECK**. A write-capable self policy would let any caller insert
-- themselves into an arbitrary org — self-service privilege escalation. Writes go
-- exclusively through the tenant-keyed policy, which requires already being
-- scoped to that org.

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;

CREATE POLICY tenants_tenant_isolation ON tenants
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

-- Read your own orgs' rows (for names in the switch-org list). The EXISTS reads
-- tenant_members, which is itself RLS'd — the self-membership policy below is
-- what makes that subquery return anything. No recursion: that policy does not
-- reference `tenants`.
CREATE POLICY tenants_own_membership ON tenants
    FOR SELECT
    USING (
        current_setting('aetheris.user_id', true) IS NOT NULL
        AND EXISTS (
            SELECT 1
            FROM tenant_members m
            WHERE m.tenant_id = tenants.tenant_id
              AND m.user_id = current_setting('aetheris.user_id', true)
        )
    );

ALTER TABLE tenant_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenant_members FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_members_tenant_isolation ON tenant_members
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

CREATE POLICY tenant_members_self_membership ON tenant_members
    FOR SELECT
    USING (
        current_setting('aetheris.user_id', true) IS NOT NULL
        AND user_id = current_setting('aetheris.user_id', true)
    );

-- ── Known limitations, recorded rather than silently left ──
--
-- 1. Creating a NEW org at runtime is not possible through these policies: a
--    fresh tenant_id matches no GUC, so the WITH CHECK on tenants_tenant_isolation
--    rejects the insert. That is a deliberate fail-closed default for this slice —
--    org provisioning needs its own authorization story and is not in scope. The
--    backfill above runs before ENABLE ROW LEVEL SECURITY, so it is unaffected.
-- 2. `routers/multi_tenant_router::register_tenant` still writes to the
--    process-memory registry in `services/multi_tenant.rs`, not to this table, so
--    a "registered" tenant has no row here and cannot have members. Pre-existing
--    divergence, out of scope, tracked in docs/memory/backlog.md.
