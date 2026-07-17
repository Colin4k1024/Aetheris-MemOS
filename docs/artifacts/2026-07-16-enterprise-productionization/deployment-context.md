# Deployment / Infra Context — P1 执行环境就绪

- **日期**：2026-07-16 ｜ **主责**：devops-engineer / tech-lead
- **用途**：P1 落码执行所需的**基建就绪**说明——一条命令起本地数据基建，之后照 `p1-execution-runbook.md` 逐 PR 开工。
- **前提**：本文档在无 Docker daemon / 无网络的沙箱内产出，**无法在此 provision**；在有 Docker+网络的机器上按下述执行即可。

---

## 结论先行

1. **基建定义已就绪**：`docker-compose.yml` 的数据服务（PostgreSQL+pgvector / Qdrant / Neo4j+apoc / Redis）版本固定、healthcheck 齐全；应用启动**自动跑迁移**（`db/mod.rs` 的 `Migrator`）。
2. **一键起**：`bash scripts/dev-infra.sh`（起数据服务 → 等健康 → 迁移 → 重生成 `.sqlx`）。
3. **两个硬门槛**（我在 P0 加的安全默认值）：① 认证默认开启，`cargo run`/`docker compose up` 必须设强 `APP_JWT_SECRET`；② Qdrant 默认度量改为 Cosine，**已存在集合需 drop 重建**。

---

## 环境清单

| 服务 | 镜像 | 端口 | 用途 | 凭据 |
|---|---|---|---|---|
| PostgreSQL | `pgvector/pgvector:pg16` | 5432 | 主库（记忆/租户/审计/迁移/RLS/advisory-lock） | memory/memory，db=memory |
| Qdrant | `qdrant/qdrant:v1.9.4` | 6333(REST)/6334(gRPC) | 向量库（LTM 向量、outbox 投递） | — |
| Neo4j | `neo4j:5`（apoc） | 7474/7687 | 图库（KG） | neo4j/password |
| Redis | `redis:7-alpine` | 6379 | STM 缓存（`redis-stm` feature） | — |
| Backend | 本地 `cargo run` 或 `backend/Dockerfile` | 8008 | API（`/api`、`/scalar`） | JWT |

---

## 部署入口

- **一键（推荐）**：`bash scripts/dev-infra.sh` — 起 4 个数据服务 + 等 PG 健康 + `sqlx migrate run` + `cargo sqlx prepare`。
- **手工**：
  ```bash
  docker compose up -d postgres qdrant neo4j redis
  export DATABASE_URL=postgres://memory:memory@localhost:5432/memory
  (cd backend && sqlx migrate run --source migrations)     # 或直接 cargo run 让 app 自动迁移
  export APP_JWT_SECRET="$(openssl rand -hex 32)"
  (cd backend && cargo run)                                 # http://127.0.0.1:8008
  ```
- **停/清**：`bash scripts/dev-infra.sh --down`（保卷）/ `--reset`（删卷，销毁数据）。
- **Docker 全栈**：`docker compose up`（含 backend，走 `docker.toml`；compose 已要求 `JWT_SECRET`，见 `.env.example`）。

---

## 配置与密钥

- `backend/config.toml`（本地 loopback）：PG `localhost:5432`、Qdrant `localhost:6334`、Neo4j `localhost:7687`——与 compose 暴露端口一致。
- `backend/docker.toml`（容器内）：主机名 `postgres`/`qdrant`/`neo4j`；`jwt.disabled=false`（P0 已改）。
- **必设密钥**：`APP_JWT_SECRET`（≥32 位、非占位；否则启动 fail-fast，P0.3）。`APP_NEO4J_PASSWORD`、`DATABASE_URL` 可覆盖。
- **sqlx 离线缓存**：改任何 `sqlx::query!` 宏的 SQL 后须连真库 `(cd backend && cargo sqlx prepare -- --tests)` 重生成 `backend/.sqlx`，否则 `SQLX_OFFLINE=true cargo check` 失效。db 层多为运行时 `sqlx::query`（不受影响）。

---

## 运行保障（P1 相关）

