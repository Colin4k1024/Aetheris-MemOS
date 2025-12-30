-- =============================================
-- 智能体记忆系统数据库创建脚本 (PostgreSQL)
-- =============================================

-- 创建数据库
CREATE DATABASE agent_memory_system 
WITH 
    ENCODING = 'UTF8'
    LC_COLLATE = 'en_US.utf8'
    LC_CTYPE = 'en_US.utf8'
    TEMPLATE = template0;

-- 连接到数据库
-- 注意：请在 psql 中手动执行 \c agent_memory_system; 命令

-- 创建扩展
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";
CREATE EXTENSION IF NOT EXISTS "btree_gin";

-- 设置时区
SET timezone = 'UTC';

-- 注意：请按以下顺序执行 SQL 文件：
-- 1. 00_create_database_postgresql.sql (本文件)
-- 2. 01_short_term_memory_postgresql.sql
-- 3. 02_long_term_memory_postgresql.sql
-- 4. 03_knowledge_graph_postgresql.sql
-- 5. 04_multimodal_memory_postgresql.sql
-- 6. 05_memory_management_postgresql.sql
