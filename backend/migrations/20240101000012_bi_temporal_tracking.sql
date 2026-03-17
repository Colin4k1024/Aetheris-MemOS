-- Bi-temporal Tracking for Long-Term Memory and Knowledge Graph
-- Issue #33: Bi-temporal Tracking Engine

-- Add bi-temporal fields to knowledge_entries table
ALTER TABLE knowledge_entries
ADD COLUMN IF NOT EXISTS valid_from TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
ADD COLUMN IF NOT EXISTS valid_until TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS superseded_by TEXT;

-- Add bi-temporal fields to entities table
ALTER TABLE entities
ADD COLUMN IF NOT EXISTS valid_from TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
ADD COLUMN IF NOT EXISTS valid_until TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS superseded_by TEXT;

-- Add bi-temporal fields to relations table
ALTER TABLE relations
ADD COLUMN IF NOT EXISTS valid_from TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
ADD COLUMN IF NOT EXISTS valid_until TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS superseded_by TEXT;

-- Create indexes for bi-temporal queries
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_valid_range
ON knowledge_entries (valid_from, valid_until)
WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_entities_valid_range
ON entities (valid_from, valid_until)
WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_relations_valid_range
ON relations (valid_from, valid_until)
WHERE status = 'active';

-- Create version history table for complete audit trail
CREATE TABLE IF NOT EXISTS knowledge_entry_versions (
    version_id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL,
    version_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    changed_by TEXT,
    change_reason TEXT,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_until TIMESTAMPTZ,
    superseded_by TEXT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_knowledge_entry_versions_entry
ON knowledge_entry_versions (entry_id, version_number);

-- Create entity version history table
CREATE TABLE IF NOT EXISTS entity_versions (
    version_id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    version_number INTEGER NOT NULL,
    entity_name TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    description TEXT,
    changed_by TEXT,
    change_reason TEXT,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_until TIMESTAMPTZ,
    superseded_by TEXT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_entity_versions_entity
ON entity_versions (entity_id, version_number);
