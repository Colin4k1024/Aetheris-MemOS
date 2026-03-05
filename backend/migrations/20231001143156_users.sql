-- 用户表结构。初始用户请通过 API 注册或部署后手动创建。
CREATE TABLE IF NOT EXISTS users
(
    id       TEXT PRIMARY KEY NOT NULL,
    username VARCHAR(255)     NOT NULL UNIQUE,
    password VARCHAR(511)     NOT NULL
);