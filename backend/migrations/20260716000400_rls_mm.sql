-- RLS vertical — MM slice: multimodal_entries + modality_relations.
--
-- Fourth RLS slice (after LTM 20260716000100, STM 20260716000200, KG
-- 20260716000300). Takes both MM tables through backfill -> enforce -> RLS in one
-- transactional file (sqlx runs it in a single tx; every statement below is
-- transaction-safe, so the whole tenant hardening applies atomically or not at all).
--
-- Plan: docs/artifacts/2026-07-16-enterprise-productionization/p1-rls-isolation-plan.md
--   and ADR-0001. GUC key: aetheris.tenant_id (transaction-local), read by policies
--   via current_setting('aetheris.tenant_id', true) — same as the LTM/STM/KG slices.
--
-- TENANT SOURCE OF TRUTH (differs from LTM/STM/KG — read carefully)
--   Unlike KG (entity_id 't:{tenant}:{ulid}' prefix) or LTM (id prefix), MM historically
--   stored its tenant inside the content_metadata JSON document, NOT the id. The M0
--   foundation (20260706000100) added a physical tenant_id column; db/mm.rs now
--   DUAL-WRITES it (physical column + content_metadata JSON tenant + a transitional
--   't:{tenant}:' source_id prefix). This migration backfills the physical column so the
--   RLS policy — which can only key off a real column, not a JSON field — has an
--   authoritative value on every legacy row.
--     multimodal_entries: tenant derived, in priority order, from
--       (1) content_metadata JSON  ->> 'tenant_id'  (the authoritative pre-RLS source),
--       (2) the transitional source_id 't:{tenant}:...' prefix, else sentinel (below).
--     modality_relations: relations carry NO tenant of their own. Both source_entry_id
--       and target_entry_id are FKs to multimodal_entries(entry_id) (ON DELETE CASCADE),
--       so a relation is backfilled by inheriting its source entry's tenant_id (target
--       entry as a defensive fallback). New rows get the physical tenant_id written by
--       db/mm.rs::create_relation (which takes a tenant_id parameter and validates both
--       endpoints belong to the caller's tenant before inserting).
--
-- ⚠️ JSON-vs-PHYSICAL-COLUMN CONSISTENCY (the headline risk for this slice)
--   Because the tenant lived in JSON and now also lives in a column, the two must never
--   disagree. This migration keeps them consistent by construction:
--     • create_entry writes BOTH from the same argument (they cannot diverge at write).
--     • This backfill derives the physical column as a SUPERSET of the JSON predicate:
--       it takes the JSON tenant first, so no row that the app-layer JSON filter would
--       show is ever hidden by (or attributed differently in) the physical column.
--     • update_entry self-heals: `tenant_id = COALESCE(tenant_id, $n)` never overwrites
--       an attributed value, only fills a NULL, so RLS and JSON stay aligned over time.
--   content_metadata is free-form TEXT: a malformed value would abort a plain
--   `content_metadata::jsonb` cast and, with it, the whole migration. M1 extracts the
--   JSON tenant per-row inside a subtransaction (DO block) so one bad row falls through
--   to the source_id / sentinel fallback instead of failing the deploy.
--
-- ⚠️ RLS COMPLETENESS PRECONDITION: every code path that reads/writes multimodal_entries
-- OR modality_relations must run inside a tenant-scoped transaction (begin_tenant_tx) so
-- the GUC is set, or RLS fail-closes it to zero rows / a NOT NULL (WITH CHECK) violation.
-- Audited PG-direct access points as of this migration (grep of src/ for
-- FROM/JOIN/INTO/UPDATE on multimodal_entries|modality_relations):
--   • db/mm.rs — ALL MMRepository read/write methods route through the tenant tx via the
--     begin_optional_tenant_tx helper (a concrete tenant => begin_tenant_tx enforced at
--     the DB layer; None => admin/plain-pool path, fail-closed under a restricted role)
--     and double-write the physical tenant_id column:
--       create_entry, get_entry_by_id, update_entry, get_entries_by_session,
--       get_entries_by_modality, create_relation, get_related_entries, list_entries.
--   • db/mm.rs::count(&PgPool) — ⚠️ ADMIN/UNSCOPED EXCEPTION. A deliberate global,
--     cross-tenant `SELECT COUNT(*) FROM multimodal_entries` with no WHERE and no tenant
--     tx. It takes an explicit pool and has NO production callers today. Once RLS is
--     enforced it returns only rows visible to the connection — zero under a restricted
--     app role, the true global total only under an owner/BYPASSRLS connection. Per-tenant
--     counts must use list_entries or memory_fusion::count_mm (both RLS-scoped).
--   • services/memory_fusion.rs — query_mm + count_mm route through begin_tenant_tx.
--   • services/multimodal_memory.rs (MultimodalMemoryService) — now threads its tenant_id
--     argument through to MMRepository (previously hardcoded None). It has no callers in
--     src/ today, but is RLS-ready rather than a latent fail-close if it is ever wired up.
-- Every other reference (routers/multimodal, routers/mcp, routers/data_io) calls the
-- MMRepository methods above with a concrete tenant and therefore inherits RLS
-- transitively — none issues raw SQL on these tables.
--
-- ⚠️ FK vs RLS: PostgreSQL referential-integrity checks (source_entry_id/target_entry_id
-- -> multimodal_entries(entry_id)) ALWAYS bypass row security. This is intentional and
-- safe here: create_relation validates both endpoints belong to the caller's tenant via
-- RLS-scoped get_entry_by_id reads before inserting, and WITH CHECK still pins the
-- relation's own tenant_id to the GUC. The FK bypass only means the existence check of a
-- referenced entry_id is not itself tenant-filtered.
--
-- ⚠️ DEPLOYMENT PRECONDITION: RLS (even FORCE) is bypassed by superusers and roles with
-- BYPASSRLS. The application connection MUST be a non-superuser, non-BYPASSRLS role for
-- these policies to take effect. The stock dev image connects as `memory` (owner +
-- superuser) — under it RLS is a NO-OP and the app-layer content_metadata JSON filter is
-- the effective guard. See tests/rls_mm_pg.rs, which provisions a dedicated restricted
-- role (aetheris_rls_probe) to prove isolation.
--
-- ⚠️ IDEMPOTENCY: this file was applied once, then manually rolled back (RLS disabled on
-- both MM tables) while mm.rs coverage was completed. To make re-enabling safe regardless
-- of leftover schema state, every statement below is re-runnable: backfill UPDATEs are
-- guarded by `tenant_id IS NULL`, the readonly_isolation INSERTs by the same predicate,
-- ENABLE/FORCE ROW LEVEL SECURITY are no-ops if already set, and each policy is dropped
-- with DROP POLICY IF EXISTS before CREATE POLICY (a bare CREATE POLICY errors with
-- 42710 "policy already exists" against a table that still has it — the failure observed
-- when this file was previously re-run against a partially-migrated DB).

-- ── M1: backfill physical tenant_id ──

-- multimodal_entries (1): authoritative source is the content_metadata JSON 'tenant_id'.
-- Extract per-row inside a subtransaction so a malformed content_metadata document leaves
-- that row for the source_id / sentinel fallback rather than aborting the migration. MM
-- tables are small; the per-row savepoint cost is acceptable for a one-shot backfill.
DO $$
DECLARE
    r RECORD;
    v_tenant TEXT;
BEGIN
    FOR r IN
        SELECT entry_id, content_metadata
        FROM multimodal_entries
        WHERE tenant_id IS NULL
          AND content_metadata IS NOT NULL
          AND btrim(content_metadata) <> ''
    LOOP
        BEGIN
            v_tenant := NULLIF(btrim(r.content_metadata::jsonb ->> 'tenant_id'), '');
        EXCEPTION WHEN others THEN
            v_tenant := NULL;  -- malformed JSON: fall through to prefix / sentinel
        END;
        IF v_tenant IS NOT NULL THEN
            UPDATE multimodal_entries SET tenant_id = v_tenant WHERE entry_id = r.entry_id;
        END IF;
    END LOOP;
END $$;

-- multimodal_entries (2): fallback to the transitional 't:{tenant}:...' source_id prefix
-- for rows whose JSON carried no tenant_id (or was malformed above).
UPDATE multimodal_entries
SET tenant_id = split_part(source_id, ':', 2)
WHERE tenant_id IS NULL
  AND source_id LIKE 't:%:%'
  AND split_part(source_id, ':', 2) <> '';

-- multimodal_entries (3): rows that cannot be attributed to a tenant are registered
-- read-only so they are auditable and remain invisible to every real tenant once RLS is on.
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT
    gen_random_uuid()::text,
    'multimodal_entries',
    entry_id,
    'unattributable_on_backfill',
    jsonb_build_object('entry_id', entry_id, 'source_id', source_id)::text
FROM multimodal_entries
WHERE tenant_id IS NULL;

-- Sentinel tenant for unattributable entries: never equal to any real request GUC, so
-- those rows stay invisible to tenants (only admin/BYPASSRLS can see them).
UPDATE multimodal_entries
SET tenant_id = '__unattributed__'
WHERE tenant_id IS NULL;

-- modality_relations (1): inherit tenant_id from the source entry (FK-guaranteed to
-- exist). Runs AFTER entries are fully attributed above, so s.tenant_id is always
-- non-null; a relation pointing at an unattributed entry correctly inherits the sentinel
-- and stays invisible too.
UPDATE modality_relations r
SET tenant_id = s.tenant_id
FROM multimodal_entries s
WHERE r.tenant_id IS NULL
  AND r.source_entry_id = s.entry_id;

-- modality_relations (2): defensive fallback to the target entry (also an FK) for any
-- relation the source join missed (should be none under the FK, but keeps the backfill
-- total and guards against pre-FK / NOT VALID data).
UPDATE modality_relations r
SET tenant_id = t.tenant_id
FROM multimodal_entries t
WHERE r.tenant_id IS NULL
  AND r.target_entry_id = t.entry_id;

-- modality_relations (3): any relation still unattributed (neither endpoint resolvable)
-- is registered read-only and parked on the sentinel tenant.
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT
    gen_random_uuid()::text,
    'modality_relations',
    relation_id,
    'unattributable_on_backfill',
    jsonb_build_object('source_entry_id', source_entry_id, 'target_entry_id', target_entry_id)::text
FROM modality_relations
WHERE tenant_id IS NULL;

UPDATE modality_relations
SET tenant_id = '__unattributed__'
WHERE tenant_id IS NULL;

-- ── M2: enforce NOT NULL ──
-- Safe as inline SET NOT NULL here because M1 guarantees no NULLs remain. For very large
-- production tables use the three-phase pattern from the plan (CHECK NOT VALID -> VALIDATE
-- -> SET NOT NULL) to avoid a long ACCESS EXCLUSIVE lock.
ALTER TABLE multimodal_entries
    ALTER COLUMN tenant_id SET NOT NULL;

ALTER TABLE modality_relations
    ALTER COLUMN tenant_id SET NOT NULL;

-- ── M3: enable Row-Level Security + fail-closed tenant policy ──

-- multimodal_entries
ALTER TABLE multimodal_entries ENABLE ROW LEVEL SECURITY;
-- FORCE so the table owner (a non-superuser app role) is also subject to the policy.
-- NOTE: still bypassed by superusers / BYPASSRLS (see deployment note).
ALTER TABLE multimodal_entries FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS multimodal_entries_tenant_isolation ON multimodal_entries;
CREATE POLICY multimodal_entries_tenant_isolation ON multimodal_entries
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

-- modality_relations
ALTER TABLE modality_relations ENABLE ROW LEVEL SECURITY;
ALTER TABLE modality_relations FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS modality_relations_tenant_isolation ON modality_relations;
CREATE POLICY modality_relations_tenant_isolation ON modality_relations
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
