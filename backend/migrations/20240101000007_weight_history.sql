-- =============================================
-- 权重调整历史表 (Weight Adjustment History) - SQLite
-- =============================================

-- 启用外键约束
PRAGMA foreign_keys = ON;

-- 权重调整历史表 (weight_adjustment_history)
CREATE TABLE IF NOT EXISTS weight_adjustment_history (
    -- 主键和标识
    history_id TEXT PRIMARY KEY,                 -- 历史记录唯一标识
    task_id TEXT NOT NULL,                      -- 任务ID
    
    -- 权重信息（JSON格式存储）
    old_weights_json TEXT NOT NULL,             -- 旧权重配置(JSON)
    new_weights_json TEXT NOT NULL,             -- 新权重配置(JSON)
    adjustment_reasons_json TEXT NOT NULL,      -- 调整原因(JSON)
    
    -- 性能影响
    performance_impact REAL DEFAULT 0.0,         -- 性能影响值
    
    -- 时间信息
    timestamp TEXT DEFAULT (datetime('now'))      -- 时间戳
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_weight_history_task_id ON weight_adjustment_history (task_id);
CREATE INDEX IF NOT EXISTS idx_weight_history_timestamp ON weight_adjustment_history (timestamp);

