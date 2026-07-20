-- Add tenant_id column to decision_trace for application-layer tenant enforcement.
--
-- The code (db/decision_trace.rs) already scopes all INSERT/SELECT by tenant_id,
-- but the column was never added to the table schema. This migration backfills
-- existing rows with the default tenant and adds a NOT NULL constraint.

ALTER TABLE decision_trace
    ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default';

CREATE INDEX IF NOT EXISTS idx_decision_trace_tenant_id ON decision_trace (tenant_id);

-- Composite index for the most common query pattern: tenant + task + time ordering.
CREATE INDEX IF NOT EXISTS idx_decision_trace_tenant_task
    ON decision_trace (tenant_id, task_id, created_at DESC);
