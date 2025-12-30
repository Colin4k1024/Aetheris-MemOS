-- =============================================
-- PostgreSQL 扩展和配置设置脚本
-- =============================================

-- 创建必要的扩展
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";
CREATE EXTENSION IF NOT EXISTS "btree_gin";

-- 设置时区
SET timezone = 'UTC';

-- 创建自动更新 updated_at 字段的函数
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 显示扩展安装状态
SELECT extname, extversion FROM pg_extension WHERE extname IN ('uuid-ossp', 'pg_trgm', 'btree_gin');
