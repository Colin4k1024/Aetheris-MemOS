-- #126: tenant-level append-only memory event stream + principal identity graph.
--
-- Three tables, all tenant-scoped with RLS (same fail-closed policy pattern as
-- 20260716000100_rls_ltm.sql and 20260824000001_rls_agent_equipment.sql):
--   memory_principals   — WHO a piece of memory belongs to (person / service
--                         account / device / anonymous). Memory attaches to a
--                         principal, never to a session (#124 Epic: "记忆挂在
--                         principal 上，不挂在 session 上").
--   principal_aliases   — identity keys that resolve to a principal (jwt_sub,
--                         username, email, device_id, external_id).
--   memory_events       — THE append-only log: conversation turns, agent replies,
--                         tool results, system notes, external CRM/HR records all
--                         land here FIRST ("统一先落事件").
--
-- ⚠️ RLS PRECONDITION (inherited): policies read the transaction-local GUC
-- `aetheris.tenant_id`, so every code path touching these tables MUST run inside
-- begin_tenant_tx, or it fails closed to zero rows. Superusers / BYPASSRLS roles
-- bypass RLS entirely — the penetration test provisions a restricted probe role.
--
-- Merge semantics: an anonymous principal is merged into a person by pointing
-- merged_into_id at the person (one-hop redirect) + status='merged'. The redirect
-- is explicitly REVERSIBLE (clear the pointer, restore 'active') and both
-- directions write audit rows into memory_audit_events from the service layer.
-- Sharing a device creates a SEPARATE device-kind principal; nothing in this
-- schema links a device alias to a person automatically.

-- ── Table 1/3: principals ────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS memory_principals (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('person', 'service_account', 'device', 'anonymous')),
    display_name TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'merged', 'deactivated')),
    -- One-hop merge redirect. Non-NULL iff status = 'merged'. Reversible by design;
    -- the service layer keeps chains short (max depth 8) and rejects cycles.
    merged_into_id TEXT REFERENCES memory_principals(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_principals_tenant_kind ON memory_principals(tenant_id, kind);
CREATE INDEX IF NOT EXISTS idx_principals_tenant_status ON memory_principals(tenant_id, status);
CREATE INDEX IF NOT EXISTS idx_principals_merged_into ON memory_principals(merged_into_id)
    WHERE merged_into_id IS NOT NULL;

-- ── Table 2/3: aliases ───────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS principal_aliases (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    principal_id TEXT NOT NULL REFERENCES memory_principals(id),
    alias_type TEXT NOT NULL CHECK (alias_type IN ('jwt_sub', 'username', 'email', 'device_id', 'external_id')),
    alias_value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, alias_type, alias_value)
);

-- Exact-match lookups by (tenant, type, value) are covered by the UNIQUE index;
-- this one serves "all identities of a principal" walks.
CREATE INDEX IF NOT EXISTS idx_aliases_principal ON principal_aliases(tenant_id, principal_id);

-- ── Table 3/3: events ────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS memory_events (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    principal_id TEXT NOT NULL REFERENCES memory_principals(id),
    -- Episodic container when the event came from a conversation/session; NULL
    -- for external records (CRM/HR) that have no session of their own.
    session_id TEXT,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'user_message', 'agent_reply', 'tool_result', 'system_event', 'external_record')),
    -- Who introduced the event: jwt uid / agent id / source-system tag. Not
    -- necessarily a principal id yet; entity alignment is a later pipeline stage.
    actor TEXT,
    content_hash TEXT NOT NULL,
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    idempotency_key TEXT
);

-- Idempotent replay protection: a retried producer MUST NOT create a second row
-- (see INSERT ... ON CONFLICT in db/memory_event.rs). Partial because callers may
-- omit the key.
CREATE UNIQUE INDEX IF NOT EXISTS idx_events_tenant_idempotency ON memory_events(tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_events_tenant_recorded ON memory_events(tenant_id, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_tenant_principal_occurred ON memory_events(tenant_id, principal_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_tenant_session ON memory_events(tenant_id, session_id, recorded_at DESC)
    WHERE session_id IS NOT NULL;

-- Append-only at the DATABASE layer, not just by convention: the hardened app
-- role gets INSERT+SELECT only. Retention/archival jobs run under a separate
-- admin connection BY DESIGN (an immutable evidence log must not be casually
-- editable by application code paths). `memory` stays superuser for dev.
REVOKE UPDATE, DELETE ON memory_events FROM aetheris_app;

COMMENT ON TABLE memory_events IS
    'Append-only memory event stream (#126): conversations, tool results and system-of-record records land here before any distillation. Update/Delete are revoked from the app role; history is corrected by supersede-style compensation events, not edits.';
COMMENT ON COLUMN memory_events.idempotency_key IS
    'Producer-supplied replay guard; unique per tenant when present.';

-- ── Row-Level Security (fail-closed tenant policy on all three tables) ──────

ALTER TABLE memory_principals ENABLE ROW LEVEL SECURITY;
ALTER TABLE memory_principals FORCE ROW LEVEL SECURITY;
CREATE POLICY memory_principals_tenant_isolation ON memory_principals
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

ALTER TABLE principal_aliases ENABLE ROW LEVEL SECURITY;
ALTER TABLE principal_aliases FORCE ROW LEVEL SECURITY;
CREATE POLICY principal_aliases_tenant_isolation ON principal_aliases
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

ALTER TABLE memory_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE memory_events FORCE ROW LEVEL SECURITY;
CREATE POLICY memory_events_tenant_isolation ON memory_events
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
