-- =============================================
-- 多模态记忆存储模型 (Multi-Modal Memory) - SQLite
-- =============================================

-- 启用外键约束
PRAGMA foreign_keys = ON;

-- 1. 多模态条目表 (multimodal_entries)
CREATE TABLE IF NOT EXISTS multimodal_entries (
    -- 主键和标识
    entry_id TEXT PRIMARY KEY,                   -- 条目唯一标识
    session_id TEXT,                             -- 关联会话ID
    source_id TEXT NOT NULL,                    -- 来源标识
    
    -- 模态信息
    modality_type TEXT NOT NULL CHECK (modality_type IN ('text', 'image', 'audio', 'video', 'mixed')), -- 模态类型
    modality_count INTEGER DEFAULT 1 CHECK (modality_count > 0),         -- 模态数量
    
    -- 内容信息
    title TEXT,                                -- 标题
    description TEXT,                                   -- 描述
    content_metadata TEXT NOT NULL,                    -- 内容元数据(JSON格式，存储为TEXT)
    
    -- 文本内容
    text_content TEXT,                             -- 文本内容
    text_embedding TEXT,                               -- 文本向量(JSON格式，存储为TEXT)
    
    -- 图像内容
    image_url TEXT,                           -- 图像URL
    image_embedding TEXT,                              -- 图像向量(JSON格式，存储为TEXT)
    image_features TEXT,                               -- 图像特征(JSON格式，存储为TEXT)
    
    -- 音频内容
    audio_url TEXT,                           -- 音频URL
    audio_embedding TEXT,                              -- 音频向量(JSON格式，存储为TEXT)
    audio_transcript TEXT,                             -- 音频转录文本
    audio_features TEXT,                               -- 音频特征(JSON格式，存储为TEXT)
    
    -- 视频内容
    video_url TEXT,                           -- 视频URL
    video_embedding TEXT,                              -- 视频向量(JSON格式，存储为TEXT)
    video_transcript TEXT,                             -- 视频转录文本
    video_features TEXT,                               -- 视频特征(JSON格式，存储为TEXT)
    
    -- 跨模态信息
    cross_modal_alignment TEXT,                        -- 跨模态对齐信息(JSON格式，存储为TEXT)
    unified_embedding TEXT,                            -- 统一向量表示(JSON格式，存储为TEXT)
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),    -- 创建时间
    updated_at TEXT DEFAULT (datetime('now')),    -- 更新时间
    
    -- 质量指标
    quality_score REAL DEFAULT 0.50 CHECK (quality_score >= 0.00 AND quality_score <= 1.00),           -- 质量评分 (0.00-1.00)
    modality_consistency REAL DEFAULT 0.50 CHECK (modality_consistency >= 0.00 AND modality_consistency <= 1.00),    -- 模态一致性 (0.00-1.00)
    
    -- 使用统计
    access_count INTEGER DEFAULT 0 CHECK (access_count >= 0),                -- 访问次数
    success_count INTEGER DEFAULT 0 CHECK (success_count >= 0),               -- 成功次数
    
    -- 状态信息
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'processing', 'error', 'archived')) -- 状态
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_multimodal_entries_session ON multimodal_entries (session_id);
CREATE INDEX IF NOT EXISTS idx_multimodal_entries_source ON multimodal_entries (source_id);
CREATE INDEX IF NOT EXISTS idx_multimodal_entries_modality_type ON multimodal_entries (modality_type);
CREATE INDEX IF NOT EXISTS idx_multimodal_entries_created_at ON multimodal_entries (created_at);
CREATE INDEX IF NOT EXISTS idx_multimodal_entries_quality_score ON multimodal_entries (quality_score);
CREATE INDEX IF NOT EXISTS idx_multimodal_entries_status ON multimodal_entries (status);

-- 创建更新时间触发器
CREATE TRIGGER IF NOT EXISTS update_multimodal_entries_updated_at 
    AFTER UPDATE ON multimodal_entries
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE multimodal_entries SET updated_at = datetime('now') WHERE entry_id = NEW.entry_id;
END;

-- 2. 模态关联表 (modality_relations)
CREATE TABLE IF NOT EXISTS modality_relations (
    -- 主键和标识
    relation_id TEXT PRIMARY KEY,                -- 关系唯一标识
    source_entry_id TEXT NOT NULL,              -- 源条目ID
    target_entry_id TEXT NOT NULL,              -- 目标条目ID
    
    -- 关联信息
    relation_type TEXT NOT NULL CHECK (relation_type IN ('temporal', 'spatial', 'semantic', 'causal', 'similar')), -- 关联类型
    relation_strength REAL NOT NULL CHECK (relation_strength >= 0.00 AND relation_strength <= 1.00),           -- 关联强度 (0.00-1.00)
    relation_confidence REAL NOT NULL CHECK (relation_confidence >= 0.00 AND relation_confidence <= 1.00),         -- 关联置信度 (0.00-1.00)
    
    -- 时间信息
    created_at TEXT DEFAULT (datetime('now')),    -- 创建时间
    
    -- 元数据
    metadata TEXT,                                     -- 关联元数据(JSON格式，存储为TEXT)
    description TEXT,                                  -- 关联描述
    
    -- 外键约束
    FOREIGN KEY (source_entry_id) REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    FOREIGN KEY (target_entry_id) REFERENCES multimodal_entries(entry_id) ON DELETE CASCADE,
    
    -- 唯一约束
    UNIQUE (source_entry_id, target_entry_id, relation_type)
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_modality_relations_source ON modality_relations (source_entry_id);
CREATE INDEX IF NOT EXISTS idx_modality_relations_target ON modality_relations (target_entry_id);
CREATE INDEX IF NOT EXISTS idx_modality_relations_type ON modality_relations (relation_type);
CREATE INDEX IF NOT EXISTS idx_modality_relations_strength ON modality_relations (relation_strength);
CREATE INDEX IF NOT EXISTS idx_modality_relations_confidence ON modality_relations (relation_confidence);

