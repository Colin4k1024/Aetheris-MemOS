-- #127: belief lifecycle tables — the write-gate's durable half.
--
-- Five tables, all tenant-scoped with the standard fail-closed RLS policy:
--   memory_predicate_policies  — the governed predicate allowlist (#125 catalog
--                                materialized into PG; seeded from
--                                models::belief::PREDICATE_CATALOG, drift-tested)
--   memory_beliefs             — bitemporal SPO edges (valid_* + recorded_at),
--                                supersede-linked, NEVER overwritten
--   memory_belief_candidates   — pre-commit propositions produced by extraction
--                                (#127 wires the distillation worker here);
--                                carry the guard verdict and idempotency key
--   memory_belief_evidence     — provenance rows binding claims/beliefs back to
--                                immutable memory_events
--   memory_contracts           — per-agent "may believe / must-not-believe"
--                                contracts (Epic #124 §记忆契约)
--
-- ⚠️ RLS PRECONDITION: every code path runs inside begin_tenant_tx or reads zero
-- rows. See rls_ltm.sql header for the full deployment caveats.

CREATE EXTENSION IF NOT EXISTS btree_gist;

-- ── Table 1/5: predicate policies (the allowlist, materialized) ─────────────

CREATE TABLE IF NOT EXISTS memory_predicate_policies (
    name TEXT PRIMARY KEY,
    cardinality TEXT NOT NULL CHECK (cardinality IN ('single', 'multi')),
    mutability TEXT NOT NULL CHECK (mutability IN ('mutable', 'immutable', 'time_bounded')),
    allowed_sources JSONB NOT NULL DEFAULT '[]'::jsonb,
    ttl_policy TEXT NOT NULL CHECK (ttl_policy IN ('no_ttl', 'stale_scan', 'sor_driven', 'expires_at_due_date')),
    -- stale_scan predicates MUST declare a positive window; others leave NULL.
    reconfirm_days INTEGER CHECK (
        (ttl_policy <> 'stale_scan')
        OR (reconfirm_days IS NOT NULL AND reconfirm_days > 0)
    ),
    risk TEXT NOT NULL CHECK (risk IN ('low', 'medium', 'high')),
    description TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Table 2/5: beliefs (bitemporal SPO edges) ───────────────────────────────

CREATE TABLE IF NOT EXISTS memory_beliefs (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    principal_id TEXT NOT NULL REFERENCES memory_principals(id),
    subject TEXT NOT NULL,
    predicate TEXT NOT NULL REFERENCES memory_predicate_policies(name),
    object TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'quarantined', 'candidate', 'active', 'needs_confirm',
        'stale', 'superseded', 'archived', 'rejected')),
    source TEXT NOT NULL CHECK (source IN (
        'user_stated', 'tool', 'system_of_record', 'web', 'inferred')),
    trust REAL NOT NULL CHECK (trust >= 0 AND trust <= 1),
    risk TEXT NOT NULL CHECK (risk IN ('low', 'medium', 'high')),
    valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_to TIMESTAMPTZ,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    supersedes_id TEXT REFERENCES memory_beliefs(id),
    superseded_by_id TEXT REFERENCES memory_beliefs(id),
    needs_confirm BOOLEAN NOT NULL DEFAULT FALSE,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- The core read surface (#128 will consume this): current edges only.
CREATE INDEX IF NOT EXISTS idx_beliefs_active_edge
ON memory_beliefs(tenant_id, subject, predicate, valid_from DESC)
WHERE valid_to IS NULL AND status = 'active';

CREATE INDEX IF NOT EXISTS idx_beliefs_tenant_principal
ON memory_beliefs(tenant_id, principal_id);
CREATE INDEX IF NOT EXISTS idx_beliefs_tenant_status
ON memory_beliefs(tenant_id, status);
CREATE INDEX IF NOT EXISTS idx_beliefs_supersedes
ON memory_beliefs(supersedes_id) WHERE supersedes_id IS NOT NULL;

-- CONCURRENCY ANCHOR (#126 acceptance analog): for SINGLE-cardinality
-- predicates at most ONE open (non-superseded) edge may exist per
-- (tenant, subject, predicate) — enforced by PostgreSQL itself, not by
-- application discipline. Two racing writers: one commits, the other violates
-- the exclusion and the gate retries against fresh state. needs_confirm edges
-- reserve the slot too (a pending confirmation blocks a second claim line).
ALTER TABLE memory_beliefs ADD CONSTRAINT beliefs_single_open_edge_per_subject
EXCLUDE USING gist (
    tenant_id WITH =,
    subject WITH =,
    predicate WITH =,
    tstzrange(valid_from, COALESCE(valid_to, 'infinity'), '[)') WITH &&
) WHERE (status IN ('active', 'needs_confirm'));

COMMENT ON CONSTRAINT beliefs_single_open_edge_per_subject ON memory_beliefs IS
    'At most one open edge per single-cardinality (tenant, subject, predicate): multi-cardinality predicates are exempted because their policies are excluded upstream by the writer (rows here are gated output), and closed edges (valid_to set) do not collide.';

-- ── Table 3/5: candidates (pre-commit propositions) ─────────────────────────

CREATE TABLE IF NOT EXISTS memory_belief_candidates (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    principal_id TEXT NOT NULL REFERENCES memory_principals(id),
    session_id TEXT,
    subject TEXT NOT NULL,
    predicate TEXT NOT NULL REFERENCES memory_predicate_policies(name),
    object TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN (
        'user_stated', 'tool', 'system_of_record', 'web', 'inferred')),
    trust REAL NOT NULL CHECK (trust >= 0 AND trust <= 1),
    -- Where the claim came from mechanically (extraction stage label).
    origin TEXT NOT NULL DEFAULT 'manual' CHECK (origin IN ('manual', 'distillation', 'external', 'api')),
    -- The gate's verdict: NULL until evaluated; one of WriteDecision values or
    -- 'rejected'/'quarantined' when the claim never became an edge.
    decision TEXT CHECK (decision IN ('add', 'supersede', 'noop', 'conflict')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'accepted', 'rejected', 'quarantined')),
    outcome_belief_id TEXT REFERENCES memory_beliefs(id),
    rejection_reason TEXT,
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- Replay guard over the CLAIM (not the event): identical retries collapse.
    idempotency_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_belief_candidates_tenant_idem
ON memory_belief_candidates(tenant_id, idempotency_key)
WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_belief_candidates_tenant_status
ON memory_belief_candidates(tenant_id, status, created_at DESC);

-- ── Table 4/5: evidence (provenance binding) ────────────────────────────────

CREATE TABLE IF NOT EXISTS memory_belief_evidence (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    belief_id TEXT REFERENCES memory_beliefs(id),
    candidate_id TEXT REFERENCES memory_belief_candidates(id),
    event_id TEXT REFERENCES memory_events(id),
    kind TEXT NOT NULL DEFAULT 'direct' CHECK (kind IN ('direct', 'derived')),
    content_hash TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (belief_id IS NOT NULL OR candidate_id IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_belief_evidence_belief
ON memory_belief_evidence(tenant_id, belief_id);
CREATE INDEX IF NOT EXISTS idx_belief_evidence_candidate
ON memory_belief_evidence(tenant_id, candidate_id);

-- ── Table 5/5: agent memory contracts ───────────────────────────────────────

CREATE TABLE IF NOT EXISTS memory_contracts (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    may_believe JSONB NOT NULL DEFAULT '[]'::jsonb,
    must_not_believe_from JSONB NOT NULL DEFAULT '{}'::jsonb,
    high_stakes_deny_below_trust REAL CHECK (
        high_stakes_deny_below_trust IS NULL
        OR (high_stakes_deny_below_trust >= 0 AND high_stakes_deny_below_trust <= 1)),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, agent_id)
);

-- ── Row-Level Security (fail-closed tenant policy, standard pattern) ────────
-- memory_predicate_policies is deliberately NOT in this loop: it is a GLOBAL
-- catalog (no tenant_id column) seeded from code — the allowlist is identical
-- for every tenant by design, exactly like the Rust enum it mirrors. Tenant
-- scoping would let a tenant "un-govern" a predicate for itself.

DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'memory_beliefs',
        'memory_belief_candidates',
        'memory_belief_evidence',
        'memory_contracts'
    ]
    LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (
                current_setting(''aetheris.tenant_id'', true) IS NOT NULL
                AND tenant_id = current_setting(''aetheris.tenant_id'', true)
            ) WITH CHECK (
                current_setting(''aetheris.tenant_id'', true) IS NOT NULL
                AND tenant_id = current_setting(''aetheris.tenant_id'', true)
            )',
            t || '_tenant_isolation',
            t
        );
    END LOOP;
END
$$;
