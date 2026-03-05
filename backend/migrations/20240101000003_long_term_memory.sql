-- Long-Term Memory (PostgreSQL)

CREATE TABLE IF NOT EXISTS knowledge_entries (
    entry_id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('document', 'api', 'database', 'web', 'user_input')),
    title TEXT,
    content TEXT NOT NULL,
    content_type TEXT NOT NULL CHECK (content_type IN ('text', 'html', 'markdown', 'json', 'structured')),
    content_hash TEXT NOT NULL,
    embedding_vector TEXT NOT NULL,
    embedding_model TEXT NOT NULL,
    embedding_dimension INTEGER NOT NULL CHECK (embedding_dimension > 0),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    last_accessed_at TIMESTAMPTZ,
    category TEXT,
    tags TEXT,
    domain TEXT,
    quality_score REAL DEFAULT 0.50 CHECK (quality_score >= 0.00 AND quality_score <= 1.00),
    relevance_score REAL DEFAULT 0.50 CHECK (relevance_score >= 0.00 AND relevance_score <= 1.00),
    confidence_score REAL DEFAULT 0.50 CHECK (confidence_score >= 0.00 AND confidence_score <= 1.00),
    access_count INTEGER DEFAULT 0 CHECK (access_count >= 0),
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0),
    failure_count INTEGER DEFAULT 0 CHECK (failure_count >= 0),
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'archived', 'deprecated')),
    version INTEGER DEFAULT 1 CHECK (version > 0)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_entries_source ON knowledge_entries (source_id, source_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_created_at ON knowledge_entries (created_at);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_category ON knowledge_entries (category);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_quality_score ON knowledge_entries (quality_score);

CREATE TABLE IF NOT EXISTS knowledge_relations (
    relation_id TEXT PRIMARY KEY,
    source_entry_id TEXT NOT NULL REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    target_entry_id TEXT NOT NULL REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL CHECK (relation_type IN ('similar', 'related', 'contradictory', 'supports', 'refutes', 'extends')),
    relation_strength REAL NOT NULL CHECK (relation_strength >= 0.00 AND relation_strength <= 1.00),
    relation_confidence REAL NOT NULL CHECK (relation_confidence >= 0.00 AND relation_confidence <= 1.00),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT,
    description TEXT,
    UNIQUE (source_entry_id, target_entry_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_relations_source ON knowledge_relations (source_entry_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_target ON knowledge_relations (target_entry_id);