- **迁移**：app 启动自动应用 `backend/migrations/*`（含未提交的 `20260706000100_...tenant_foundation.sql` 与后续 P1 迁移）。
- **Testcontainers CI gate**：P1 的 RLS/outbox 行为无法离线验证，须建连真 PG/Qdrant 的集成测试（`#[ignore]` 默认跳过、CI 显式跑）作为放行门禁（见 runbook）。
- **A2A**：默认关闭 feature；启用需联网 pin a2a 依赖后 `cargo build --features a2a`（任务 #17）。
- **Plane B（MCP wasmtime）**：需 wasm 工具链（PR-8）。

---

## 恢复能力

- **本地**：`--reset` 删卷重来；数据非持久重要（dev）。
- **生产 HA/备份/恢复**：见 ADR-0005（托管基建：PG DBaaS Multi-AZ+PITR、Qdrant Cloud、Neo4j Aura）与 ADR-0003（运维就绪 gate）。本文档只覆盖 P1 开发/集成环境；生产上线前须补 `release-plan` + `launch-acceptance` + 恢复演练证据。

## 数据库角色与 RLS 安全（P1 新增）

P1 已交付四层记忆表（LTM/STM/KG/MM）的 schema-level RLS。**RLS 的有效性取决于连接数据库的角色**：

- **应用角色（推荐）**：`aetheris_app` — 非 superuser、非 BYPASSRLS，RLS 策略强制生效。跨租户查询被 DB 层拒绝。
- **运维角色**：`aetheris_admin` — superuser / BYPASSRLS，用于迁移、备份、恢复。**仅在运维窗口使用**。
- **默认 `memory` 角色**：docker-compose 默认角色可能为 superuser → RLS 成为 NO-OP。**生产部署必须替换为受限应用角色**。

### 角色创建（参考）

```sql
-- 应用角色（RLS 生效）
CREATE ROLE aetheris_app WITH LOGIN PASSWORD '<secure-password>';
GRANT CONNECT ON DATABASE memory TO aetheris_app;
GRANT USAGE ON SCHEMA public TO aetheris_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO aetheris_app;
-- 明确不授予 BYPASSRLS / SUPERUSER

-- 运维角色（仅运维窗口使用）
CREATE ROLE aetheris_admin WITH LOGIN PASSWORD '<secure-password>' SUPERUSER;
```

### 渗透验证

```bash
# 验证 RLS 生效：受限角色跨租户查询应返回 0 行
PGPASSWORD=... psql -U aetheris_app -d memory -c "
  SET aetheris.tenant_id = 'tenant-A';
  SELECT COUNT(*) FROM knowledge_entries WHERE tenant_id = 'tenant-B';
"
# 预期：0（RLS 策略拒绝，非应用层过滤）
```

## 备份与恢复演练（ADR-0003 要求）

生产发布前需完成以下演练并记录证据：

| 演练项 | 工具 | 验收标准 |
|--------|------|----------|
| PostgreSQL 全量备份 | `pg_dump` / DBaaS 自动备份 | 备份文件可恢复至新实例 |
| PostgreSQL PITR 恢复 | DBaaS PITR / WAL 归档 | 恢复到指定时间点，数据一致 |
| Qdrant 快照恢复 | Qdrant snapshot API | 快照恢复后向量搜索召回一致 |
| Neo4j 备份恢复 | `neo4j-admin dump/load` | 图数据完整恢复 |
| 跨区域恢复 | 托管基建跨区域复制 | 故障切换后服务可用 |
| 回滚演练 | 蓝绿/金丝雀部署 | 回滚后数据一致、无请求丢失 |

> **当前状态**：以上演练均未执行。P1 地基阶段完成后，需在 staging 环境逐一验证并记录证据至 `launch-acceptance.md`。

---

## 沙箱内当前状态（诚实标注）

- Docker daemon 未运行、无缓存镜像、5432/6334/7687/6379 均无服务、无网络 → **本会话无法实机拉起或验证 PG/Qdrant 行为**。
- 因此 P1 的 DB 行为（RLS 拒跨租户、outbox 幂等/对账、事务回滚）**尚未实机验证**——待上述基建就绪后按 runbook + Testcontainers 验证。
