-- =============================================
-- 记忆管理配置表 (Memory Management)
-- =============================================

-- 1. 记忆配置表 (memory_configurations)
CREATE TABLE memory_configurations (
    -- 主键和标识
    config_id VARCHAR(64) PRIMARY KEY,                 -- 配置唯一标识
    user_id VARCHAR(64) NOT NULL,                      -- 用户标识
    agent_id VARCHAR(64) NOT NULL,                     -- 智能体标识
    
    -- 配置信息
    config_name VARCHAR(200) NOT NULL,                 -- 配置名称
    config_type ENUM('default', 'custom', 'optimized') NOT NULL, -- 配置类型
    
    -- 记忆层配置
    stm_enabled BOOLEAN DEFAULT TRUE,                  -- 短期记忆启用
    stm_max_length INT UNSIGNED DEFAULT 4096,          -- 短期记忆最大长度
    stm_retention_hours INT UNSIGNED DEFAULT 24,       -- 短期记忆保留时间(小时)
    
    ltm_enabled BOOLEAN DEFAULT TRUE,                  -- 长期记忆启用
    ltm_max_entries INT UNSIGNED DEFAULT 10000,        -- 长期记忆最大条目数
    ltm_quality_threshold DECIMAL(3,2) DEFAULT 0.50,   -- 长期记忆质量阈值
    
    kg_enabled BOOLEAN DEFAULT FALSE,                  -- 知识图谱启用
    kg_max_entities INT UNSIGNED DEFAULT 1000,         -- 知识图谱最大实体数
    kg_confidence_threshold DECIMAL(3,2) DEFAULT 0.70, -- 知识图谱置信度阈值
    
    mm_enabled BOOLEAN DEFAULT FALSE,                  -- 多模态记忆启用
    mm_max_entries INT UNSIGNED DEFAULT 1000,          -- 多模态记忆最大条目数
    mm_modality_types JSON,                            -- 支持的模态类型
    
    -- 性能配置
    max_response_time_ms INT UNSIGNED DEFAULT 2000,    -- 最大响应时间(毫秒)
    max_memory_usage_mb INT UNSIGNED DEFAULT 1024,     -- 最大内存使用量(MB)
    max_cpu_usage_percent TINYINT UNSIGNED DEFAULT 80, -- 最大CPU使用率(%)
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 更新时间
    
    -- 状态信息
    status ENUM('active', 'inactive', 'testing') DEFAULT 'active', -- 状态
    
    -- 索引
    INDEX idx_user_agent (user_id, agent_id),
    INDEX idx_config_type (config_type),
    INDEX idx_status (status),
    INDEX idx_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 2. 性能监控表 (performance_metrics)
CREATE TABLE performance_metrics (
    -- 主键和标识
    metric_id VARCHAR(64) PRIMARY KEY,                 -- 指标唯一标识
    session_id VARCHAR(64),                            -- 会话ID
    config_id VARCHAR(64) NOT NULL,                    -- 配置ID
    
    -- 时间信息
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,     -- 时间戳
    date_hour DATETIME GENERATED ALWAYS AS (DATE_FORMAT(timestamp, '%Y-%m-%d %H:00:00')) STORED, -- 小时聚合
    
    -- 性能指标
    response_time_ms INT UNSIGNED,                     -- 响应时间(毫秒)
    memory_usage_mb DECIMAL(8,2),                      -- 内存使用量(MB)
    cpu_usage_percent DECIMAL(5,2),                    -- CPU使用率(%)
    
    -- 记忆层指标
    stm_usage_count INT UNSIGNED DEFAULT 0,            -- 短期记忆使用次数
    ltm_usage_count INT UNSIGNED DEFAULT 0,            -- 长期记忆使用次数
    kg_usage_count INT UNSIGNED DEFAULT 0,             -- 知识图谱使用次数
    mm_usage_count INT UNSIGNED DEFAULT 0,             -- 多模态记忆使用次数
    
    -- 质量指标
    accuracy_score DECIMAL(3,2),                       -- 准确率评分 (0.00-1.00)
    coherence_score DECIMAL(3,2),                      -- 连贯性评分 (0.00-1.00)
    user_satisfaction DECIMAL(3,2),                    -- 用户满意度 (0.00-1.00)
    
    -- 错误信息
    error_count INT UNSIGNED DEFAULT 0,                -- 错误次数
    error_types JSON,                                  -- 错误类型统计
    
    -- 外键约束
    FOREIGN KEY (config_id) REFERENCES memory_configurations(config_id) ON DELETE CASCADE,
    
    -- 索引
    INDEX idx_session (session_id),
    INDEX idx_config (config_id),
    INDEX idx_timestamp (timestamp),
    INDEX idx_date_hour (date_hour),
    INDEX idx_response_time (response_time_ms),
    INDEX idx_memory_usage (memory_usage_mb),
    INDEX idx_cpu_usage (cpu_usage_percent)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
