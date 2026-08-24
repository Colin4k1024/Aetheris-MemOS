-- #89: retrofit Row-Level Security onto agent_equipment.
-- The table was created in 20260813000001_distillation_pipeline.sql with a
-- tenant_id column + CHECK/UNIQUE constraints but NO RLS, so it relied solely
-- on the application layer for tenant isolation. RLS makes the database
-- fail-close: even a handler that forgets to scope by tenant_id cannot leak
-- cross-tenant equipment rows. Mirrors the knowledge_entries RLS pattern
-- (20260716000100_rls_ltm.sql).

ALTER TABLE agent_equipment ENABLE ROW LEVEL SECURITY;
-- FORCE so a non-superuser app role is also subject to the policy (still
-- bypassed by superusers / BYPASSRLS — see deployment notes in ADR-RLS).
ALTER TABLE agent_equipment FORCE ROW LEVEL SECURITY;

CREATE POLICY agent_equipment_tenant_isolation ON agent_equipment
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
