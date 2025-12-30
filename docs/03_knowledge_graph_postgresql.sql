-- =============================================
-- 知识图谱存储模型 (Knowledge Graph) - PostgreSQL
-- =============================================

-- 1. 实体表 (entities)
CREATE TABLE entities (
    -- 主键和标识
    entity_id VARCHAR(64) PRIMARY KEY,                  -- 实体唯一标识
    entity_name VARCHAR(500) NOT NULL,                  -- 实体名称
    entity_type VARCHAR(100) NOT NULL,                  -- 实体类型
    
    -- 实体信息
    description TEXT,                                   -- 实体描述
    attributes JSONB,                                    -- 实体属性
    aliases JSONB,                                       -- 实体别名
    
    -- 向量信息
    embedding_vector JSONB,                              -- 实体向量表示
    embedding_model VARCHAR(100),                       -- 向量模型名称
    embedding_dimension INTEGER,                   -- 向量维度
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 更新时间
    
    -- 质量指标
    confidence_score DECIMAL(3,2) DEFAULT 0.50 CHECK (confidence_score >= 0.00 AND confidence_score <= 1.00),        -- 置信度评分 (0.00-1.00)
    popularity_score DECIMAL(3,2) DEFAULT 0.50 CHECK (popularity_score >= 0.00 AND popularity_score <= 1.00),        -- 流行度评分 (0.00-1.00)
    
    -- 统计信息
    relation_count INTEGER DEFAULT 0,              -- 关系数量
    mention_count INTEGER DEFAULT 0,               -- 提及次数
    
    -- 状态信息
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'merged', 'deprecated')), -- 状态
    
    -- 约束
    CONSTRAINT chk_embedding_dimension CHECK (embedding_dimension IS NULL OR embedding_dimension > 0),
    CONSTRAINT chk_relation_count CHECK (relation_count >= 0),
    CONSTRAINT chk_mention_count CHECK (mention_count >= 0)
);

-- 创建索引
CREATE INDEX idx_entities_entity_type ON entities (entity_type);
CREATE INDEX idx_entities_entity_name ON entities (entity_name);
CREATE INDEX idx_entities_created_at ON entities (created_at);
CREATE INDEX idx_entities_confidence_score ON entities (confidence_score);
CREATE INDEX idx_entities_popularity_score ON entities (popularity_score);
CREATE INDEX idx_entities_status ON entities (status);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_entities_attributes ON entities USING GIN (attributes);
CREATE INDEX idx_entities_aliases ON entities USING GIN (aliases);
CREATE INDEX idx_entities_embedding_vector ON entities USING GIN (embedding_vector);

-- 创建全文搜索索引
CREATE INDEX idx_entities_description_fts ON entities USING GIN (to_tsvector('english', description));

-- 创建更新时间触发器
CREATE TRIGGER update_entities_updated_at 
    BEFORE UPDATE ON entities 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- 2. 关系表 (relations)
