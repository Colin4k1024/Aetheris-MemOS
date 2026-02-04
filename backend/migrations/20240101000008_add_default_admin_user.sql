-- =============================================
-- 添加默认管理员用户 (Add Default Admin User)
-- =============================================

-- 启用外键约束
PRAGMA foreign_keys = ON;

-- 插入默认管理员用户（如果不存在）
-- 用户名: admin
-- 密码: ant.design
INSERT OR IGNORE INTO users (id, username, password) 
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'admin',
    '$argon2id$v=19$m=19456,t=2,p=1$iQleADL0WPFlH8YLv4Db8Q$iMVSmbrvLg8Jygq3854xbdv35cL5ousdYeySGjBJHvQ'
);

