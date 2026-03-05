-- Knowledge Graph (PostgreSQL)

CREATE TABLE IF NOT EXISTS entities (
    entity_id TEXT PRIMARY KEY,
    entity_name TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    description TEXT,
    attributes TEXT,
    aliases TEXT,
    embedding_vector TEXT,
    embedding_model TEXT,
    embedding_dimension INTEGER CHECK (embedding_dimension IS NULL OR embedding_dimension > 0),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    confidence_score REAL DEFAULT 0.50 CHECK (confidence_score >= 0.00 AND confidence_score <= 1.00),
    popularity_score REAL DEFAULT 0.50 CHECK (popularity_score >= 0.00 AND popularity_score <= 1.00),
    relation_count INTEGER DEFAULT 0 CHECK (relation_count >= 0),
    mention_count INTEGER DEFAULT 0 CHECK (mention_count >= 0),
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'merged', 'deprecated'))
);

CREATE INDEX IF NOT EXISTS idx_entities_entity_type ON entities (entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_entity_name ON entities (entity_name);

CREATE TABLE IF NOT EXISTS relations (
    relation_id TEXT PRIMARY KEY,
    source_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    target_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    relation_name TEXT,
    description TEXT,
    properties TEXT,
    weight REAL DEFAULT 1.0000 CHECK (weight >= 0.0000 AND weight <= 1.0000),
    confidence REAL NOT NULL CHECK (confidence >= 0.00 AND confidence <= 1.00),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    usage_count INTEGER DEFAULT 0 CHECK (usage_count >= 0),
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0),
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'deprecated')),
    UNIQUE (source_entity_id, target_entity_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_relations_source ON relations (source_entity_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations (target_entity_id);

CREATE TABLE IF NOT EXISTS reasoning_paths (
    path_id TEXT PRIMARY KEY,
    source_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    target_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    path_length INTEGER NOT NULL CHECK (path_length > 0),
    path_entities TEXT NOT NULL,
    path_relations TEXT NOT NULL,
    reasoning_type TEXT NOT NULL CHECK (reasoning_type IN ('deduction', 'induction', 'abduction', 'analogy')),
    reasoning_strength REAL NOT NULL CHECK (reasoning_strength >= 0.00 AND reasoning_strength <= 1.00),
    reasoning_confidence REAL NOT NULL CHECK (reasoning_confidence >= 0.00 AND reasoning_confidence <= 1.00),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMPTZ,
    usage_count INTEGER DEFAULT 0 CHECK (usage_count >= 0),
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0)
);

CREATE INDEX IF NOT EXISTS idx_reasoning_paths_source ON reasoning_paths (source_entity_id);
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_target ON reasoning_paths (target_entity_id);
