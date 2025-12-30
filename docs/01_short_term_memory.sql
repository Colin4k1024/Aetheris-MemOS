-- =============================================
-- 短期记忆存储模型 (Short-Term Memory)
-- =============================================

-- 1. 上下文会话表 (context_sessions)
CREATE TABLE context_sessions (
    -- 主键和标识
    session_id VARCHAR(64) PRIMARY KEY,                    -- 会话唯一标识
    user_id VARCHAR(64) NOT NULL,                         -- 用户标识
    agent_id VARCHAR(64) NOT NULL,                        -- 智能体标识
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,       -- 创建时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 更新时间
    expires_at TIMESTAMP NOT NULL,                        -- 过期时间
    
    -- 会话元数据
    session_type ENUM('conversation', 'task', 'query') NOT NULL, -- 会话类型
    context_length INT UNSIGNED DEFAULT 0,                -- 当前上下文长度
    max_context_length INT UNSIGNED DEFAULT 4096,         -- 最大上下文长度
    
    -- 状态信息
    status ENUM('active', 'paused', 'completed', 'expired') DEFAULT 'active', -- 会话状态
    priority TINYINT UNSIGNED DEFAULT 5,                  -- 优先级 (1-10)
    
    -- 性能指标
    response_time_ms INT UNSIGNED,                        -- 平均响应时间(毫秒)
    memory_usage_bytes BIGINT UNSIGNED,                   -- 内存使用量(字节)
    
    -- 索引
    INDEX idx_user_agent (user_id, agent_id),
    INDEX idx_created_at (created_at),
    INDEX idx_expires_at (expires_at),
    INDEX idx_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 2. 上下文消息表 (context_messages)
CREATE TABLE context_messages (
    -- 主键和标识
    message_id VARCHAR(64) PRIMARY KEY,                   -- 消息唯一标识
    session_id VARCHAR(64) NOT NULL,                     -- 关联会话ID
    message_index INT UNSIGNED NOT NULL,                 -- 消息在会话中的序号
    
    -- 消息内容
    role ENUM('user', 'assistant', 'system') NOT NULL,   -- 消息角色
    content TEXT NOT NULL,                               -- 消息内容
    content_type ENUM('text', 'json', 'markdown') DEFAULT 'text', -- 内容类型
    content_length INT UNSIGNED NOT NULL,                -- 内容长度
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,      -- 创建时间
    processed_at TIMESTAMP NULL,                         -- 处理完成时间
    
    -- 消息元数据
    token_count INT UNSIGNED,                            -- Token数量
    embedding_vector JSON,                               -- 向量表示(JSON格式)
    metadata JSON,                                       -- 扩展元数据
    
    -- 状态信息
    is_processed BOOLEAN DEFAULT FALSE,                  -- 是否已处理
    is_important BOOLEAN DEFAULT FALSE,                  -- 是否重要消息
    retention_score DECIMAL(3,2) DEFAULT 0.50,          -- 保留评分 (0.00-1.00)
    
    -- 外键约束
    FOREIGN KEY (session_id) REFERENCES context_sessions(session_id) ON DELETE CASCADE,
    
    -- 索引
    INDEX idx_session_index (session_id, message_index),
    INDEX idx_created_at (created_at),
    INDEX idx_role (role),
    INDEX idx_processed (is_processed),
    INDEX idx_important (is_important),
    INDEX idx_retention_score (retention_score)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

