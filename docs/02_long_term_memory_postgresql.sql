-- =============================================
-- 长期记忆存储模型 (Long-Term Memory) - PostgreSQL
-- =============================================

-- 1. 知识条目表 (knowledge_entries)
CREATE TABLE knowledge_entries (
    -- 主键和标识
    entry_id VARCHAR(64) PRIMARY KEY,                    -- 知识条目唯一标识
    source_id VARCHAR(64) NOT NULL,                     -- 来源标识
    source_type VARCHAR(20) NOT NULL CHECK (source_type IN ('document', 'api', 'database', 'web', 'user_input')), -- 来源类型
    
    -- 内容信息
    title VARCHAR(500),                                 -- 标题
    content TEXT NOT NULL,                          -- 内容
    content_type VARCHAR(20) NOT NULL CHECK (content_type IN ('text', 'html', 'markdown', 'json', 'structured')), -- 内容类型
    content_hash VARCHAR(64) NOT NULL,                  -- 内容哈希值
    
    -- 向量信息
    embedding_vector JSONB NOT NULL,                     -- 向量表示
    embedding_model VARCHAR(100) NOT NULL,              -- 向量模型名称
    embedding_dimension INTEGER NOT NULL,          -- 向量维度
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,     -- 创建时间
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,     -- 更新时间
    last_accessed_at TIMESTAMP WITH TIME ZONE,                    -- 最后访问时间
    
    -- 分类和标签
    category VARCHAR(100),                              -- 分类
    tags JSONB,                                          -- 标签列表
    domain VARCHAR(100),                                -- 领域
    
    -- 质量指标
    quality_score DECIMAL(3,2) DEFAULT 0.50 CHECK (quality_score >= 0.00 AND quality_score <= 1.00),           -- 质量评分 (0.00-1.00)
    relevance_score DECIMAL(3,2) DEFAULT 0.50 CHECK (relevance_score >= 0.00 AND relevance_score <= 1.00),         -- 相关性评分 (0.00-1.00)
    confidence_score DECIMAL(3,2) DEFAULT 0.50 CHECK (confidence_score >= 0.00 AND confidence_score <= 1.00),        -- 置信度评分 (0.00-1.00)
    
    -- 使用统计
    access_count INTEGER DEFAULT 0,                -- 访问次数
    success_count INTEGER DEFAULT 0,               -- 成功使用次数
    failure_count INTEGER DEFAULT 0,               -- 失败使用次数
    
    -- 状态信息
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'archived', 'deprecated')), -- 状态
    version INTEGER DEFAULT 1,                     -- 版本号
    
    -- 约束
    CONSTRAINT chk_embedding_dimension CHECK (embedding_dimension > 0),
    CONSTRAINT chk_access_count CHECK (access_count >= 0),
    CONSTRAINT chk_success_count CHECK (success_count >= 0),
    CONSTRAINT chk_failure_count CHECK (failure_count >= 0),
    CONSTRAINT chk_version CHECK (version > 0)
);

-- 创建索引
CREATE INDEX idx_knowledge_entries_source ON knowledge_entries (source_id, source_type);
CREATE INDEX idx_knowledge_entries_created_at ON knowledge_entries (created_at);
CREATE INDEX idx_knowledge_entries_category ON knowledge_entries (category);
CREATE INDEX idx_knowledge_entries_domain ON knowledge_entries (domain);
CREATE INDEX idx_knowledge_entries_quality_score ON knowledge_entries (quality_score);
CREATE INDEX idx_knowledge_entries_relevance_score ON knowledge_entries (relevance_score);
CREATE INDEX idx_knowledge_entries_status ON knowledge_entries (status);
CREATE INDEX idx_knowledge_entries_last_accessed ON knowledge_entries (last_accessed_at);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_knowledge_entries_embedding_vector ON knowledge_entries USING GIN (embedding_vector);
CREATE INDEX idx_knowledge_entries_tags ON knowledge_entries USING GIN (tags);

-- 创建全文搜索索引
CREATE INDEX idx_knowledge_entries_content_fts ON knowledge_entries USING GIN (to_tsvector('english', title || ' ' || content));

-- 确保函数存在
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 创建更新时间触发器
CREATE TRIGGER update_knowledge_entries_updated_at 
    BEFORE UPDATE ON knowledge_entries 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- 2. 知识关联表 (knowledge_relations)
CREATE TABLE knowledge_relations (
    -- 主键和标识
    relation_id VARCHAR(64) PRIMARY KEY,                -- 关系唯一标识
    source_entry_id VARCHAR(64) NOT NULL,              -- 源知识条目ID
    target_entry_id VARCHAR(64) NOT NULL,              -- 目标知识条目ID
    
    -- 关系信息
    relation_type VARCHAR(20) NOT NULL CHECK (relation_type IN ('similar', 'related', 'contradictory', 'supports', 'refutes', 'extends')), -- 关系类型
    relation_strength DECIMAL(3,2) NOT NULL CHECK (relation_strength >= 0.00 AND relation_strength <= 1.00),           -- 关系强度 (0.00-1.00)
    relation_confidence DECIMAL(3,2) NOT NULL CHECK (relation_confidence >= 0.00 AND relation_confidence <= 1.00),         -- 关系置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 更新时间
    
    -- 元数据
    metadata JSONB,                                     -- 关系元数据
    description TEXT,                                  -- 关系描述
    
    -- 外键约束
    CONSTRAINT fk_knowledge_relations_source 
        FOREIGN KEY (source_entry_id) REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    CONSTRAINT fk_knowledge_relations_target 
        FOREIGN KEY (target_entry_id) REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    
    -- 唯一约束
    CONSTRAINT uk_knowledge_relations_source_target 
        UNIQUE (source_entry_id, target_entry_id, relation_type)
);

-- 创建索引
CREATE INDEX idx_knowledge_relations_source ON knowledge_relations (source_entry_id);
CREATE INDEX idx_knowledge_relations_target ON knowledge_relations (target_entry_id);
CREATE INDEX idx_knowledge_relations_type ON knowledge_relations (relation_type);
CREATE INDEX idx_knowledge_relations_strength ON knowledge_relations (relation_strength);
CREATE INDEX idx_knowledge_relations_confidence ON knowledge_relations (relation_confidence);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_knowledge_relations_metadata ON knowledge_relations USING GIN (metadata);

-- 创建更新时间触发器
CREATE TRIGGER update_knowledge_relations_updated_at 
    BEFORE UPDATE ON knowledge_relations 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
