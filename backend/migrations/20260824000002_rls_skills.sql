-- #90: retrofit Row-Level Security onto skills.
-- The skills table was created in 20260813000001_distillation_pipeline.sql
-- with a tenant_id column + CHECK/UNIQUE/indexes but NO RLS. RLS makes the
-- database fail-close: a handler that forgets to scope by tenant_id cannot
-- leak cross-tenant skills. Mirrors agent_equipment (20260824000001) +
-- knowledge_entries (20260716000100_rls_ltm).

ALTER TABLE skills ENABLE ROW LEVEL SECURITY;
ALTER TABLE skills FORCE ROW LEVEL SECURITY;

CREATE POLICY skills_tenant_isolation ON skills
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