CREATE TABLE relations (
    -- 主键和标识
    relation_id VARCHAR(64) PRIMARY KEY,                -- 关系唯一标识
    source_entity_id VARCHAR(64) NOT NULL,             -- 源实体ID
    target_entity_id VARCHAR(64) NOT NULL,             -- 目标实体ID
    relation_type VARCHAR(100) NOT NULL,                -- 关系类型
    
    -- 关系信息
    relation_name VARCHAR(200),                         -- 关系名称
    description TEXT,                                   -- 关系描述
    properties JSONB,                                    -- 关系属性
    
    -- 权重和置信度
    weight DECIMAL(5,4) DEFAULT 1.0000 CHECK (weight >= 0.0000 AND weight <= 1.0000),                -- 关系权重 (0.0000-1.0000)
    confidence DECIMAL(3,2) NOT NULL CHECK (confidence >= 0.00 AND confidence <= 1.00),                  -- 关系置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 更新时间
    
    -- 统计信息
    usage_count INTEGER DEFAULT 0,                -- 使用次数
    success_count INTEGER DEFAULT 0,              -- 成功使用次数
    
    -- 状态信息
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'deprecated')), -- 状态
    
    -- 外键约束
    CONSTRAINT fk_relations_source 
        FOREIGN KEY (source_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    CONSTRAINT fk_relations_target 
        FOREIGN KEY (target_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- 唯一约束
    CONSTRAINT uk_relations_source_target_type 
        UNIQUE (source_entity_id, target_entity_id, relation_type),
    
    -- 约束
    CONSTRAINT chk_usage_count CHECK (usage_count >= 0),
    CONSTRAINT chk_success_count CHECK (success_count >= 0)
);

-- 创建索引
CREATE INDEX idx_relations_source ON relations (source_entity_id);
CREATE INDEX idx_relations_target ON relations (target_entity_id);
CREATE INDEX idx_relations_type ON relations (relation_type);
CREATE INDEX idx_relations_weight ON relations (weight);
CREATE INDEX idx_relations_confidence ON relations (confidence);
CREATE INDEX idx_relations_status ON relations (status);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_relations_properties ON relations USING GIN (properties);

-- 创建更新时间触发器
CREATE TRIGGER update_relations_updated_at 
    BEFORE UPDATE ON relations 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- 3. 推理路径表 (reasoning_paths)
CREATE TABLE reasoning_paths (
    -- 主键和标识
    path_id VARCHAR(64) PRIMARY KEY,                    -- 路径唯一标识
    source_entity_id VARCHAR(64) NOT NULL,             -- 源实体ID
    target_entity_id VARCHAR(64) NOT NULL,             -- 目标实体ID
    
    -- 路径信息
    path_length SMALLINT NOT NULL,              -- 路径长度
    path_entities JSONB NOT NULL,                        -- 路径实体列表
    path_relations JSONB NOT NULL,                       -- 路径关系列表
    
    -- 推理信息
    reasoning_type VARCHAR(20) NOT NULL CHECK (reasoning_type IN ('deduction', 'induction', 'abduction', 'analogy')), -- 推理类型
    reasoning_strength DECIMAL(3,2) NOT NULL CHECK (reasoning_strength >= 0.00 AND reasoning_strength <= 1.00),          -- 推理强度 (0.00-1.00)
    reasoning_confidence DECIMAL(3,2) NOT NULL CHECK (reasoning_confidence >= 0.00 AND reasoning_confidence <= 1.00),        -- 推理置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    last_used_at TIMESTAMP WITH TIME ZONE,                       -- 最后使用时间
    
    -- 使用统计
    usage_count INTEGER DEFAULT 0,                -- 使用次数
    success_count INTEGER DEFAULT 0,              -- 成功次数
    
    -- 外键约束
    CONSTRAINT fk_reasoning_paths_source 
        FOREIGN KEY (source_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    CONSTRAINT fk_reasoning_paths_target 
        FOREIGN KEY (target_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- 约束
    CONSTRAINT chk_path_length CHECK (path_length > 0),
    CONSTRAINT chk_usage_count CHECK (usage_count >= 0),
    CONSTRAINT chk_success_count CHECK (success_count >= 0)
);

-- 创建索引
CREATE INDEX idx_reasoning_paths_source ON reasoning_paths (source_entity_id);
CREATE INDEX idx_reasoning_paths_target ON reasoning_paths (target_entity_id);
CREATE INDEX idx_reasoning_paths_length ON reasoning_paths (path_length);
CREATE INDEX idx_reasoning_paths_type ON reasoning_paths (reasoning_type);
CREATE INDEX idx_reasoning_paths_strength ON reasoning_paths (reasoning_strength);
CREATE INDEX idx_reasoning_paths_confidence ON reasoning_paths (reasoning_confidence);
CREATE INDEX idx_reasoning_paths_last_used ON reasoning_paths (last_used_at);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_reasoning_paths_entities ON reasoning_paths USING GIN (path_entities);
CREATE INDEX idx_reasoning_paths_relations ON reasoning_paths USING GIN (path_relations);
