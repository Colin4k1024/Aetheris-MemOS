-- Multi-Modal Memory (PostgreSQL)

CREATE TABLE IF NOT EXISTS multimodal_entries (
    entry_id TEXT PRIMARY KEY,
    session_id TEXT,
    source_id TEXT NOT NULL,
    modality_type TEXT NOT NULL CHECK (modality_type IN ('text', 'image', 'audio', 'video', 'mixed')),
    modality_count INTEGER DEFAULT 1 CHECK (modality_count > 0),
    title TEXT,
    description TEXT,
    content_metadata TEXT NOT NULL,
    text_content TEXT,
    text_embedding TEXT,
    image_url TEXT,
    image_embedding TEXT,
    image_features TEXT,
    audio_url TEXT,
    audio_embedding TEXT,
    audio_transcript TEXT,
    audio_features TEXT,
    video_url TEXT,
    video_embedding TEXT,
    video_transcript TEXT,
    video_features TEXT,
    cross_modal_alignment TEXT,
    unified_embedding TEXT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    quality_score REAL DEFAULT 0.50 CHECK (quality_score >= 0.00 AND quality_score <= 1.00),
    modality_consistency REAL DEFAULT 0.50 CHECK (modality_consistency >= 0.00 AND modality_consistency <= 1.00),
    access_count INTEGER DEFAULT 0 CHECK (access_count >= 0),
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0),
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'processing', 'error', 'archived'))
);

CREATE INDEX IF NOT EXISTS idx_multimodal_entries_session ON multimodal_entries (session_id);
CREATE INDEX IF NOT EXISTS idx_multimodal_entries_modality_type ON multimodal_entries (modality_type);

CREATE TABLE IF NOT EXISTS modality_relations (
    relation_id TEXT PRIMARY KEY,
    source_entry_id TEXT NOT NULL REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    target_entry_id TEXT NOT NULL REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL CHECK (relation_type IN ('temporal', 'spatial', 'semantic', 'causal', 'similar')),
    relation_strength REAL NOT NULL CHECK (relation_strength >= 0.00 AND relation_strength <= 1.00),
    relation_confidence REAL NOT NULL CHECK (relation_confidence >= 0.00 AND relation_confidence <= 1.00),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT,
    description TEXT,
    UNIQUE (source_entry_id, target_entry_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_modality_relations_source ON modality_relations (source_entry_id);
CREATE INDEX IF NOT EXISTS idx_modality_relations_target ON modality_relations (target_entry_id);
