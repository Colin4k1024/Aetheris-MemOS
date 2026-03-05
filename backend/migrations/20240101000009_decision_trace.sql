-- Decision trace (PostgreSQL)

CREATE TABLE IF NOT EXISTS decision_trace (
    trace_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    trace_json TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_decision_trace_task_id ON decision_trace (task_id);
CREATE INDEX IF NOT EXISTS idx_decision_trace_created_at ON decision_trace (created_at);
