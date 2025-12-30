-- =============================================
-- 多模态记忆存储模型 (Multi-Modal Memory) - PostgreSQL
-- =============================================

-- 1. 多模态条目表 (multimodal_entries)
CREATE TABLE multimodal_entries (
    -- 主键和标识
    entry_id VARCHAR(64) PRIMARY KEY,                   -- 条目唯一标识
    session_id VARCHAR(64),                             -- 关联会话ID
    source_id VARCHAR(64) NOT NULL,                    -- 来源标识
    
    -- 模态信息
    modality_type VARCHAR(20) NOT NULL CHECK (modality_type IN ('text', 'image', 'audio', 'video', 'mixed')), -- 模态类型
    modality_count SMALLINT DEFAULT 1,         -- 模态数量
    
    -- 内容信息
    title VARCHAR(500),                                -- 标题
    description TEXT,                                   -- 描述
    content_metadata JSONB NOT NULL,                    -- 内容元数据
    
    -- 文本内容
    text_content TEXT,                             -- 文本内容
    text_embedding JSONB,                               -- 文本向量
    
    -- 图像内容
    image_url VARCHAR(1000),                           -- 图像URL
    image_embedding JSONB,                              -- 图像向量
    image_features JSONB,                               -- 图像特征
    
    -- 音频内容
    audio_url VARCHAR(1000),                           -- 音频URL
    audio_embedding JSONB,                              -- 音频向量
    audio_transcript TEXT,                             -- 音频转录文本
    audio_features JSONB,                               -- 音频特征
    
    -- 视频内容
    video_url VARCHAR(1000),                           -- 视频URL
    video_embedding JSONB,                              -- 视频向量
    video_transcript TEXT,                             -- 视频转录文本
    video_features JSONB,                               -- 视频特征
    
    -- 跨模态信息
    cross_modal_alignment JSONB,                        -- 跨模态对齐信息
    unified_embedding JSONB,                            -- 统一向量表示
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 更新时间
    
    -- 质量指标
    quality_score DECIMAL(3,2) DEFAULT 0.50 CHECK (quality_score >= 0.00 AND quality_score <= 1.00),           -- 质量评分 (0.00-1.00)
    modality_consistency DECIMAL(3,2) DEFAULT 0.50 CHECK (modality_consistency >= 0.00 AND modality_consistency <= 1.00),    -- 模态一致性 (0.00-1.00)
    
    -- 使用统计
    access_count INTEGER DEFAULT 0,                -- 访问次数
    success_count INTEGER DEFAULT 0,               -- 成功次数
    
    -- 状态信息
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'processing', 'error', 'archived')), -- 状态
    
    -- 约束
    CONSTRAINT chk_modality_count CHECK (modality_count > 0),
    CONSTRAINT chk_access_count CHECK (access_count >= 0),
    CONSTRAINT chk_success_count CHECK (success_count >= 0)
);

-- 创建索引
CREATE INDEX idx_multimodal_entries_session ON multimodal_entries (session_id);
CREATE INDEX idx_multimodal_entries_source ON multimodal_entries (source_id);
CREATE INDEX idx_multimodal_entries_modality_type ON multimodal_entries (modality_type);
CREATE INDEX idx_multimodal_entries_created_at ON multimodal_entries (created_at);
CREATE INDEX idx_multimodal_entries_quality_score ON multimodal_entries (quality_score);
CREATE INDEX idx_multimodal_entries_status ON multimodal_entries (status);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_multimodal_entries_content_metadata ON multimodal_entries USING GIN (content_metadata);
CREATE INDEX idx_multimodal_entries_text_embedding ON multimodal_entries USING GIN (text_embedding);
CREATE INDEX idx_multimodal_entries_image_embedding ON multimodal_entries USING GIN (image_embedding);
CREATE INDEX idx_multimodal_entries_image_features ON multimodal_entries USING GIN (image_features);
CREATE INDEX idx_multimodal_entries_audio_embedding ON multimodal_entries USING GIN (audio_embedding);
CREATE INDEX idx_multimodal_entries_audio_features ON multimodal_entries USING GIN (audio_features);
CREATE INDEX idx_multimodal_entries_video_embedding ON multimodal_entries USING GIN (video_embedding);
CREATE INDEX idx_multimodal_entries_video_features ON multimodal_entries USING GIN (video_features);
CREATE INDEX idx_multimodal_entries_cross_modal_alignment ON multimodal_entries USING GIN (cross_modal_alignment);
CREATE INDEX idx_multimodal_entries_unified_embedding ON multimodal_entries USING GIN (unified_embedding);

-- 创建全文搜索索引
CREATE INDEX idx_multimodal_entries_text_content_fts ON multimodal_entries USING GIN (to_tsvector('english', text_content));
CREATE INDEX idx_multimodal_entries_description_fts ON multimodal_entries USING GIN (to_tsvector('english', description));

-- 创建更新时间触发器
CREATE TRIGGER update_multimodal_entries_updated_at 
    BEFORE UPDATE ON multimodal_entries 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- 2. 模态关联表 (modality_relations)
CREATE TABLE modality_relations (
    -- 主键和标识
    relation_id VARCHAR(64) PRIMARY KEY,                -- 关系唯一标识
    source_entry_id VARCHAR(64) NOT NULL,              -- 源条目ID
    target_entry_id VARCHAR(64) NOT NULL,              -- 目标条目ID
    
    -- 关联信息
    relation_type VARCHAR(20) NOT NULL CHECK (relation_type IN ('temporal', 'spatial', 'semantic', 'causal', 'similar')), -- 关联类型
    relation_strength DECIMAL(3,2) NOT NULL CHECK (relation_strength >= 0.00 AND relation_strength <= 1.00),           -- 关联强度 (0.00-1.00)
    relation_confidence DECIMAL(3,2) NOT NULL CHECK (relation_confidence >= 0.00 AND relation_confidence <= 1.00),         -- 关联置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,    -- 创建时间
    
    -- 元数据
    metadata JSONB,                                     -- 关联元数据
    description TEXT,                                  -- 关联描述
    
    -- 外键约束
    CONSTRAINT fk_modality_relations_source 
        FOREIGN KEY (source_entry_id) REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    CONSTRAINT fk_modality_relations_target 
        FOREIGN KEY (target_entry_id) REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    
    -- 唯一约束
    CONSTRAINT uk_modality_relations_source_target_type 
        UNIQUE (source_entry_id, target_entry_id, relation_type)
);

-- 创建索引
CREATE INDEX idx_modality_relations_source ON modality_relations (source_entry_id);
CREATE INDEX idx_modality_relations_target ON modality_relations (target_entry_id);
CREATE INDEX idx_modality_relations_type ON modality_relations (relation_type);
CREATE INDEX idx_modality_relations_strength ON modality_relations (relation_strength);
CREATE INDEX idx_modality_relations_confidence ON modality_relations (relation_confidence);

-- 创建 GIN 索引用于 JSONB 字段
CREATE INDEX idx_modality_relations_metadata ON modality_relations USING GIN (metadata);
