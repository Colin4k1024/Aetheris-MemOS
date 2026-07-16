-- Memory Storage Tenant Foundation (expand phase)
--
-- This migration intentionally performs only additive, nullable changes. It is
-- the first step of the expand-contract plan documented in
-- docs/artifacts/2026-07-06-memory-storage-reliability/.
--
-- Later phases are responsible for backfill, read-only isolation enforcement,
-- NOT NULL constraints, tenant-scoped unique constraints, and RLS policies.

-- STM tenant scope
ALTER TABLE context_sessions
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE context_messages
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE session_messages
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

CREATE INDEX IF NOT EXISTS idx_context_sessions_tenant_session
ON context_sessions (tenant_id, session_id);

CREATE INDEX IF NOT EXISTS idx_context_sessions_tenant_user_status
ON context_sessions (tenant_id, user_id, status, expires_at);

CREATE INDEX IF NOT EXISTS idx_context_messages_tenant_session
ON context_messages (tenant_id, session_id, message_index);

CREATE INDEX IF NOT EXISTS idx_session_messages_tenant_session
ON session_messages (tenant_id, session_id, created_at);

-- LTM tenant scope
ALTER TABLE knowledge_entries
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE knowledge_relations
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE knowledge_entry_versions
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

CREATE INDEX IF NOT EXISTS idx_knowledge_entries_tenant_entry
ON knowledge_entries (tenant_id, entry_id);

CREATE INDEX IF NOT EXISTS idx_knowledge_entries_tenant_source
ON knowledge_entries (tenant_id, source_id, source_type);

CREATE INDEX IF NOT EXISTS idx_knowledge_entries_tenant_status_created
ON knowledge_entries (tenant_id, status, created_at);

CREATE INDEX IF NOT EXISTS idx_knowledge_entries_tenant_hash
ON knowledge_entries (tenant_id, content_hash);

CREATE INDEX IF NOT EXISTS idx_knowledge_relations_tenant_source
ON knowledge_relations (tenant_id, source_entry_id);

CREATE INDEX IF NOT EXISTS idx_knowledge_relations_tenant_target
ON knowledge_relations (tenant_id, target_entry_id);

CREATE INDEX IF NOT EXISTS idx_knowledge_entry_versions_tenant_entry
ON knowledge_entry_versions (tenant_id, entry_id, version_number);

-- KG tenant scope
ALTER TABLE entities
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE relations
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE reasoning_paths
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE entity_versions
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

CREATE INDEX IF NOT EXISTS idx_entities_tenant_entity
ON entities (tenant_id, entity_id);

CREATE INDEX IF NOT EXISTS idx_entities_tenant_name_type
ON entities (tenant_id, entity_name, entity_type);

CREATE INDEX IF NOT EXISTS idx_relations_tenant_source
ON relations (tenant_id, source_entity_id);

CREATE INDEX IF NOT EXISTS idx_relations_tenant_target
ON relations (tenant_id, target_entity_id);

CREATE INDEX IF NOT EXISTS idx_reasoning_paths_tenant_source
ON reasoning_paths (tenant_id, source_entity_id);

