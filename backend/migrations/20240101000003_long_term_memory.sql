-- =============================================
-- 长期记忆存储模型 (Long-Term Memory) - SQLite
-- =============================================

-- 启用外键约束
PRAGMA foreign_keys = ON;

-- 1. 知识条目表 (knowledge_entries)
CREATE TABLE IF NOT EXISTS knowledge_entries (
    -- 主键和标识
    entry_id TEXT PRIMARY KEY,                    -- 知识条目唯一标识
    source_id TEXT NOT NULL,                     -- 来源标识
    source_type TEXT NOT NULL CHECK (source_type IN ('document', 'api', 'database', 'web', 'user_input')), -- 来源类型
    
    -- 内容信息
    title TEXT,                                 -- 标题
    content TEXT NOT NULL,                          -- 内容
    content_type TEXT NOT NULL CHECK (content_type IN ('text', 'html', 'markdown', 'json', 'structured')), -- 内容类型
    content_hash TEXT NOT NULL,                  -- 内容哈希值
    
    -- 向量信息
    embedding_vector TEXT NOT NULL,                     -- 向量表示(JSON格式，存储为TEXT)
    embedding_model TEXT NOT NULL,              -- 向量模型名称
    embedding_dimension INTEGER NOT NULL CHECK (embedding_dimension > 0),          -- 向量维度
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),     -- 创建时间
    updated_at TEXT DEFAULT (datetime('now')),     -- 更新时间
    last_accessed_at TEXT,                    -- 最后访问时间
    
    -- 分类和标签
    category TEXT,                              -- 分类
    tags TEXT,                                          -- 标签列表(JSON格式，存储为TEXT)
    domain TEXT,                                -- 领域
    
    -- 质量指标
    quality_score REAL DEFAULT 0.50 CHECK (quality_score >= 0.00 AND quality_score <= 1.00),           -- 质量评分 (0.00-1.00)
    relevance_score REAL DEFAULT 0.50 CHECK (relevance_score >= 0.00 AND relevance_score <= 1.00),         -- 相关性评分 (0.00-1.00)
    confidence_score REAL DEFAULT 0.50 CHECK (confidence_score >= 0.00 AND confidence_score <= 1.00),        -- 置信度评分 (0.00-1.00)
    
    -- 使用统计
    access_count INTEGER DEFAULT 0 CHECK (access_count >= 0),                -- 访问次数
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0),               -- 成功使用次数
    failure_count INTEGER DEFAULT 0 CHECK (failure_count >= 0),               -- 失败使用次数
    
    -- 状态信息
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'archived', 'deprecated')), -- 状态
    version INTEGER DEFAULT 1 CHECK (version > 0)                     -- 版本号
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_source ON knowledge_entries (source_id, source_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_created_at ON knowledge_entries (created_at);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_category ON knowledge_entries (category);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_domain ON knowledge_entries (domain);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_quality_score ON knowledge_entries (quality_score);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_relevance_score ON knowledge_entries (relevance_score);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_status ON knowledge_entries (status);
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_last_accessed ON knowledge_entries (last_accessed_at);

-- 创建更新时间触发器
CREATE TRIGGER IF NOT EXISTS update_knowledge_entries_updated_at 
    AFTER UPDATE ON knowledge_entries
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE knowledge_entries SET updated_at = datetime('now') WHERE entry_id = NEW.entry_id;
END;

-- 2. 知识关联表 (knowledge_relations)
CREATE TABLE IF NOT EXISTS knowledge_relations (
    -- 主键和标识
    relation_id TEXT PRIMARY KEY,                -- 关系唯一标识
    source_entry_id TEXT NOT NULL,              -- 源知识条目ID
    target_entry_id TEXT NOT NULL,              -- 目标知识条目ID
    
    -- 关系信息
    relation_type TEXT NOT NULL CHECK (relation_type IN ('similar', 'related', 'contradictory', 'supports', 'refutes', 'extends')), -- 关系类型
    relation_strength REAL NOT NULL CHECK (relation_strength >= 0.00 AND relation_strength <= 1.00),           -- 关系强度 (0.00-1.00)
    relation_confidence REAL NOT NULL CHECK (relation_confidence >= 0.00 AND relation_confidence <= 1.00),         -- 关系置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),    -- 创建时间
    updated_at TEXT DEFAULT (datetime('now')),    -- 更新时间
    
    -- 元数据
    metadata TEXT,                                     -- 关系元数据(JSON格式，存储为TEXT)
    description TEXT,                                  -- 关系描述
    
    -- 外键约束
    FOREIGN KEY (source_entry_id) REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entry_id) REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    
    -- 唯一约束
    UNIQUE (source_entry_id, target_entry_id, relation_type)
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_source ON knowledge_relations (source_entry_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_target ON knowledge_relations (target_entry_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_type ON knowledge_relations (relation_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_strength ON knowledge_relations (relation_strength);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_confidence ON knowledge_relations (relation_confidence);

-- 创建更新时间触发器
CREATE TRIGGER IF NOT EXISTS update_knowledge_relations_updated_at 
    AFTER UPDATE ON knowledge_relations
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE knowledge_relations SET updated_at = datetime('now') WHERE relation_id = NEW.relation_id;
END;

