-- Weight History (PostgreSQL)

CREATE TABLE IF NOT EXISTS weight_adjustment_history (
    history_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    old_weights_json TEXT NOT NULL,
    new_weights_json TEXT NOT NULL,
    adjustment_reasons_json TEXT NOT NULL,
    performance_impact REAL DEFAULT 0.0,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_weight_history_task_id ON weight_adjustment_history (task_id);
CREATE INDEX IF NOT EXISTS idx_weight_history_timestamp ON weight_adjustment_history (timestamp);
