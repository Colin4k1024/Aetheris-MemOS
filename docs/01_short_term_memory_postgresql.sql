-- =============================================
-- 短期记忆存储模型 (Short-Term Memory) - PostgreSQL
-- =============================================

-- 1. 上下文会话表 (context_sessions)
CREATE TABLE context_sessions (
    -- 主键和标识
    session_id VARCHAR(64) PRIMARY KEY,                    -- 会话唯一标识
    user_id VARCHAR(64) NOT NULL,                         -- 用户标识
    agent_id VARCHAR(64) NOT NULL,                        -- 智能体标识
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,       -- 创建时间
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,       -- 更新时间
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,                        -- 过期时间
    
    -- 会话元数据
    session_type VARCHAR(20) NOT NULL CHECK (session_type IN ('conversation', 'task', 'query')), -- 会话类型
    context_length INTEGER DEFAULT 0,                -- 当前上下文长度
    max_context_length INTEGER DEFAULT 4096,         -- 最大上下文长度
    
    -- 状态信息
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'paused', 'completed', 'expired')), -- 会话状态
    priority SMALLINT DEFAULT 5 CHECK (priority >= 1 AND priority <= 10),                  -- 优先级 (1-10)
    
    -- 性能指标
    response_time_ms INTEGER,                        -- 平均响应时间(毫秒)
    memory_usage_bytes BIGINT,                   -- 内存使用量(字节)
    
    -- 约束
    CONSTRAINT chk_context_length CHECK (context_length >= 0),
    CONSTRAINT chk_max_context_length CHECK (max_context_length > 0),
    CONSTRAINT chk_response_time CHECK (response_time_ms IS NULL OR response_time_ms >= 0),
    CONSTRAINT chk_memory_usage CHECK (memory_usage_bytes IS NULL OR memory_usage_bytes >= 0)
);

-- 创建索引
CREATE INDEX idx_context_sessions_user_agent ON context_sessions (user_id, agent_id);
CREATE INDEX idx_context_sessions_created_at ON context_sessions (created_at);
CREATE INDEX idx_context_sessions_expires_at ON context_sessions (expires_at);
CREATE INDEX idx_context_sessions_status ON context_sessions (status);

-- 确保函数存在
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 创建更新时间触发器
CREATE TRIGGER update_context_sessions_updated_at 
    BEFORE UPDATE ON context_sessions 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- 2. 上下文消息表 (context_messages)
CREATE TABLE context_messages (
    -- 主键和标识
    message_id VARCHAR(64) PRIMARY KEY,                   -- 消息唯一标识
    session_id VARCHAR(64) NOT NULL,                     -- 关联会话ID
    message_index INTEGER NOT NULL,                 -- 消息在会话中的序号
    
    -- 消息内容
    role VARCHAR(20) NOT NULL CHECK (role IN ('user', 'assistant', 'system')),   -- 消息角色
    content TEXT NOT NULL,                               -- 消息内容
    content_type VARCHAR(20) DEFAULT 'text' CHECK (content_type IN ('text', 'json', 'markdown')), -- 内容类型
    content_length INTEGER NOT NULL,                -- 内容长度
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,      -- 创建时间
    processed_at TIMESTAMP WITH TIME ZONE,                         -- 处理完成时间
    
    -- 消息元数据
    token_count INTEGER,                            -- Token数量
    embedding_vector JSONB,                               -- 向量表示(JSONB格式)
    metadata JSONB,                                       -- 扩展元数据
    
    -- 状态信息
    is_processed BOOLEAN DEFAULT FALSE,                  -- 是否已处理
    is_important BOOLEAN DEFAULT FALSE,                  -- 是否重要消息
    retention_score DECIMAL(3,2) DEFAULT 0.50 CHECK (retention_score >= 0.00 AND retention_score <= 1.00),          -- 保留评分 (0.00-1.00)
    
    -- 外键约束
    CONSTRAINT fk_context_messages_session_id 
        FOREIGN KEY (session_id) REFERENCES context_sessions(session_id) ON DELETE CASCADE,
    
    -- 约束
    CONSTRAINT chk_message_index CHECK (message_index >= 0),
    CONSTRAINT chk_content_length CHECK (content_length >= 0),
    CONSTRAINT chk_token_count CHECK (token_count IS NULL OR token_count >= 0)
);

-- 创建索引
CREATE INDEX idx_context_messages_session_index ON context_messages (session_id, message_index);
CREATE INDEX idx_context_messages_created_at ON context_messages (created_at);
CREATE INDEX idx_context_messages_role ON context_messages (role);
CREATE INDEX idx_context_messages_processed ON context_messages (is_processed);
CREATE INDEX idx_context_messages_important ON context_messages (is_important);
CREATE INDEX idx_context_messages_retention_score ON context_messages (retention_score);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_context_messages_embedding_vector ON context_messages USING GIN (embedding_vector);
CREATE INDEX idx_context_messages_metadata ON context_messages USING GIN (metadata);

-- 创建全文搜索索引
CREATE INDEX idx_context_messages_content_fts ON context_messages USING GIN (to_tsvector('english', content));
