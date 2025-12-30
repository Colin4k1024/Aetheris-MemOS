-- =============================================
-- 多模态记忆存储模型 (Multi-Modal Memory)
-- =============================================

-- 1. 多模态条目表 (multimodal_entries)
CREATE TABLE multimodal_entries (
    -- 主键和标识
    entry_id VARCHAR(64) PRIMARY KEY,                   -- 条目唯一标识
    session_id VARCHAR(64),                             -- 关联会话ID
    source_id VARCHAR(64) NOT NULL,                    -- 来源标识
    
    -- 模态信息
    modality_type ENUM('text', 'image', 'audio', 'video', 'mixed') NOT NULL, -- 模态类型
    modality_count TINYINT UNSIGNED DEFAULT 1,         -- 模态数量
    
    -- 内容信息
    title VARCHAR(500),                                -- 标题
    description TEXT,                                   -- 描述
    content_metadata JSON NOT NULL,                    -- 内容元数据
    
    -- 文本内容
    text_content LONGTEXT,                             -- 文本内容
    text_embedding JSON,                               -- 文本向量
    
    -- 图像内容
    image_url VARCHAR(1000),                           -- 图像URL
    image_embedding JSON,                              -- 图像向量
    image_features JSON,                               -- 图像特征
    
    -- 音频内容
    audio_url VARCHAR(1000),                           -- 音频URL
    audio_embedding JSON,                              -- 音频向量
    audio_transcript TEXT,                             -- 音频转录文本
    audio_features JSON,                               -- 音频特征
    
    -- 视频内容
    video_url VARCHAR(1000),                           -- 视频URL
    video_embedding JSON,                              -- 视频向量
    video_transcript TEXT,                             -- 视频转录文本
    video_features JSON,                               -- 视频特征
    
    -- 跨模态信息
    cross_modal_alignment JSON,                        -- 跨模态对齐信息
    unified_embedding JSON,                            -- 统一向量表示
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 更新时间
    
    -- 质量指标
    quality_score DECIMAL(3,2) DEFAULT 0.50,           -- 质量评分 (0.00-1.00)
    modality_consistency DECIMAL(3,2) DEFAULT 0.50,    -- 模态一致性 (0.00-1.00)
    
    -- 使用统计
    access_count INT UNSIGNED DEFAULT 0,                -- 访问次数
    success_count INT UNSIGNED DEFAULT 0,               -- 成功次数
    
    -- 状态信息
    status ENUM('active', 'processing', 'error', 'archived') DEFAULT 'active', -- 状态
    
    -- 索引
    INDEX idx_session (session_id),
    INDEX idx_source (source_id),
    INDEX idx_modality_type (modality_type),
    INDEX idx_created_at (created_at),
    INDEX idx_quality_score (quality_score),
    INDEX idx_status (status),
    FULLTEXT idx_text_content (text_content),
    FULLTEXT idx_description (description)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 2. 模态关联表 (modality_relations)
CREATE TABLE modality_relations (
    -- 主键和标识
    relation_id VARCHAR(64) PRIMARY KEY,                -- 关系唯一标识
    source_entry_id VARCHAR(64) NOT NULL,              -- 源条目ID
    target_entry_id VARCHAR(64) NOT NULL,              -- 目标条目ID
    
    -- 关联信息
    relation_type ENUM('temporal', 'spatial', 'semantic', 'causal', 'similar') NOT NULL, -- 关联类型
    relation_strength DECIMAL(3,2) NOT NULL,           -- 关联强度 (0.00-1.00)
    relation_confidence DECIMAL(3,2) NOT NULL,         -- 关联置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    
    -- 元数据
    metadata JSON,                                     -- 关联元数据
    description TEXT,                                  -- 关联描述
    
    -- 外键约束
    FOREIGN KEY (source_entry_id) REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entry_id) REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    
    -- 索引
    INDEX idx_source (source_entry_id),
    INDEX idx_target (target_entry_id),
    INDEX idx_relation_type (relation_type),
    INDEX idx_strength (relation_strength),
    INDEX idx_confidence (relation_confidence),
    UNIQUE KEY uk_source_target_type (source_entry_id, target_entry_id, relation_type)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
