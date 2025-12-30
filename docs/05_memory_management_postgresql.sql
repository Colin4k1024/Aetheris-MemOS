-- =============================================
-- 记忆管理配置表 (Memory Management) - PostgreSQL
-- =============================================

-- 1. 记忆配置表 (memory_configurations)
CREATE TABLE memory_configurations (
    -- 主键和标识
    config_id VARCHAR(64) PRIMARY KEY,                 -- 配置唯一标识
    user_id VARCHAR(64) NOT NULL,                      -- 用户标识
    agent_id VARCHAR(64) NOT NULL,                     -- 智能体标识
    
    -- 配置信息
    config_name VARCHAR(200) NOT NULL,                 -- 配置名称
    config_type VARCHAR(20) NOT NULL CHECK (config_type IN ('default', 'custom', 'optimized')), -- 配置类型
    
    -- 记忆层配置
    stm_enabled BOOLEAN DEFAULT TRUE,                  -- 短期记忆启用
    stm_max_length INTEGER DEFAULT 4096,          -- 短期记忆最大长度
    stm_retention_hours INTEGER DEFAULT 24,       -- 短期记忆保留时间(小时)
    
    ltm_enabled BOOLEAN DEFAULT TRUE,                  -- 长期记忆启用
    ltm_max_entries INTEGER DEFAULT 10000,        -- 长期记忆最大条目数
    ltm_quality_threshold DECIMAL(3,2) DEFAULT 0.50 CHECK (ltm_quality_threshold >= 0.00 AND ltm_quality_threshold <= 1.00),   -- 长期记忆质量阈值
    
    kg_enabled BOOLEAN DEFAULT FALSE,                  -- 知识图谱启用
    kg_max_entities INTEGER DEFAULT 1000,         -- 知识图谱最大实体数
    kg_confidence_threshold DECIMAL(3,2) DEFAULT 0.70 CHECK (kg_confidence_threshold >= 0.00 AND kg_confidence_threshold <= 1.00), -- 知识图谱置信度阈值
    
    mm_enabled BOOLEAN DEFAULT FALSE,                  -- 多模态记忆启用
    mm_max_entries INTEGER DEFAULT 1000,          -- 多模态记忆最大条目数
    mm_modality_types JSONB,                            -- 支持的模态类型
    
    -- 性能配置
    max_response_time_ms INTEGER DEFAULT 2000,    -- 最大响应时间(毫秒)
    max_memory_usage_mb INTEGER DEFAULT 1024,     -- 最大内存使用量(MB)
    max_cpu_usage_percent SMALLINT DEFAULT 80, -- 最大CPU使用率(%)
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 更新时间
    
    -- 状态信息
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'testing')), -- 状态
    
    -- 约束
    CONSTRAINT chk_stm_max_length CHECK (stm_max_length > 0),
    CONSTRAINT chk_stm_retention_hours CHECK (stm_retention_hours > 0),
    CONSTRAINT chk_ltm_max_entries CHECK (ltm_max_entries > 0),
    CONSTRAINT chk_kg_max_entities CHECK (kg_max_entities > 0),
    CONSTRAINT chk_mm_max_entries CHECK (mm_max_entries > 0),
    CONSTRAINT chk_max_response_time CHECK (max_response_time_ms > 0),
    CONSTRAINT chk_max_memory_usage CHECK (max_memory_usage_mb > 0),
    CONSTRAINT chk_max_cpu_usage CHECK (max_cpu_usage_percent > 0 AND max_cpu_usage_percent <= 100)
);

-- 创建索引
CREATE INDEX idx_memory_configurations_user_agent ON memory_configurations (user_id, agent_id);
CREATE INDEX idx_memory_configurations_type ON memory_configurations (config_type);
CREATE INDEX idx_memory_configurations_status ON memory_configurations (status);
CREATE INDEX idx_memory_configurations_created_at ON memory_configurations (created_at);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_memory_configurations_modality_types ON memory_configurations USING GIN (mm_modality_types);

-- 创建更新时间触发器
CREATE TRIGGER update_memory_configurations_updated_at 
    BEFORE UPDATE ON memory_configurations 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- 2. 性能监控表 (performance_metrics)
