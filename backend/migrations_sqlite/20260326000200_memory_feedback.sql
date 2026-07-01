-- Agent memory feedback (SQLite)

CREATE TABLE IF NOT EXISTS memory_feedback (
    feedback_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    memory_id TEXT NOT NULL,
    useful INTEGER NOT NULL CHECK (useful IN (0, 1)),
    query TEXT,
    trace_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_memory_feedback_tenant_memory
    ON memory_feedback (tenant_id, memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_feedback_trace_id
    ON memory_feedback (trace_id);
