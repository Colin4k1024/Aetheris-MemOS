-- Workflow evidence graph baseline (PostgreSQL)

CREATE TABLE IF NOT EXISTS workflow_evidence_runs (
    run_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    sequence_number BIGINT NOT NULL CHECK (sequence_number >= 0),
    prev_hash TEXT,
    node_hash TEXT NOT NULL,
    tool_invocations JSONB NOT NULL DEFAULT '[]'::jsonb,
    context_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (workflow_id, sequence_number)
);

CREATE TABLE IF NOT EXISTS workflow_evidence_nodes (
    node_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES workflow_evidence_runs(run_id) ON DELETE RESTRICT,
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    sequence_number BIGINT NOT NULL CHECK (sequence_number >= 0),
    node_kind TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    llm_input_hash TEXT NOT NULL,
    llm_output_hash TEXT NOT NULL,
    tool_invocations JSONB NOT NULL DEFAULT '[]'::jsonb,
    context_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    prev_hash TEXT,
    node_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (run_id, sequence_number)
);

CREATE TABLE IF NOT EXISTS workflow_evidence_edges (
    edge_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES workflow_evidence_runs(run_id) ON DELETE RESTRICT,
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    sequence_number BIGINT NOT NULL CHECK (sequence_number >= 0),
    source_node_id TEXT NOT NULL REFERENCES workflow_evidence_nodes(node_id) ON DELETE RESTRICT,
    target_node_id TEXT NOT NULL REFERENCES workflow_evidence_nodes(node_id) ON DELETE RESTRICT,
    edge_kind TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    tool_invocations JSONB NOT NULL DEFAULT '[]'::jsonb,
    context_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    prev_hash TEXT,
    node_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (run_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS idx_workflow_evidence_runs_workflow_id
    ON workflow_evidence_runs (workflow_id, attempt_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_workflow_evidence_nodes_workflow_id
    ON workflow_evidence_nodes (workflow_id, attempt_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_workflow_evidence_edges_workflow_id
    ON workflow_evidence_edges (workflow_id, attempt_id, sequence_number);

CREATE OR REPLACE FUNCTION prevent_workflow_evidence_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'workflow evidence tables are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS workflow_evidence_runs_append_only ON workflow_evidence_runs;
CREATE TRIGGER workflow_evidence_runs_append_only
BEFORE UPDATE OR DELETE ON workflow_evidence_runs
FOR EACH ROW EXECUTE FUNCTION prevent_workflow_evidence_mutation();

DROP TRIGGER IF EXISTS workflow_evidence_nodes_append_only ON workflow_evidence_nodes;
CREATE TRIGGER workflow_evidence_nodes_append_only
BEFORE UPDATE OR DELETE ON workflow_evidence_nodes
FOR EACH ROW EXECUTE FUNCTION prevent_workflow_evidence_mutation();

DROP TRIGGER IF EXISTS workflow_evidence_edges_append_only ON workflow_evidence_edges;
CREATE TRIGGER workflow_evidence_edges_append_only
BEFORE UPDATE OR DELETE ON workflow_evidence_edges
FOR EACH ROW EXECUTE FUNCTION prevent_workflow_evidence_mutation();
