-- =============================================
-- 智能体记忆系统数据库创建脚本
-- =============================================

-- 创建数据库
CREATE DATABASE IF NOT EXISTS agent_memory_system 
DEFAULT CHARACTER SET utf8mb4 
COLLATE utf8mb4_unicode_ci;

-- 使用数据库
USE agent_memory_system;

-- 设置会话参数
SET NAMES utf8mb4;
SET FOREIGN_KEY_CHECKS = 0;

-- 注意：请按以下顺序执行 SQL 文件：
-- 1. 00_create_database.sql (本文件)
-- 2. 01_short_term_memory.sql
-- 3. 02_long_term_memory.sql
-- 4. 03_knowledge_graph.sql
-- 5. 04_multimodal_memory.sql
-- 6. 05_memory_management.sql

-- 恢复外键检查
SET FOREIGN_KEY_CHECKS = 1;
