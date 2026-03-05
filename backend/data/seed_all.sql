-- =============================================
-- 测试数据加载脚本 - 汇总
-- 使用方法: sqlite3 data.db < data/seed_all.sql
-- =============================================

-- 首先创建数据库表（如果不存在）
-- 注意：需要先运行 migrations 中的表结构创建脚本

-- =============================================
-- 1. 加载短期记忆测试数据 (STM)
-- =============================================
.read test_stm.sql

-- =============================================
-- 2. 加载长期记忆测试数据 (LTM)
-- =============================================
.read test_ltm.sql

-- =============================================
-- 3. 加载知识图谱测试数据 (KG)
-- =============================================
.read test_kg.sql

-- =============================================
-- 验证数据加载结果
-- =============================================
SELECT '=== 短期记忆 (会话) ===' as info;
SELECT COUNT(*) as session_count FROM context_sessions;

SELECT '=== 短期记忆 (消息) ===' as info;
SELECT COUNT(*) as message_count FROM context_messages;

SELECT '=== 长期记忆 (知识条目) ===' as info;
SELECT COUNT(*) as knowledge_count FROM knowledge_entries;

SELECT '=== 知识图谱 (实体) ===' as info;
SELECT COUNT(*) as entity_count FROM entities;

SELECT '=== 知识图谱 (关系) ===' as info;
SELECT COUNT(*) as relation_count FROM relations;

-- 显示会话类型分布
SELECT '=== 会话类型分布 ===' as info;
SELECT session_type, COUNT(*) as count FROM context_sessions GROUP BY session_type;

-- 显示知识领域分布
SELECT '=== 知识领域分布 ===' as info;
SELECT domain, COUNT(*) as count FROM knowledge_entries GROUP BY domain;

-- 显示实体类型分布
SELECT '=== 实体类型分布 ===' as info;
SELECT entity_type, COUNT(*) as count FROM entities GROUP BY entity_type;

-- 显示关系类型分布
SELECT '=== 关系类型分布 ===' as info;
SELECT relation_type, COUNT(*) as count FROM relations GROUP BY relation_type;

SELECT '=== 数据加载完成 ===' as info;
