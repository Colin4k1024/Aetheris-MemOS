-- RLS vertical — first verifiable slice: knowledge_entries (LTM) only.
--
-- This migration takes knowledge_entries through backfill -> enforce -> RLS in a
-- single transactional file (sqlx runs it in one tx; every statement below is
-- transaction-safe, so the whole tenant hardening applies atomically or not at
-- all). stm/kg/mm follow the same pattern in later migrations.
--
-- Plan: docs/artifacts/2026-07-16-enterprise-productionization/p1-rls-isolation-plan.md
--   (§2 migration phasing, §3 GUC wiring, §4 LTM query inventory).
-- GUC key: aetheris.tenant_id (transaction-local), read by policies via
--   current_setting('aetheris.tenant_id', true).
--
-- ⚠️ RLS COMPLETENESS PRECONDITION: every code path that reads/writes
-- knowledge_entries must run inside a tenant-scoped transaction (begin_tenant_tx)
-- so the GUC is set, or RLS fail-closes it to zero rows. As of this migration the
-- audited paths are: db/ltm.rs (all methods), services/memory_search.rs
-- keyword_search_for_tenant, services/memory_fusion.rs query_ltm/count_ltm — all
-- routed through begin_tenant_tx. Two known exceptions require a BYPASSRLS/owner
-- admin connection (tracked as follow-up, NOT wired here):
--   1. db/ltm.rs::list_qdrant_tenant_backfill_entries (global cross-tenant scan)
--   2. db/kg.rs::search_knowledge_by_entity_for_tenant (vestigial LEFT JOIN to
--      knowledge_entries; safe today because it selects no ke columns and does not
--      filter on ke, so a fail-closed 0-row ke side leaves the entities result
--      unchanged — but it stops repairing/joining once RLS is on).
--
-- ⚠️ DEPLOYMENT PRECONDITION: RLS (even FORCE) is bypassed by superusers and roles
-- with BYPASSRLS. The application connection MUST be a non-superuser, non-BYPASSRLS
-- role for this policy to take effect. The stock dev image connects as `memory`
-- (owner + superuser) — under it RLS is a NO-OP. See the penetration test, which
-- provisions a dedicated restricted role to prove isolation.

-- ── M1: backfill physical tenant_id from the transitional source_id prefix ──
-- source_id shape: 't:{tenant}:{source}' -> split_part(...,':',2) = tenant.
UPDATE knowledge_entries
SET tenant_id = split_part(source_id, ':', 2)
WHERE tenant_id IS NULL
  AND source_id LIKE 't:%:%';

-- Rows that cannot be attributed to a tenant are registered read-only so they are
-- auditable and remain invisible to every real tenant once RLS is on.
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT
    gen_random_uuid()::text,
    'knowledge_entries',
    entry_id,
    'unattributable_on_backfill',
    jsonb_build_object('source_id', source_id)::text
FROM knowledge_entries
WHERE tenant_id IS NULL;

-- Sentinel tenant for the unattributable rows: it is never equal to any real
-- request GUC, so those rows stay invisible to tenants (only an admin/BYPASSRLS
-- connection can see them for remediation).
UPDATE knowledge_entries
SET tenant_id = '__unattributed__'
WHERE tenant_id IS NULL;

-- ── M2: enforce NOT NULL ──
-- Safe as an inline SET NOT NULL here because M1 above guarantees no NULLs remain
-- and this slice is the small first example. For large production tables use the
-- three-phase pattern from the plan (§2 M2): CHECK NOT VALID -> VALIDATE
-- (-- no-transaction) -> SET NOT NULL, to avoid a long ACCESS EXCLUSIVE lock.
ALTER TABLE knowledge_entries
    ALTER COLUMN tenant_id SET NOT NULL;

-- ── M3: enable Row-Level Security + fail-closed tenant policy ──
ALTER TABLE knowledge_entries ENABLE ROW LEVEL SECURITY;
-- FORCE so the table owner (a non-superuser app role) is also subject to the
-- policy. NOTE: still bypassed by superusers / BYPASSRLS (see deployment note).
ALTER TABLE knowledge_entries FORCE ROW LEVEL SECURITY;

CREATE POLICY knowledge_entries_tenant_isolation ON knowledge_entries
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
