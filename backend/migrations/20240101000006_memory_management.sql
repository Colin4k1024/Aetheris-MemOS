-- =============================================
-- 记忆管理配置表 (Memory Management) - SQLite
-- =============================================

-- 启用外键约束
PRAGMA foreign_keys = ON;

-- 1. 记忆配置表 (memory_configurations)
CREATE TABLE IF NOT EXISTS memory_configurations (
    -- 主键和标识
    config_id TEXT PRIMARY KEY,                 -- 配置唯一标识
    user_id TEXT NOT NULL,                      -- 用户标识
    agent_id TEXT NOT NULL,                     -- 智能体标识
    
    -- 配置信息
    config_name TEXT NOT NULL,                 -- 配置名称
    config_type TEXT NOT NULL CHECK (config_type IN ('default', 'custom', 'optimized')), -- 配置类型
    
    -- 记忆层配置
    stm_enabled INTEGER DEFAULT 1 CHECK (stm_enabled IN (0, 1)),                  -- 短期记忆启用 (0=false, 1=true)
    stm_max_length INTEGER DEFAULT 4096 CHECK (stm_max_length > 0),          -- 短期记忆最大长度
    stm_retention_hours INTEGER DEFAULT 24 CHECK (stm_retention_hours > 0),       -- 短期记忆保留时间(小时)
    
    ltm_enabled INTEGER DEFAULT 1 CHECK (ltm_enabled IN (0, 1)),                  -- 长期记忆启用 (0=false, 1=true)
    ltm_max_entries INTEGER DEFAULT 10000 CHECK (ltm_max_entries > 0),        -- 长期记忆最大条目数
    ltm_quality_threshold REAL DEFAULT 0.50 CHECK (ltm_quality_threshold >= 0.00 AND ltm_quality_threshold <= 1.00),   -- 长期记忆质量阈值
    
    kg_enabled INTEGER DEFAULT 0 CHECK (kg_enabled IN (0, 1)),                  -- 知识图谱启用 (0=false, 1=true)
    kg_max_entities INTEGER DEFAULT 1000 CHECK (kg_max_entities > 0),         -- 知识图谱最大实体数
    kg_confidence_threshold REAL DEFAULT 0.70 CHECK (kg_confidence_threshold >= 0.00 AND kg_confidence_threshold <= 1.00), -- 知识图谱置信度阈值
    
    mm_enabled INTEGER DEFAULT 0 CHECK (mm_enabled IN (0, 1)),                  -- 多模态记忆启用 (0=false, 1=true)
    mm_max_entries INTEGER DEFAULT 1000 CHECK (mm_max_entries > 0),          -- 多模态记忆最大条目数
    mm_modality_types TEXT,                            -- 支持的模态类型(JSON格式，存储为TEXT)
    
    -- 性能配置
    max_response_time_ms INTEGER DEFAULT 2000 CHECK (max_response_time_ms > 0),    -- 最大响应时间(毫秒)
    max_memory_usage_mb INTEGER DEFAULT 1024 CHECK (max_memory_usage_mb > 0),     -- 最大内存使用量(MB)
    max_cpu_usage_percent INTEGER DEFAULT 80 CHECK (max_cpu_usage_percent > 0 AND max_cpu_usage_percent <= 100), -- 最大CPU使用率(%)
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),    -- 创建时间
    updated_at TEXT DEFAULT (datetime('now')),    -- 更新时间
    
    -- 状态信息
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'testing')) -- 状态
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_memory_configurations_user_agent ON memory_configurations (user_id, agent_id);
CREATE INDEX IF NOT EXISTS idx_memory_configurations_type ON memory_configurations (config_type);
CREATE INDEX IF NOT EXISTS idx_memory_configurations_status ON memory_configurations (status);
CREATE INDEX IF NOT EXISTS idx_memory_configurations_created_at ON memory_configurations (created_at);

-- 创建更新时间触发器
CREATE TRIGGER IF NOT EXISTS update_memory_configurations_updated_at 
    AFTER UPDATE ON memory_configurations
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE memory_configurations SET updated_at = datetime('now') WHERE config_id = NEW.config_id;
END;

