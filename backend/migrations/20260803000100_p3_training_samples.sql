-- P3a: Training samples table (ADR-0008 §2)
--
-- Append-only store for feature vectors + labels used by the offline batch trainer.
-- Tenant-scoped (RLS) so training data never leaks across tenants.
-- policy_tag distinguishes exploration samples from normal operation.
-- split_tag is set during train/val/test splitting to prevent data leakage.

CREATE TABLE IF NOT EXISTS training_samples (
    sample_id       TEXT PRIMARY KEY,
    tenant_id       TEXT NOT NULL,
    task_id         TEXT NOT NULL,
    config_id       TEXT NOT NULL,
    -- Feature vector (JSON): complexity, modality_count, temporal_scope,
    -- reasoning_depth, context_dependency, resource_cpu, resource_memory,
    -- response_time_p50, response_time_p95, memory_weights, etc.
    features_json   TEXT NOT NULL DEFAULT '{}',
    -- Label vector (JSON): accuracy, coherence, latency_ms, cost,
    -- user_satisfaction (optional). Must come from independent measurement
    -- (LLM-judge / task oracle / OTel), NOT from predictor output.
    labels_json     TEXT NOT NULL DEFAULT '{}',
    -- Whether this sample was collected under exploration policy (ε-random).
    -- Exploration samples are unbiased; normal samples have selection bias.
    policy_tag      TEXT NOT NULL DEFAULT 'normal' CHECK (policy_tag IN ('normal', 'exploration')),
    -- Train/val/test split assignment (set during offline splitting, not at collection).
    split_tag       TEXT CHECK (split_tag IN ('train', 'val', 'test', NULL)),
    collected_at    TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- RLS: tenant isolation (same pattern as knowledge_entries / context_sessions).
ALTER TABLE training_samples ENABLE ROW LEVEL SECURITY;
ALTER TABLE training_samples FORCE ROW LEVEL SECURITY;

CREATE POLICY training_samples_tenant_isolation ON training_samples
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );

-- Indexes for training pipeline queries.
CREATE INDEX IF NOT EXISTS idx_training_samples_tenant_collected
    ON training_samples (tenant_id, collected_at);

CREATE INDEX IF NOT EXISTS idx_training_samples_split
    ON training_samples (split_tag, tenant_id);

CREATE INDEX IF NOT EXISTS idx_training_samples_policy
    ON training_samples (policy_tag, tenant_id);

-- Model registry: versioned model artifacts with lifecycle status.
-- Predictor loads the 'active' version; shadow/canary versions are compared
-- before promotion.
CREATE TABLE IF NOT EXISTS model_versions (
    version         TEXT PRIMARY KEY,
    model_type      TEXT NOT NULL DEFAULT 'linear' CHECK (model_type IN ('linear', 'gbdt', 'ensemble')),
    artifact_uri    TEXT NOT NULL,
    -- Model card (JSON): feature list, training metrics (R², MAE, win-rate),
    -- training data window, limitations, calibration info.
    metrics_json    TEXT NOT NULL DEFAULT '{}',
    feature_names   TEXT NOT NULL DEFAULT '[]',
    trained_at      TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status          TEXT NOT NULL DEFAULT 'shadow' CHECK (status IN ('shadow', 'canary', 'active', 'rolled_back')),
    promoted_at     TIMESTAMPTZ,
    rolled_back_at  TIMESTAMPTZ,
    notes           TEXT
);

-- Only one model can be 'active' at a time (enforced by application layer,
-- not a DB constraint, to allow emergency multi-version rollback).
CREATE INDEX IF NOT EXISTS idx_model_versions_status
    ON model_versions (status, trained_at);

-- Config archetype definitions for the candidate configuration space.
-- scheduler uses these to generate candidates for argmax selection.
CREATE TABLE IF NOT EXISTS config_archetypes (
    archetype_id    TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT,
    -- JSON: {stm_weight, ltm_weight, kg_weight, mm_weight, reasoning_depth, enable_multimodal, primary_memory, secondary_memory}
    config_json     TEXT NOT NULL DEFAULT '{}',
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Seed default archetypes (8 configurations covering the common space).
INSERT INTO config_archetypes (archetype_id, name, description, config_json) VALUES
    ('stm-only', 'STM Only', 'Short-term memory only, no external layers',
     '{"stm_weight":1.0,"ltm_weight":0.0,"kg_weight":0.0,"mm_weight":0.0,"reasoning_depth":"shallow","enable_multimodal":false,"primary_memory":"stm","secondary_memory":[]}'),
    ('stm-ltm', 'STM + LTM', 'Short + long-term memory, no knowledge graph',
     '{"stm_weight":0.5,"ltm_weight":0.5,"kg_weight":0.0,"mm_weight":0.0,"reasoning_depth":"medium","enable_multimodal":false,"primary_memory":"stm","secondary_memory":["ltm"]}'),
    ('stm-ltm-kg', 'STM + LTM + KG', 'Full text memory with knowledge graph',
     '{"stm_weight":0.3,"ltm_weight":0.4,"kg_weight":0.3,"mm_weight":0.0,"reasoning_depth":"deep","enable_multimodal":false,"primary_memory":"stm","secondary_memory":["ltm","kg"]}'),
    ('full-stack', 'Full Stack', 'All layers enabled',
     '{"stm_weight":0.25,"ltm_weight":0.25,"kg_weight":0.25,"mm_weight":0.25,"reasoning_depth":"deep","enable_multimodal":true,"primary_memory":"stm","secondary_memory":["ltm","kg","mm"]}'),
    ('ltm-heavy', 'LTM Heavy', 'Long-term memory dominant for factual tasks',
     '{"stm_weight":0.2,"ltm_weight":0.6,"kg_weight":0.1,"mm_weight":0.1,"reasoning_depth":"medium","enable_multimodal":false,"primary_memory":"ltm","secondary_memory":["stm","kg"]}'),
    ('kg-heavy', 'KG Heavy', 'Knowledge graph dominant for reasoning tasks',
     '{"stm_weight":0.1,"ltm_weight":0.2,"kg_weight":0.6,"mm_weight":0.1,"reasoning_depth":"deep","enable_multimodal":false,"primary_memory":"kg","secondary_memory":["stm","ltm"]}'),
    ('efficiency', 'Efficiency', 'Minimal layers for fast responses',
     '{"stm_weight":0.7,"ltm_weight":0.3,"kg_weight":0.0,"mm_weight":0.0,"reasoning_depth":"shallow","enable_multimodal":false,"primary_memory":"stm","secondary_memory":["ltm"]}'),
    ('multimodal', 'Multimodal', 'Text + multimodal for rich content tasks',
     '{"stm_weight":0.2,"ltm_weight":0.3,"kg_weight":0.1,"mm_weight":0.4,"reasoning_depth":"medium","enable_multimodal":true,"primary_memory":"stm","secondary_memory":["ltm","kg","mm"]}')
ON CONFLICT (archetype_id) DO NOTHING;
