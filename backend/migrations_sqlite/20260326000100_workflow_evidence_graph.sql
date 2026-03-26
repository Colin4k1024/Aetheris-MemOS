-- Workflow evidence graph baseline (SQLite)

CREATE TABLE IF NOT EXISTS workflow_evidence_runs (
    run_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    sequence_number INTEGER NOT NULL CHECK (sequence_number >= 0),
    prev_hash TEXT,
    node_hash TEXT NOT NULL,
    tool_invocations TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tool_invocations)),
    context_snapshot TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(context_snapshot)),
    metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (workflow_id, sequence_number)
);

CREATE TABLE IF NOT EXISTS workflow_evidence_nodes (
    node_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES workflow_evidence_runs(run_id) ON DELETE RESTRICT,
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    sequence_number INTEGER NOT NULL CHECK (sequence_number >= 0),
    node_kind TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    llm_input_hash TEXT NOT NULL,
    llm_output_hash TEXT NOT NULL,
    tool_invocations TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tool_invocations)),
    context_snapshot TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(context_snapshot)),
    metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    prev_hash TEXT,
    node_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (run_id, sequence_number)
);

CREATE TABLE IF NOT EXISTS workflow_evidence_edges (
    edge_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES workflow_evidence_runs(run_id) ON DELETE RESTRICT,
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    sequence_number INTEGER NOT NULL CHECK (sequence_number >= 0),
    source_node_id TEXT NOT NULL REFERENCES workflow_evidence_nodes(node_id) ON DELETE RESTRICT,
    target_node_id TEXT NOT NULL REFERENCES workflow_evidence_nodes(node_id) ON DELETE RESTRICT,
    edge_kind TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    tool_invocations TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tool_invocations)),
    context_snapshot TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(context_snapshot)),
    metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    prev_hash TEXT,
    node_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (run_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS idx_workflow_evidence_runs_workflow_id
    ON workflow_evidence_runs (workflow_id, attempt_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_workflow_evidence_nodes_workflow_id
    ON workflow_evidence_nodes (workflow_id, attempt_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_workflow_evidence_edges_workflow_id
    ON workflow_evidence_edges (workflow_id, attempt_id, sequence_number);

CREATE TRIGGER IF NOT EXISTS workflow_evidence_runs_no_update
BEFORE UPDATE ON workflow_evidence_runs
BEGIN
    SELECT RAISE(ABORT, 'workflow_evidence_runs is append-only');
END;

CREATE TRIGGER IF NOT EXISTS workflow_evidence_runs_no_delete
BEFORE DELETE ON workflow_evidence_runs
BEGIN
    SELECT RAISE(ABORT, 'workflow_evidence_runs is append-only');
END;

CREATE TRIGGER IF NOT EXISTS workflow_evidence_nodes_no_update
BEFORE UPDATE ON workflow_evidence_nodes
BEGIN
    SELECT RAISE(ABORT, 'workflow_evidence_nodes is append-only');
END;

CREATE TRIGGER IF NOT EXISTS workflow_evidence_nodes_no_delete
BEFORE DELETE ON workflow_evidence_nodes
BEGIN
    SELECT RAISE(ABORT, 'workflow_evidence_nodes is append-only');
END;

CREATE TRIGGER IF NOT EXISTS workflow_evidence_edges_no_update
BEFORE UPDATE ON workflow_evidence_edges
BEGIN
    SELECT RAISE(ABORT, 'workflow_evidence_edges is append-only');
END;

CREATE TRIGGER IF NOT EXISTS workflow_evidence_edges_no_delete
BEFORE DELETE ON workflow_evidence_edges
BEGIN
    SELECT RAISE(ABORT, 'workflow_evidence_edges is append-only');
END;
