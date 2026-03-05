-- Short-Term Memory (PostgreSQL)

-- 1. context_sessions
CREATE TABLE IF NOT EXISTS context_sessions (
    session_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    session_type TEXT NOT NULL CHECK (session_type IN ('conversation', 'task', 'query')),
    context_length INTEGER DEFAULT 0 CHECK (context_length >= 0),
    max_context_length INTEGER DEFAULT 4096 CHECK (max_context_length > 0),
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'paused', 'completed', 'expired')),
    priority INTEGER DEFAULT 5 CHECK (priority >= 1 AND priority <= 10),
    response_time_ms INTEGER CHECK (response_time_ms IS NULL OR response_time_ms >= 0),
    memory_usage_bytes INTEGER CHECK (memory_usage_bytes IS NULL OR memory_usage_bytes >= 0)
);

CREATE INDEX IF NOT EXISTS idx_context_sessions_user_agent ON context_sessions (user_id, agent_id);
CREATE INDEX IF NOT EXISTS idx_context_sessions_created_at ON context_sessions (created_at);
CREATE INDEX IF NOT EXISTS idx_context_sessions_expires_at ON context_sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_context_sessions_status ON context_sessions (status);

-- 2. context_messages
CREATE TABLE IF NOT EXISTS context_messages (
    message_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES context_sessions(session_id) ON DELETE CASCADE,
    message_index INTEGER NOT NULL CHECK (message_index >= 0),
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    content_type TEXT DEFAULT 'text' CHECK (content_type IN ('text', 'json', 'markdown')),
    content_length INTEGER NOT NULL CHECK (content_length >= 0),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMPTZ,
    token_count INTEGER CHECK (token_count IS NULL OR token_count >= 0),
    embedding_vector TEXT,
    metadata TEXT,
    is_processed SMALLINT DEFAULT 0 CHECK (is_processed IN (0, 1)),
    is_important SMALLINT DEFAULT 0 CHECK (is_important IN (0, 1)),
    retention_score REAL DEFAULT 0.50 CHECK (retention_score >= 0.00 AND retention_score <= 1.00)
);

CREATE INDEX IF NOT EXISTS idx_context_messages_session_index ON context_messages (session_id, message_index);
CREATE INDEX IF NOT EXISTS idx_context_messages_created_at ON context_messages (created_at);
CREATE INDEX IF NOT EXISTS idx_context_messages_role ON context_messages (role);

-- 3. session_messages (app-facing view / table used by STMRepository)
CREATE TABLE IF NOT EXISTS session_messages (
    message_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES context_sessions(session_id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    token_count INTEGER,
    importance_score REAL
);

CREATE INDEX IF NOT EXISTS idx_session_messages_session_id ON session_messages (session_id);