CREATE INDEX IF NOT EXISTS idx_reasoning_paths_tenant_target
ON reasoning_paths (tenant_id, target_entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_versions_tenant_entity
ON entity_versions (tenant_id, entity_id, version_number);

-- MM tenant scope
ALTER TABLE multimodal_entries
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

ALTER TABLE modality_relations
ADD COLUMN IF NOT EXISTS tenant_id TEXT;

CREATE INDEX IF NOT EXISTS idx_multimodal_entries_tenant_entry
ON multimodal_entries (tenant_id, entry_id);

CREATE INDEX IF NOT EXISTS idx_multimodal_entries_tenant_session
ON multimodal_entries (tenant_id, session_id);

CREATE INDEX IF NOT EXISTS idx_multimodal_entries_tenant_source
ON multimodal_entries (tenant_id, source_id);

CREATE INDEX IF NOT EXISTS idx_modality_relations_tenant_source
ON modality_relations (tenant_id, source_entry_id);

CREATE INDEX IF NOT EXISTS idx_modality_relations_tenant_target
ON modality_relations (tenant_id, target_entry_id);

-- Read-only isolation for historical rows that cannot be safely attributed to a tenant.
CREATE TABLE IF NOT EXISTS memory_tenant_readonly_isolation (
    isolation_id TEXT PRIMARY KEY,
    table_name TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    evidence_json TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    reviewed_at TIMESTAMPTZ,
    reviewed_by TEXT,
    status TEXT NOT NULL DEFAULT 'readonly' CHECK (status IN ('readonly', 'resolved'))
);

CREATE INDEX IF NOT EXISTS idx_memory_tenant_readonly_isolation_resource
ON memory_tenant_readonly_isolation (table_name, resource_id);

CREATE INDEX IF NOT EXISTS idx_memory_tenant_readonly_isolation_status
ON memory_tenant_readonly_isolation (status, created_at);

-- Durable outbox foundation for LTM/Qdrant synchronization.
CREATE TABLE IF NOT EXISTS memory_vector_outbox (
    event_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
    payload_json TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'processing', 'applied', 'failed', 'dead_letter')
    ),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_retry_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    locked_at TIMESTAMPTZ,
    locked_by TEXT,
    last_error TEXT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    applied_at TIMESTAMPTZ,
    dead_lettered_at TIMESTAMPTZ,
    UNIQUE (tenant_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_memory_vector_outbox_status_retry
ON memory_vector_outbox (status, next_retry_at, created_at);

CREATE INDEX IF NOT EXISTS idx_memory_vector_outbox_tenant_entry
ON memory_vector_outbox (tenant_id, entry_id, created_at);

-- Reconciliation run and item records. Repair remains dry-run by default in later services.
CREATE TABLE IF NOT EXISTS memory_vector_reconciliation_runs (
    run_id TEXT PRIMARY KEY,
    tenant_id TEXT,
    mode TEXT NOT NULL DEFAULT 'dry_run' CHECK (mode IN ('dry_run', 'repair')),
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed')),
    started_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ,
    summary_json TEXT NOT NULL DEFAULT '{}',
    error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_memory_vector_reconciliation_runs_tenant
ON memory_vector_reconciliation_runs (tenant_id, started_at);

CREATE TABLE IF NOT EXISTS memory_vector_reconciliation_items (
    item_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES memory_vector_reconciliation_runs(run_id) ON DELETE CASCADE,
    tenant_id TEXT,
    entry_id TEXT,
    qdrant_point_id TEXT,
    drift_type TEXT NOT NULL CHECK (
        drift_type IN ('missing', 'orphan', 'tenant_mismatch', 'content_hash_mismatch')
    ),
    action TEXT NOT NULL DEFAULT 'report' CHECK (
        action IN ('report', 'upsert', 'delete', 'rewrite_payload', 'readonly')
    ),
    details_json TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    repaired_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_memory_vector_reconciliation_items_run
ON memory_vector_reconciliation_items (run_id, drift_type);

CREATE INDEX IF NOT EXISTS idx_memory_vector_reconciliation_items_tenant_entry
ON memory_vector_reconciliation_items (tenant_id, entry_id);

-- Persistent audit event foundation for memory storage reliability gates.
CREATE TABLE IF NOT EXISTS memory_audit_events (
    event_id TEXT PRIMARY KEY,
    tenant_id TEXT,
    actor_id TEXT,
    event_type TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    correlation_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_memory_audit_events_tenant_created
ON memory_audit_events (tenant_id, created_at);

CREATE INDEX IF NOT EXISTS idx_memory_audit_events_resource
ON memory_audit_events (resource_type, resource_id, created_at);

CREATE INDEX IF NOT EXISTS idx_memory_audit_events_correlation
ON memory_audit_events (correlation_id);
