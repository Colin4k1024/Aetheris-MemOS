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

---

## 沙箱内当前状态（诚实标注）

- Docker daemon 未运行、无缓存镜像、5432/6334/7687/6379 均无服务、无网络 → **本会话无法实机拉起或验证 PG/Qdrant 行为**。
- 因此 P1 的 DB 行为（RLS 拒跨租户、outbox 幂等/对账、事务回滚）**尚未实机验证**——待上述基建就绪后按 runbook + Testcontainers 验证。
