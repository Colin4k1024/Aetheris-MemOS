-- RLS vertical — STM slice: context_sessions + session_messages.
--
-- Mirrors the LTM slice (20260716000100_rls_ltm.sql): backfill -> enforce -> RLS
-- in one transactional file (sqlx runs the whole migration in one tx; every
-- statement below is transaction-safe, so tenant hardening applies atomically or
-- not at all). kg/mm follow the same pattern in later 000300/000400 migrations.
--
-- Plan: docs/artifacts/2026-07-16-enterprise-productionization/p1-rls-isolation-plan.md
-- GUC key: aetheris.tenant_id (transaction-local), read by policies via
--   current_setting('aetheris.tenant_id', true). Set by db::tenant_scope::begin_tenant_tx.
--
-- ⚠️ RLS COMPLETENESS PRECONDITION: every code path that reads/writes
-- context_sessions or session_messages must run inside a tenant-scoped
-- transaction (begin_tenant_tx) so the GUC is set, or RLS fail-closes it to zero
-- rows. As of this migration the audited paths are ALL routed through
-- begin_tenant_tx:
--   • db/stm.rs — every STMRepository method (create/get/add_message/
--     get_session_messages/get_recent_sessions/list_sessions/get_active_user_ids/
--     get_active_agent_ids/delete_session). Writes double-write the physical
--     tenant_id column (context_sessions on create, session_messages on
--     add_message) so WITH CHECK holds.
--   • services/memory_fusion.rs — query_stm (session_messages JOIN
--     context_sessions) and count_stm (context_sessions COUNT).
--   • services/memory_ingestion.rs — evict_stm_messages (raw DELETE on
--     session_messages) now takes tenant_id and runs inside begin_tenant_tx;
--     get_active_user_ids is now tenant-scoped (see below).
--   • services/memory_transfer.rs — reaches STM only via the repo methods above.
--   • services/context_compressor.rs / memory_search.rs / memory_storage.rs /
--     routers/{mcp,memory_storage}.rs — all go through STMRepository, so they
--     inherit begin_tenant_tx. NOTE: context_compressor::compress_session still
--     passes get_default_tenant() (pre-existing tenant-plumbing gap): under RLS
--     it fail-closes to 0 rows for any non-`default` tenant's session rather than
--     leaking — tracked as a follow-up, not a bypass.
--   • db/adapters/redis_stm.rs — Redis-only STM adapter, touches no PostgreSQL, so
--     PG RLS does not apply (it enforces isolation in the Redis keyspace instead).
--
-- SIGNATURE CHANGE: STMRepository::get_active_user_ids() previously did a GLOBAL
-- cross-tenant scan (`SELECT DISTINCT user_id ... WHERE status='active'`) with no
-- tenant filter. Under RLS with no GUC that fail-closes to zero rows, silently
-- breaking reflection/transfer. Both callers (memory_ingestion,
-- memory_transfer) already hold a tenant_id, so it now takes `&TenantId` and runs
-- inside begin_tenant_tx. This also closes a latent cross-tenant bleed where the
-- per-tenant reflection cycle iterated every tenant's users. No BYPASSRLS admin
-- exception is required for STM.
--
-- ⚠️ DEPLOYMENT PRECONDITION: RLS (even FORCE) is bypassed by superusers and roles
-- with BYPASSRLS. The application connection MUST be a non-superuser, non-BYPASSRLS
-- role for these policies to take effect. The stock dev image connects as `memory`
-- (owner + superuser) — under it RLS is a NO-OP. See tests/rls_stm_pg.rs, which
-- provisions a dedicated restricted role to prove isolation.

-- ── M1: backfill physical tenant_id ──

-- context_sessions: tenant lives in the transitional user_id prefix
-- 't:{tenant}:{user}' -> split_part(user_id, ':', 2) = tenant.
UPDATE context_sessions
SET tenant_id = split_part(user_id, ':', 2)
WHERE tenant_id IS NULL
  AND user_id LIKE 't:%:%';

-- context_sessions rows that cannot be attributed (no 't:{tenant}:' prefix, e.g.
-- legacy MVP rows where user_id was the bare tenant) are registered read-only so
-- they stay auditable and invisible to every real tenant once RLS is on.
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT
    gen_random_uuid()::text,
    'context_sessions',
    session_id,
    'unattributable_on_backfill',
    jsonb_build_object('user_id', user_id)::text
FROM context_sessions
WHERE tenant_id IS NULL;

-- Sentinel tenant for unattributable sessions: never equal to any real request
-- GUC, so those rows stay invisible to tenants (only admin/BYPASSRLS can see them).
UPDATE context_sessions
SET tenant_id = '__unattributed__'
WHERE tenant_id IS NULL;

-- session_messages carry no user_id — derive tenant from the parent session.
-- Runs AFTER context_sessions backfill (incl. the sentinel), so attributed and
-- sentinel'd sessions both propagate; only true orphans (no parent row) stay NULL.
UPDATE session_messages sm
SET tenant_id = cs.tenant_id
FROM context_sessions cs
WHERE sm.session_id = cs.session_id
  AND sm.tenant_id IS NULL;

-- Orphan messages (session_id with no context_sessions row) are unattributable.
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT
    gen_random_uuid()::text,
    'session_messages',
    message_id,
    'unattributable_on_backfill',
    jsonb_build_object('session_id', session_id)::text
FROM session_messages
WHERE tenant_id IS NULL;

UPDATE session_messages
SET tenant_id = '__unattributed__'
WHERE tenant_id IS NULL;

-- ── M2: enforce NOT NULL ──
-- Safe as an inline SET NOT NULL here because M1 guarantees no NULLs remain. For
-- large production tables use the three-phase pattern (CHECK NOT VALID -> VALIDATE
-- -> SET NOT NULL) to avoid a long ACCESS EXCLUSIVE lock.
ALTER TABLE context_sessions
    ALTER COLUMN tenant_id SET NOT NULL;

ALTER TABLE session_messages
    ALTER COLUMN tenant_id SET NOT NULL;

-- ── M3: enable Row-Level Security + fail-closed tenant policy ──

ALTER TABLE context_sessions ENABLE ROW LEVEL SECURITY;
-- FORCE so the table owner (a non-superuser app role) is also subject to the
-- policy. NOTE: still bypassed by superusers / BYPASSRLS (see deployment note).
ALTER TABLE context_sessions FORCE ROW LEVEL SECURITY;

CREATE POLICY context_sessions_tenant_isolation ON context_sessions
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

ALTER TABLE session_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE session_messages FORCE ROW LEVEL SECURITY;

CREATE POLICY session_messages_tenant_isolation ON session_messages
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
