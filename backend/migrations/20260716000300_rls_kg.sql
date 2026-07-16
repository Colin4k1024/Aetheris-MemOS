-- RLS vertical — KG slice: entities + relations.
--
-- Third RLS slice (after LTM 20260716000100 and STM 20260716000200). Takes both
-- KG tables through backfill -> enforce -> RLS in one transactional file (sqlx
-- runs it in a single tx; every statement below is transaction-safe, so the whole
-- tenant hardening applies atomically or not at all).
--
-- Plan: docs/artifacts/2026-07-16-enterprise-productionization/p1-rls-isolation-plan.md
--   and ADR-0001. GUC key: aetheris.tenant_id (transaction-local), read by policies
--   via current_setting('aetheris.tenant_id', true) — same as the LTM/STM slices.
--
-- TENANT SOURCE OF TRUTH
--   entities:  entity_id is 't:{tenant}:{ulid}' -> split_part(entity_id,':',2)=tenant.
--   relations: relations carry NO tenant prefix of their own. They are backfilled
--              by joining to entities via source_entity_id (an FK: every relation's
--              source_entity_id REFERENCES entities(entity_id)), inheriting that
--              entity's tenant_id. New rows get the physical tenant_id written by
--              db/kg.rs::create_relation (which now takes a tenant_id parameter).
--
-- ⚠️ RLS COMPLETENESS PRECONDITION: every code path that reads/writes entities OR
-- relations must run inside a tenant-scoped transaction (begin_tenant_tx) so the GUC
-- is set, or RLS fail-closes it to zero rows. Audited PG-direct access points as of
-- this migration (grep of src/ for FROM/JOIN/INTO/UPDATE on entities|relations):
--   • db/kg.rs — ALL KGRepository methods now route through begin_tenant_tx and
--     double-write the physical tenant_id column:
--       create_entity, get_entity_by_name, get_entity_by_id, create_relation
--       (now takes tenant_id), get_related_entities, search_knowledge_by_entity_for_tenant,
--       search_entries_by_entity_for_tenant, list_entities, get_entity_at_time,
--       get_entity_history, supersede_entity.
--   • services/memory_fusion.rs — query_kg + count_kg now route through begin_tenant_tx.
-- Every other reference (routers/knowledge_graph, routers/mcp, routers/data_io,
-- routers/visualization, routers/memory_search, services/bitemporal_kg,
-- services/memory_search) calls the KGRepository methods above and therefore inherits
-- RLS transitively — none issues raw SQL on entities/relations.
-- db/neo4j.rs targets Neo4j (a different datastore), NOT these PG tables — excluded.
-- reasoning_paths and entity_versions have NO raw-SQL access path in src/ at all;
-- they are hardened at the schema level below for defense in depth but have no
-- code path to break.
-- NO admin/BYPASSRLS exception is required for the KG slice: unlike LTM there is no
-- global cross-tenant scan (no equivalent of list_qdrant_tenant_backfill_entries).
--
-- ⚠️ FK vs RLS: PostgreSQL referential-integrity checks (the relations ->
-- entities(entity_id) FK) ALWAYS bypass row security. This is intentional and safe
-- here: create_relation validates both endpoints belong to the caller's tenant via
-- RLS-scoped get_entity_by_id reads before inserting, and WITH CHECK still pins the
-- relation's own tenant_id to the GUC. The FK bypass only means the existence check
-- of the referenced entity_id is not itself tenant-filtered.
--
-- ⚠️ DEPLOYMENT PRECONDITION: RLS (even FORCE) is bypassed by superusers and roles
-- with BYPASSRLS. The application connection MUST be a non-superuser, non-BYPASSRLS
-- role for these policies to take effect. The stock dev image connects as `memory`
-- (owner + superuser) — under it RLS is a NO-OP. See tests/rls_kg_pg.rs, which
-- provisions a dedicated restricted role to prove isolation.

-- ── M1: backfill physical tenant_id ──

-- entities: derive tenant from the transitional 't:{tenant}:{ulid}' entity_id prefix.
UPDATE entities
SET tenant_id = split_part(entity_id, ':', 2)
WHERE tenant_id IS NULL
  AND entity_id LIKE 't:%:%';

-- entities that cannot be attributed to a tenant are registered read-only so they are
-- auditable and remain invisible to every real tenant once RLS is on.
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT
    gen_random_uuid()::text,
    'entities',
    entity_id,
    'unattributable_on_backfill',
    jsonb_build_object('entity_id', entity_id)::text
FROM entities
WHERE tenant_id IS NULL;

-- Sentinel tenant for unattributable entities: never equal to any real request GUC,
-- so those rows stay invisible to tenants (only admin/BYPASSRLS can see them).
UPDATE entities
SET tenant_id = '__unattributed__'
WHERE tenant_id IS NULL;

-- relations: inherit tenant_id from the source entity (FK-guaranteed to exist).
-- Runs AFTER entities are fully attributed above, so e.tenant_id is always non-null;
-- a relation pointing at an unattributed entity correctly inherits the sentinel and
-- stays invisible too.
UPDATE relations r
SET tenant_id = e.tenant_id
FROM entities e
WHERE r.tenant_id IS NULL
  AND r.source_entity_id = e.entity_id;

-- Defensive: any relation whose source_entity_id has no matching entity (should be
-- impossible under the FK, but guards against pre-FK/NOT VALID data) is registered
-- read-only and parked on the sentinel tenant.
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT
    gen_random_uuid()::text,
    'relations',
    relation_id,
    'unattributable_on_backfill',
    jsonb_build_object('source_entity_id', source_entity_id, 'target_entity_id', target_entity_id)::text
FROM relations
WHERE tenant_id IS NULL;

UPDATE relations
SET tenant_id = '__unattributed__'
WHERE tenant_id IS NULL;

-- ── M2: enforce NOT NULL ──
-- Safe as inline SET NOT NULL here because M1 guarantees no NULLs remain. For very
-- large production tables use the three-phase pattern from the plan (CHECK NOT VALID
-- -> VALIDATE -> SET NOT NULL) to avoid a long ACCESS EXCLUSIVE lock.
ALTER TABLE entities
    ALTER COLUMN tenant_id SET NOT NULL;

ALTER TABLE relations
    ALTER COLUMN tenant_id SET NOT NULL;

-- ── M3: enable Row-Level Security + fail-closed tenant policy ──

-- entities
ALTER TABLE entities ENABLE ROW LEVEL SECURITY;
-- FORCE so the table owner (a non-superuser app role) is also subject to the policy.
-- NOTE: still bypassed by superusers / BYPASSRLS (see deployment note).
ALTER TABLE entities FORCE ROW LEVEL SECURITY;

CREATE POLICY entities_tenant_isolation ON entities
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

-- relations
ALTER TABLE relations ENABLE ROW LEVEL SECURITY;
ALTER TABLE relations FORCE ROW LEVEL SECURITY;

CREATE POLICY relations_tenant_isolation ON relations
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
