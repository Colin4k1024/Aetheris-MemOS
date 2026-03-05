-- Memory Management (PostgreSQL)

CREATE TABLE IF NOT EXISTS memory_configurations (
    config_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    config_name TEXT NOT NULL,
    config_type TEXT NOT NULL CHECK (config_type IN ('default', 'custom', 'optimized')),
    stm_enabled SMALLINT DEFAULT 1 CHECK (stm_enabled IN (0, 1)),
    stm_max_length INTEGER DEFAULT 4096 CHECK (stm_max_length > 0),
    stm_retention_hours INTEGER DEFAULT 24 CHECK (stm_retention_hours > 0),
    ltm_enabled SMALLINT DEFAULT 1 CHECK (ltm_enabled IN (0, 1)),
    ltm_max_entries INTEGER DEFAULT 10000 CHECK (ltm_max_entries > 0),
    ltm_quality_threshold REAL DEFAULT 0.50 CHECK (ltm_quality_threshold >= 0.00 AND ltm_quality_threshold <= 1.00),
    kg_enabled SMALLINT DEFAULT 0 CHECK (kg_enabled IN (0, 1)),
    kg_max_entities INTEGER DEFAULT 1000 CHECK (kg_max_entities > 0),
    kg_confidence_threshold REAL DEFAULT 0.70 CHECK (kg_confidence_threshold >= 0.00 AND kg_confidence_threshold <= 1.00),
    mm_enabled SMALLINT DEFAULT 0 CHECK (mm_enabled IN (0, 1)),
    mm_max_entries INTEGER DEFAULT 1000 CHECK (mm_max_entries > 0),
    mm_modality_types TEXT,
    max_response_time_ms INTEGER DEFAULT 2000 CHECK (max_response_time_ms > 0),
    max_memory_usage_mb INTEGER DEFAULT 1024 CHECK (max_memory_usage_mb > 0),
    max_cpu_usage_percent INTEGER DEFAULT 80 CHECK (max_cpu_usage_percent > 0 AND max_cpu_usage_percent <= 100),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'testing'))
);

CREATE INDEX IF NOT EXISTS idx_memory_configurations_user_agent ON memory_configurations (user_id, agent_id);

CREATE TABLE IF NOT EXISTS performance_metrics (
    metric_id TEXT PRIMARY KEY,
    session_id TEXT,
    config_id TEXT NOT NULL REFERENCES memory_configurations(config_id) ON DELETE CASCADE,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    date_hour TEXT,
    response_time_ms INTEGER CHECK (response_time_ms IS NULL OR response_time_ms >= 0),
    memory_usage_mb REAL CHECK (memory_usage_mb IS NULL OR memory_usage_mb >= 0),
    cpu_usage_percent REAL CHECK (cpu_usage_percent IS NULL OR (cpu_usage_percent >= 0 AND cpu_usage_percent <= 100)),
    stm_usage_count INTEGER DEFAULT 0 CHECK (stm_usage_count >= 0),
    ltm_usage_count INTEGER DEFAULT 0 CHECK (ltm_usage_count >= 0),
    kg_usage_count INTEGER DEFAULT 0 CHECK (kg_usage_count >= 0),
    mm_usage_count INTEGER DEFAULT 0 CHECK (mm_usage_count >= 0),
    accuracy_score REAL CHECK (accuracy_score IS NULL OR (accuracy_score >= 0.00 AND accuracy_score <= 1.00)),
    coherence_score REAL CHECK (coherence_score IS NULL OR (coherence_score >= 0.00 AND coherence_score <= 1.00)),
    user_satisfaction REAL CHECK (user_satisfaction IS NULL OR (user_satisfaction >= 0.00 AND user_satisfaction <= 1.00)),
    error_count INTEGER DEFAULT 0 CHECK (error_count >= 0),
    error_types TEXT
);

CREATE INDEX IF NOT EXISTS idx_performance_metrics_config ON performance_metrics (config_id);
CREATE INDEX IF NOT EXISTS idx_performance_metrics_timestamp ON performance_metrics (timestamp);
