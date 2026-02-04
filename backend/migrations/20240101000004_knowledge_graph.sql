-- =============================================
-- 知识图谱存储模型 (Knowledge Graph) - SQLite
-- =============================================

-- 启用外键约束
PRAGMA foreign_keys = ON;

-- 1. 实体表 (entities)
CREATE TABLE IF NOT EXISTS entities (
    -- 主键和标识
    entity_id TEXT PRIMARY KEY,                  -- 实体唯一标识
    entity_name TEXT NOT NULL,                  -- 实体名称
    entity_type TEXT NOT NULL,                  -- 实体类型
    
    -- 实体信息
    description TEXT,                                   -- 实体描述
    attributes TEXT,                                    -- 实体属性(JSON格式，存储为TEXT)
    aliases TEXT,                                       -- 实体别名(JSON格式，存储为TEXT)
    
    -- 向量信息
    embedding_vector TEXT,                              -- 实体向量表示(JSON格式，存储为TEXT)
    embedding_model TEXT,                       -- 向量模型名称
    embedding_dimension INTEGER CHECK (embedding_dimension IS NULL OR embedding_dimension > 0),                   -- 向量维度
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),    -- 创建时间
    updated_at TEXT DEFAULT (datetime('now')),    -- 更新时间
    
    -- 质量指标
    confidence_score REAL DEFAULT 0.50 CHECK (confidence_score >= 0.00 AND confidence_score <= 1.00),        -- 置信度评分 (0.00-1.00)
    popularity_score REAL DEFAULT 0.50 CHECK (popularity_score >= 0.00 AND popularity_score <= 1.00),        -- 流行度评分 (0.00-1.00)
    
    -- 统计信息
    relation_count INTEGER DEFAULT 0 CHECK (relation_count >= 0),              -- 关系数量
    mention_count INTEGER DEFAULT 0 CHECK (mention_count >= 0),               -- 提及次数
    
    -- 状态信息
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'merged', 'deprecated')) -- 状态
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_entities_entity_type ON entities (entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_entity_name ON entities (entity_name);
CREATE INDEX IF NOT EXISTS idx_entities_created_at ON entities (created_at);
CREATE INDEX IF NOT EXISTS idx_entities_confidence_score ON entities (confidence_score);
CREATE INDEX IF NOT EXISTS idx_entities_popularity_score ON entities (popularity_score);
CREATE INDEX IF NOT EXISTS idx_entities_status ON entities (status);

-- 创建更新时间触发器
CREATE TRIGGER IF NOT EXISTS update_entities_updated_at 
    AFTER UPDATE ON entities
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE entities SET updated_at = datetime('now') WHERE entity_id = NEW.entity_id;
END;

-- 2. 关系表 (relations)
CREATE TABLE IF NOT EXISTS relations (
    -- 主键和标识
    relation_id TEXT PRIMARY KEY,                -- 关系唯一标识
    source_entity_id TEXT NOT NULL,             -- 源实体ID
    target_entity_id TEXT NOT NULL,             -- 目标实体ID
    relation_type TEXT NOT NULL,                -- 关系类型
    
    -- 关系信息
    relation_name TEXT,                         -- 关系名称
    description TEXT,                                   -- 关系描述
    properties TEXT,                                    -- 关系属性(JSON格式，存储为TEXT)
    
    -- 权重和置信度
    weight REAL DEFAULT 1.0000 CHECK (weight >= 0.0000 AND weight <= 1.0000),                -- 关系权重 (0.0000-1.0000)
    confidence REAL NOT NULL CHECK (confidence >= 0.00 AND confidence <= 1.00),                  -- 关系置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),    -- 创建时间
    updated_at TEXT DEFAULT (datetime('now')),    -- 更新时间
    
    -- 统计信息
    usage_count INTEGER DEFAULT 0 CHECK (usage_count >= 0),                -- 使用次数
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0),              -- 成功使用次数
    
    -- 状态信息
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'deprecated')), -- 状态
    
    -- 外键约束
    FOREIGN KEY (source_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- 唯一约束
    UNIQUE (source_entity_id, target_entity_id, relation_type),
    
    -- 约束
    CHECK (usage_count >= 0),
    CHECK (success_count >= 0)
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_relations_source ON relations (source_entity_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations (target_entity_id);
CREATE INDEX IF NOT EXISTS idx_relations_type ON relations (relation_type);
CREATE INDEX IF NOT EXISTS idx_relations_weight ON relations (weight);
CREATE INDEX IF NOT EXISTS idx_relations_confidence ON relations (confidence);
CREATE INDEX IF NOT EXISTS idx_relations_status ON relations (status);

-- 创建更新时间触发器
CREATE TRIGGER IF NOT EXISTS update_relations_updated_at 
    AFTER UPDATE ON relations
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE relations SET updated_at = datetime('now') WHERE relation_id = NEW.relation_id;
END;

-- 3. 推理路径表 (reasoning_paths)
CREATE TABLE IF NOT EXISTS reasoning_paths (
    -- 主键和标识
    path_id TEXT PRIMARY KEY,                    -- 路径唯一标识
    source_entity_id TEXT NOT NULL,             -- 源实体ID
    target_entity_id TEXT NOT NULL,             -- 目标实体ID
    
    -- 路径信息
    path_length INTEGER NOT NULL CHECK (path_length > 0),              -- 路径长度
    path_entities TEXT NOT NULL,                        -- 路径实体列表(JSON格式，存储为TEXT)
    path_relations TEXT NOT NULL,                       -- 路径关系列表(JSON格式，存储为TEXT)
    
    -- 推理信息
    reasoning_type TEXT NOT NULL CHECK (reasoning_type IN ('deduction', 'induction', 'abduction', 'analogy')), -- 推理类型
    reasoning_strength REAL NOT NULL CHECK (reasoning_strength >= 0.00 AND reasoning_strength <= 1.00),          -- 推理强度 (0.00-1.00)
    reasoning_confidence REAL NOT NULL CHECK (reasoning_confidence >= 0.00 AND reasoning_confidence <= 1.00),        -- 推理置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),    -- 创建时间
    last_used_at TEXT,                       -- 最后使用时间
    
    -- 使用统计
    usage_count INTEGER DEFAULT 0 CHECK (usage_count >= 0),                -- 使用次数
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0),              -- 成功次数
    
    -- 外键约束
    FOREIGN KEY (source_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_source ON reasoning_paths (source_entity_id);
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_target ON reasoning_paths (target_entity_id);
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_length ON reasoning_paths (path_length);
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_type ON reasoning_paths (reasoning_type);
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_strength ON reasoning_paths (reasoning_strength);
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_confidence ON reasoning_paths (reasoning_confidence);
CREATE INDEX IF NOT EXISTS idx_reasoning_paths_last_used ON reasoning_paths (last_used_at);

