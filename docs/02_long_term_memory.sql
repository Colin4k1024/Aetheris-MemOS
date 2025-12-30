-- =============================================
-- 长期记忆存储模型 (Long-Term Memory)
-- =============================================

-- 1. 知识条目表 (knowledge_entries)
CREATE TABLE knowledge_entries (
    -- 主键和标识
    entry_id VARCHAR(64) PRIMARY KEY,                    -- 知识条目唯一标识
    source_id VARCHAR(64) NOT NULL,                     -- 来源标识
    source_type ENUM('document', 'api', 'database', 'web', 'user_input') NOT NULL, -- 来源类型
    
    -- 内容信息
    title VARCHAR(500),                                 -- 标题
    content LONGTEXT NOT NULL,                          -- 内容
    content_type ENUM('text', 'html', 'markdown', 'json', 'structured') NOT NULL, -- 内容类型
    content_hash VARCHAR(64) NOT NULL,                  -- 内容哈希值
    
    -- 向量信息
    embedding_vector JSON NOT NULL,                     -- 向量表示
    embedding_model VARCHAR(100) NOT NULL,              -- 向量模型名称
    embedding_dimension INT UNSIGNED NOT NULL,          -- 向量维度
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,     -- 创建时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 更新时间
    last_accessed_at TIMESTAMP NULL,                    -- 最后访问时间
    
    -- 分类和标签
    category VARCHAR(100),                              -- 分类
    tags JSON,                                          -- 标签列表
    domain VARCHAR(100),                                -- 领域
    
    -- 质量指标
    quality_score DECIMAL(3,2) DEFAULT 0.50,           -- 质量评分 (0.00-1.00)
    relevance_score DECIMAL(3,2) DEFAULT 0.50,         -- 相关性评分 (0.00-1.00)
    confidence_score DECIMAL(3,2) DEFAULT 0.50,        -- 置信度评分 (0.00-1.00)
    
    -- 使用统计
    access_count INT UNSIGNED DEFAULT 0,                -- 访问次数
    success_count INT UNSIGNED DEFAULT 0,               -- 成功使用次数
    failure_count INT UNSIGNED DEFAULT 0,               -- 失败使用次数
    
    -- 状态信息
    status ENUM('active', 'archived', 'deprecated') DEFAULT 'active', -- 状态
    version INT UNSIGNED DEFAULT 1,                     -- 版本号
    
    -- 索引
    INDEX idx_source (source_id, source_type),
    INDEX idx_created_at (created_at),
    INDEX idx_category (category),
    INDEX idx_domain (domain),
    INDEX idx_quality_score (quality_score),
    INDEX idx_relevance_score (relevance_score),
    INDEX idx_status (status),
    INDEX idx_last_accessed (last_accessed_at),
    FULLTEXT idx_content (title, content)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 2. 知识关联表 (knowledge_relations)
CREATE TABLE knowledge_relations (
    -- 主键和标识
    relation_id VARCHAR(64) PRIMARY KEY,                -- 关系唯一标识
    source_entry_id VARCHAR(64) NOT NULL,              -- 源知识条目ID
    target_entry_id VARCHAR(64) NOT NULL,              -- 目标知识条目ID
    
    -- 关系信息
    relation_type ENUM('similar', 'related', 'contradictory', 'supports', 'refutes', 'extends') NOT NULL, -- 关系类型
    relation_strength DECIMAL(3,2) NOT NULL,           -- 关系强度 (0.00-1.00)
    relation_confidence DECIMAL(3,2) NOT NULL,         -- 关系置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 更新时间
    
    -- 元数据
    metadata JSON,                                     -- 关系元数据
    description TEXT,                                  -- 关系描述
    
    -- 外键约束
    FOREIGN KEY (source_entry_id) REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entry_id) REFERENCES knowledge_entries(entry_id) ON DELETE CASCADE,
    
    -- 索引
    INDEX idx_source (source_entry_id),
    INDEX idx_target (target_entry_id),
    INDEX idx_relation_type (relation_type),
    INDEX idx_strength (relation_strength),
    INDEX idx_confidence (relation_confidence),
    UNIQUE KEY uk_source_target (source_entry_id, target_entry_id, relation_type)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;