CREATE TABLE performance_metrics (
    -- 主键和标识
    metric_id VARCHAR(64) PRIMARY KEY,                 -- 指标唯一标识
    session_id VARCHAR(64),                            -- 会话ID
    config_id VARCHAR(64) NOT NULL,                    -- 配置ID
    
    -- 时间信息
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,     -- 时间戳
    date_hour TIMESTAMP WITH TIME ZONE, -- 小时聚合（通过触发器更新）
    
    -- 性能指标
    response_time_ms INTEGER,                     -- 响应时间(毫秒)
    memory_usage_mb DECIMAL(8,2),                      -- 内存使用量(MB)
    cpu_usage_percent DECIMAL(5,2),                    -- CPU使用率(%)
    
    -- 记忆层指标
    stm_usage_count INTEGER DEFAULT 0,            -- 短期记忆使用次数
    ltm_usage_count INTEGER DEFAULT 0,            -- 长期记忆使用次数
    kg_usage_count INTEGER DEFAULT 0,             -- 知识图谱使用次数
    mm_usage_count INTEGER DEFAULT 0,             -- 多模态记忆使用次数
    
    -- 质量指标
    accuracy_score DECIMAL(3,2),                       -- 准确率评分 (0.00-1.00)
    coherence_score DECIMAL(3,2),                      -- 连贯性评分 (0.00-1.00)
    user_satisfaction DECIMAL(3,2),                    -- 用户满意度 (0.00-1.00)
    
    -- 错误信息
    error_count INTEGER DEFAULT 0,                -- 错误次数
    error_types JSONB,                                  -- 错误类型统计
    
    -- 外键约束
    CONSTRAINT fk_performance_metrics_config 
        FOREIGN KEY (config_id) REFERENCES memory_configurations(config_id) ON DELETE CASCADE,
    
    -- 约束
    CONSTRAINT chk_response_time CHECK (response_time_ms IS NULL OR response_time_ms >= 0),
    CONSTRAINT chk_memory_usage CHECK (memory_usage_mb IS NULL OR memory_usage_mb >= 0),
    CONSTRAINT chk_cpu_usage CHECK (cpu_usage_percent IS NULL OR (cpu_usage_percent >= 0 AND cpu_usage_percent <= 100)),
    CONSTRAINT chk_usage_counts CHECK (
        stm_usage_count >= 0 AND 
        ltm_usage_count >= 0 AND 
        kg_usage_count >= 0 AND 
        mm_usage_count >= 0
    ),
    CONSTRAINT chk_quality_scores CHECK (
        (accuracy_score IS NULL OR (accuracy_score >= 0.00 AND accuracy_score <= 1.00)) AND
        (coherence_score IS NULL OR (coherence_score >= 0.00 AND coherence_score <= 1.00)) AND
        (user_satisfaction IS NULL OR (user_satisfaction >= 0.00 AND user_satisfaction <= 1.00))
    ),
    CONSTRAINT chk_error_count CHECK (error_count >= 0)
);

-- 创建索引
CREATE INDEX idx_performance_metrics_session ON performance_metrics (session_id);
CREATE INDEX idx_performance_metrics_config ON performance_metrics (config_id);
CREATE INDEX idx_performance_metrics_timestamp ON performance_metrics (timestamp);
CREATE INDEX idx_performance_metrics_date_hour ON performance_metrics (date_hour);
CREATE INDEX idx_performance_metrics_response_time ON performance_metrics (response_time_ms);
CREATE INDEX idx_performance_metrics_memory_usage ON performance_metrics (memory_usage_mb);
CREATE INDEX idx_performance_metrics_cpu_usage ON performance_metrics (cpu_usage_percent);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_performance_metrics_error_types ON performance_metrics USING GIN (error_types);

-- 创建自动更新 date_hour 字段的触发器
CREATE OR REPLACE FUNCTION update_date_hour_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.date_hour = date_trunc('hour', NEW.timestamp);
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_performance_metrics_date_hour 
    BEFORE INSERT OR UPDATE ON performance_metrics 
    FOR EACH ROW EXECUTE FUNCTION update_date_hour_column();

-- 创建分区表（按时间分区）
-- 注意：这需要在创建表后执行
-- CREATE TABLE performance_metrics_y2024m01 PARTITION OF performance_metrics
--     FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');
