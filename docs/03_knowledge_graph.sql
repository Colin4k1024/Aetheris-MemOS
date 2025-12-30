-- =============================================
-- 知识图谱存储模型 (Knowledge Graph)
-- =============================================

-- 1. 实体表 (entities)
CREATE TABLE entities (
    -- 主键和标识
    entity_id VARCHAR(64) PRIMARY KEY,                  -- 实体唯一标识
    entity_name VARCHAR(500) NOT NULL,                  -- 实体名称
    entity_type VARCHAR(100) NOT NULL,                  -- 实体类型
    
    -- 实体信息
    description TEXT,                                   -- 实体描述
    attributes JSON,                                    -- 实体属性
    aliases JSON,                                       -- 实体别名
    
    -- 向量信息
    embedding_vector JSON,                              -- 实体向量表示
    embedding_model VARCHAR(100),                       -- 向量模型名称
    embedding_dimension INT UNSIGNED,                   -- 向量维度
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 更新时间
    
    -- 质量指标
    confidence_score DECIMAL(3,2) DEFAULT 0.50,        -- 置信度评分 (0.00-1.00)
    popularity_score DECIMAL(3,2) DEFAULT 0.50,        -- 流行度评分 (0.00-1.00)
    
    -- 统计信息
    relation_count INT UNSIGNED DEFAULT 0,              -- 关系数量
    mention_count INT UNSIGNED DEFAULT 0,               -- 提及次数
    
    -- 状态信息
    status ENUM('active', 'merged', 'deprecated') DEFAULT 'active', -- 状态
    
    -- 索引
    INDEX idx_entity_type (entity_type),
    INDEX idx_entity_name (entity_name),
    INDEX idx_created_at (created_at),
    INDEX idx_confidence_score (confidence_score),
    INDEX idx_popularity_score (popularity_score),
    INDEX idx_status (status),
    FULLTEXT idx_description (description)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

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
    properties JSON,                                    -- 关系属性
    
    -- 权重和置信度
    weight DECIMAL(5,4) DEFAULT 1.0000,                -- 关系权重 (0.0000-1.0000)
    confidence DECIMAL(3,2) NOT NULL,                  -- 关系置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 更新时间
    
    -- 统计信息
    usage_count INT UNSIGNED DEFAULT 0,                -- 使用次数
    success_count INT UNSIGNED DEFAULT 0,              -- 成功使用次数
    
    -- 状态信息
    status ENUM('active', 'inactive', 'deprecated') DEFAULT 'active', -- 状态
    
    -- 外键约束
    FOREIGN KEY (source_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- 索引
    INDEX idx_source (source_entity_id),
    INDEX idx_target (target_entity_id),
    INDEX idx_relation_type (relation_type),
    INDEX idx_weight (weight),
    INDEX idx_confidence (confidence),
    INDEX idx_status (status),
    UNIQUE KEY uk_source_target_type (source_entity_id, target_entity_id, relation_type)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 3. 推理路径表 (reasoning_paths)
CREATE TABLE reasoning_paths (
    -- 主键和标识
    path_id VARCHAR(64) PRIMARY KEY,                    -- 路径唯一标识
    source_entity_id VARCHAR(64) NOT NULL,             -- 源实体ID
    target_entity_id VARCHAR(64) NOT NULL,             -- 目标实体ID
    
    -- 路径信息
    path_length TINYINT UNSIGNED NOT NULL,              -- 路径长度
    path_entities JSON NOT NULL,                        -- 路径实体列表
    path_relations JSON NOT NULL,                       -- 路径关系列表
    
    -- 推理信息
    reasoning_type ENUM('deduction', 'induction', 'abduction', 'analogy') NOT NULL, -- 推理类型
    reasoning_strength DECIMAL(3,2) NOT NULL,          -- 推理强度 (0.00-1.00)
    reasoning_confidence DECIMAL(3,2) NOT NULL,        -- 推理置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    last_used_at TIMESTAMP NULL,                       -- 最后使用时间
    
    -- 使用统计
    usage_count INT UNSIGNED DEFAULT 0,                -- 使用次数
    success_count INT UNSIGNED DEFAULT 0,              -- 成功次数
    
    -- 外键约束
    FOREIGN KEY (source_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- 索引
    INDEX idx_source (source_entity_id),
    INDEX idx_target (target_entity_id),
    INDEX idx_path_length (path_length),
    INDEX idx_reasoning_type (reasoning_type),
    INDEX idx_strength (reasoning_strength),
    INDEX idx_confidence (reasoning_confidence),
    INDEX idx_last_used (last_used_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;


