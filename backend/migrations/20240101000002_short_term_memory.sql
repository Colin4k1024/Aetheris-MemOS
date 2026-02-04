-- =============================================
-- 短期记忆存储模型 (Short-Term Memory) - SQLite
-- =============================================

-- 启用外键约束
PRAGMA foreign_keys = ON;

-- 1. 上下文会话表 (context_sessions)
CREATE TABLE IF NOT EXISTS context_sessions (
    -- 主键和标识
    session_id TEXT PRIMARY KEY,                    -- 会话唯一标识
    user_id TEXT NOT NULL,                         -- 用户标识
    agent_id TEXT NOT NULL,                        -- 智能体标识
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),       -- 创建时间
    updated_at TEXT DEFAULT (datetime('now')),      -- 更新时间
    expires_at TEXT NOT NULL,                        -- 过期时间
    
    -- 会话元数据
    session_type TEXT NOT NULL CHECK (session_type IN ('conversation', 'task', 'query')), -- 会话类型
    context_length INTEGER DEFAULT 0 CHECK (context_length >= 0),                -- 当前上下文长度
    max_context_length INTEGER DEFAULT 4096 CHECK (max_context_length > 0),         -- 最大上下文长度
    
    -- 状态信息
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'paused', 'completed', 'expired')), -- 会话状态
    priority INTEGER DEFAULT 5 CHECK (priority >= 1 AND priority <= 10),                  -- 优先级 (1-10)
    
    -- 性能指标
    response_time_ms INTEGER CHECK (response_time_ms IS NULL OR response_time_ms >= 0),                        -- 平均响应时间(毫秒)
    memory_usage_bytes INTEGER CHECK (memory_usage_bytes IS NULL OR memory_usage_bytes >= 0)                   -- 内存使用量(字节)
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_context_sessions_user_agent ON context_sessions (user_id, agent_id);
CREATE INDEX IF NOT EXISTS idx_context_sessions_created_at ON context_sessions (created_at);
CREATE INDEX IF NOT EXISTS idx_context_sessions_expires_at ON context_sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_context_sessions_status ON context_sessions (status);

-- 创建更新时间触发器
CREATE TRIGGER IF NOT EXISTS update_context_sessions_updated_at 
    AFTER UPDATE ON context_sessions
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE context_sessions SET updated_at = datetime('now') WHERE session_id = NEW.session_id;
END;

-- 2. 上下文消息表 (context_messages)
CREATE TABLE IF NOT EXISTS context_messages (
    -- 主键和标识
    message_id TEXT PRIMARY KEY,                   -- 消息唯一标识
    session_id TEXT NOT NULL,                     -- 关联会话ID
    message_index INTEGER NOT NULL CHECK (message_index >= 0),                 -- 消息在会话中的序号
    
    -- 消息内容
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),   -- 消息角色
    content TEXT NOT NULL,                               -- 消息内容
    content_type TEXT DEFAULT 'text' CHECK (content_type IN ('text', 'json', 'markdown')), -- 内容类型
    content_length INTEGER NOT NULL CHECK (content_length >= 0),                -- 内容长度
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),      -- 创建时间
    processed_at TEXT,                         -- 处理完成时间
    
    -- 消息元数据
    token_count INTEGER CHECK (token_count IS NULL OR token_count >= 0),                            -- Token数量
    embedding_vector TEXT,                               -- 向量表示(JSON格式，存储为TEXT)
    metadata TEXT,                                       -- 扩展元数据(JSON格式，存储为TEXT)
    
    -- 状态信息
    is_processed INTEGER DEFAULT 0 CHECK (is_processed IN (0, 1)),                  -- 是否已处理 (0=false, 1=true)
    is_important INTEGER DEFAULT 0 CHECK (is_important IN (0, 1)),                  -- 是否重要消息 (0=false, 1=true)
    retention_score REAL DEFAULT 0.50 CHECK (retention_score >= 0.00 AND retention_score <= 1.00),          -- 保留评分 (0.00-1.00)
    
    -- 外键约束
    FOREIGN KEY (session_id) REFERENCES context_sessions(session_id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_context_messages_session_index ON context_messages (session_id, message_index);
CREATE INDEX IF NOT EXISTS idx_context_messages_created_at ON context_messages (created_at);
CREATE INDEX IF NOT EXISTS idx_context_messages_role ON context_messages (role);
CREATE INDEX IF NOT EXISTS idx_context_messages_processed ON context_messages (is_processed);
CREATE INDEX IF NOT EXISTS idx_context_messages_important ON context_messages (is_important);
CREATE INDEX IF NOT EXISTS idx_context_messages_retention_score ON context_messages (retention_score);

