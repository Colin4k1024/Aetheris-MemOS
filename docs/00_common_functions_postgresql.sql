-- =============================================
-- PostgreSQL 通用函数定义
-- =============================================

-- 创建自动更新 updated_at 字段的函数
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 创建 UUID 生成函数（如果扩展不可用）
CREATE OR REPLACE FUNCTION generate_uuid_v4()
RETURNS UUID AS $$
BEGIN
    -- 尝试使用 uuid-ossp 扩展
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'uuid-ossp') THEN
        RETURN uuid_generate_v4();
    ELSE
        -- 如果扩展不可用，使用内置的 gen_random_uuid()
        RETURN gen_random_uuid();
    END IF;
END;
$$ language 'plpgsql';

-- 显示函数创建状态
SELECT 
    proname as function_name,
    pronargs as num_args,
    prorettype::regtype as return_type
FROM pg_proc 
WHERE proname IN ('update_updated_at_column', 'generate_uuid_v4')
ORDER BY proname;