-- 2. 性能监控表 (performance_metrics)
CREATE TABLE IF NOT EXISTS performance_metrics (
    -- 主键和标识
    metric_id TEXT PRIMARY KEY,                 -- 指标唯一标识
    session_id TEXT,                            -- 会话ID
    config_id TEXT NOT NULL,                    -- 配置ID
    
    -- 时间信息
    timestamp TEXT DEFAULT (datetime('now')),     -- 时间戳
    date_hour TEXT, -- 小时聚合（通过触发器更新）
    
    -- 性能指标
    response_time_ms INTEGER CHECK (response_time_ms IS NULL OR response_time_ms >= 0),                     -- 响应时间(毫秒)
    memory_usage_mb REAL CHECK (memory_usage_mb IS NULL OR memory_usage_mb >= 0),                      -- 内存使用量(MB)
    cpu_usage_percent REAL CHECK (cpu_usage_percent IS NULL OR (cpu_usage_percent >= 0 AND cpu_usage_percent <= 100)),                    -- CPU使用率(%)
    
    -- 记忆层指标
    stm_usage_count INTEGER DEFAULT 0 CHECK (stm_usage_count >= 0),            -- 短期记忆使用次数
    ltm_usage_count INTEGER DEFAULT 0 CHECK (ltm_usage_count >= 0),            -- 长期记忆使用次数
    kg_usage_count INTEGER DEFAULT 0 CHECK (kg_usage_count >= 0),             -- 知识图谱使用次数
    mm_usage_count INTEGER DEFAULT 0 CHECK (mm_usage_count >= 0),             -- 多模态记忆使用次数
    
    -- 质量指标
    accuracy_score REAL CHECK (accuracy_score IS NULL OR (accuracy_score >= 0.00 AND accuracy_score <= 1.00)),                       -- 准确率评分 (0.00-1.00)
    coherence_score REAL CHECK (coherence_score IS NULL OR (coherence_score >= 0.00 AND coherence_score <= 1.00)),                      -- 连贯性评分 (0.00-1.00)
    user_satisfaction REAL CHECK (user_satisfaction IS NULL OR (user_satisfaction >= 0.00 AND user_satisfaction <= 1.00)),                    -- 用户满意度 (0.00-1.00)
    
    -- 错误信息
    error_count INTEGER DEFAULT 0 CHECK (error_count >= 0),                -- 错误次数
    error_types TEXT,                                  -- 错误类型统计(JSON格式，存储为TEXT)
    
    -- 外键约束
    FOREIGN KEY (config_id) REFERENCES memory_configurations(config_id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_performance_metrics_session ON performance_metrics (session_id);
CREATE INDEX IF NOT EXISTS idx_performance_metrics_config ON performance_metrics (config_id);
CREATE INDEX IF NOT EXISTS idx_performance_metrics_timestamp ON performance_metrics (timestamp);
CREATE INDEX IF NOT EXISTS idx_performance_metrics_date_hour ON performance_metrics (date_hour);
CREATE INDEX IF NOT EXISTS idx_performance_metrics_response_time ON performance_metrics (response_time_ms);
CREATE INDEX IF NOT EXISTS idx_performance_metrics_memory_usage ON performance_metrics (memory_usage_mb);
CREATE INDEX IF NOT EXISTS idx_performance_metrics_cpu_usage ON performance_metrics (cpu_usage_percent);

-- 创建自动更新 date_hour 字段的触发器
CREATE TRIGGER IF NOT EXISTS update_performance_metrics_date_hour 
    AFTER INSERT ON performance_metrics
    FOR EACH ROW
    WHEN NEW.date_hour IS NULL
BEGIN
    UPDATE performance_metrics 
    SET date_hour = strftime('%Y-%m-%d %H:00:00', NEW.timestamp)
    WHERE metric_id = NEW.metric_id;
END;

CREATE TRIGGER IF NOT EXISTS update_performance_metrics_date_hour_update
    AFTER UPDATE OF timestamp ON performance_metrics
    FOR EACH ROW
BEGIN
    UPDATE performance_metrics 
    SET date_hour = strftime('%Y-%m-%d %H:00:00', NEW.timestamp)
    WHERE metric_id = NEW.metric_id;
END;

