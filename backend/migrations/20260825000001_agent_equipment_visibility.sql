-- #89: Add visibility column to agent_equipment for ACL semantics.
-- Private = default (backward-compatible — existing rows were private).
-- Application-layer visibility is enforced by the service layer;
-- RLS already enforces tenant isolation (20260824000001).

ALTER TABLE agent_equipment
    ADD COLUMN IF NOT EXISTS visibility TEXT NOT NULL DEFAULT 'private'
    CHECK (visibility IN ('private', 'team', 'tenant', 'agent', 'task'));

COMMENT ON COLUMN agent_equipment.visibility IS
    'Controls who can discover and use this binding: private (default), team, tenant, agent, task';