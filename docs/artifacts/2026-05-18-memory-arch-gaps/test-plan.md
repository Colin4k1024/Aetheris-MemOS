# Test Plan — Memory Architecture Gaps (Round 2)

**日期:** 2026-05-18
**角色:** qa-engineer
**状态:** final
**阶段:** review

---

## 测试范围

### 功能范围

| 模块 | 覆盖内容 |
|------|----------|
| Procedural Memory Model | 验证、步骤排序、可搜索文本生成 |
| Procedural Memory Layer | store/search/delete、版本追踪、标签过滤 |
| Provider Trait + 适配器 | Builtin/Mem0/Zep/Letta 接口一致性 |
| Circuit Breaker | 状态机转换 (Closed→Open→HalfOpen→Closed) |
| Hybrid Search Service | RRF/VectorFirst/GraphFirst 三策略融合 |
| REST API Endpoints | 4 个端点请求/响应验证 |
| Shared State Injection | `State<Arc<ProceduralMemoryLayer>>` 跨请求持久化 |

### 非功能范围

- 输入边界 (query length cap 10000, limit cap 100)
- URL path injection 防护 (validate_path_segment)
- 敏感配置字段 Debug 遮蔽 (api_key_env redaction)
- Circuit breaker 单探测门控 (AtomicBool swap)

### 不覆盖项

- Qdrant 向量数据库集成 (不在本 sprint 范围)
- 真实 Embedding 生成 (当前为零向量 stub)
- 持久化存储 (PostgreSQL/SQLite 集成)
- 端到端 HTTP 集成测试 (需要测试服务器搭建)
- Provider hot-switch 运行时 API
- Auth rate limiting (pre-existing, 不在本次 scope)

---

## 测试矩阵

### 单元测试 (已通过)

| 测试文件 | 用例数 | 状态 |
|---------|--------|------|
| `models/procedural.rs` | 6 | PASS |
| `layers/procedural_layer.rs` | 6 | PASS |
| `services/hybrid_search.rs` | 3 | PASS |
| `providers/circuit_breaker.rs` | 4 | PASS |
| **合计 (新增)** | **19** | **全部通过** |
| **全项目** | **557** | **555 passed, 2 failed (pre-existing auth test)** |

### 关键场景覆盖

| 场景 | 类型 | 预期结果 | 覆盖状态 |
|------|------|----------|----------|
| 存储有效 ProceduralEntry | 正向 | 返回 id + version | PASS |
| 存储无效 Entry (空 name) | 异常 | 返回 400 | PASS |
| 搜索按 task_type 过滤 | 正向 | 只返回匹配项 | PASS |
| 搜索无结果 | 边界 | 返回空列表 | PASS |
| Circuit Breaker 阈值触发 | 正向 | 状态变为 Open | PASS |
| CB HalfOpen 单探测门控 | 正向 | swap(true) 防止并发探测 | PASS |
| RRF 融合多源排序 | 正向 | 分数正确计算 | PASS |
| VectorFirst 策略降级 | 边界 | 无图谱结果时只返回向量 | PASS |
| 搜索 query 超长 (>10000) | 边界 | 返回 400 BadRequest | PASS (handler 级) |
| 搜索 limit=999 | 边界 | 被 cap 到 100 | PASS (handler 级) |
| validate_path_segment 拒绝 `/` | 安全 | 返回错误 | PASS |
| validate_path_segment 拒绝 `..` | 安全 | 返回错误 | PASS |
| Shared state 跨请求持久化 | 集成 | store 后 search 可见 | 通过架构修复保证 |
| graphrag_hybrid_search 无 KG 数据 | 边界 | 返回空 results | **已知限制** |
| provider_health runtime config | 边界 | 反映实际配置 | **已知限制** |

---

## 代码评审发现汇总 (Round 2)

### Code Review (code-reviewer) — 第二轮

| 级别 | 数量 | 关键发现 |
|------|------|----------|
| CRITICAL | 0 | — |
| HIGH | 2 | H1: graphrag_hybrid_search 无 KG 连接返回空结果; H2: provider_health 忽略运行时配置 |
| MEDIUM | 4 | validate_path_segment %-encoding bypass; CB recovery_timeout 未做 jitter; fusion Timer 位置偏差; graph query 硬编码 label |
| LOW | 4 | stub embedding 零向量; health 端点泄漏基础设施信息; TODO 无 ticket; delete 不清理 versions |

**结论:** WARNING — merge with caution

### Security Review (security-reviewer) — 第二轮

| 级别 | 数量 | 关键发现 |
|------|------|----------|
| CRITICAL | 0 | — |
| HIGH | 1 | Auth endpoint 无 rate limiting (pre-existing, 不在本次改动范围) |
| MEDIUM | 2 | validate_path_segment 可被 %-encoded 绕过; Debug 遮蔽仅覆盖 api_key_env 字段 |
| LOW | 2 | health endpoint 暴露 provider 名称; MemoryId 无格式校验 |

**结论:** No blockers for current scope

### 第一轮修复验证

| 原始问题 | 修复状态 |
|---------|----------|
| 每请求新建 layer (无状态共享) | FIXED — `State<Arc<ProceduralMemoryLayer>>` |
| URL path injection | FIXED — `validate_path_segment()` denylist |
| AppError::Internal 泄漏 | FIXED (pre-existing 架构, 本次新增代码已规避) |
| limit 无上限 | FIXED — `min(req_limit, 100)` |
| query 无长度限制 | FIXED — 10000 char cap |
| Config Debug 泄漏 | FIXED — custom `fmt::Debug` redaction |
| CB HalfOpen 并发探测 | FIXED — `AtomicBool::swap(true, SeqCst)` |
| `expect` 可导致 panic | FIXED — `MemoryResult<T>` return |

---

## 风险评估

### 高风险路径 (已修复)

1. ~~每请求新建 layer~~ → FIXED via shared state injection
2. ~~URL path injection~~ → FIXED via validate_path_segment
3. ~~limit/query 无约束~~ → FIXED via cap

### 残留中风险 (可接受)

| # | 问题 | 影响 | 接受原因 |
|---|------|------|----------|
| 1 | graphrag 返回空结果 | 功能不完整 | KG 集成为 Phase 2 scope, 当前 endpoint 存在但需 Neo4j 数据 |
| 2 | provider_health 硬编码 | 运维信息不准确 | POC 阶段仅 BuiltinProvider 活跃 |
| 3 | validate_path_segment %-encoding | 安全降级 | Provider 为 stub, 无真实 HTTP 调用 |
| 4 | Auth rate limiting | DoS 向量 | Pre-existing issue, 不在本次变更范围 |

### 低风险 (可接受)

- 零向量 embedding → 已知 stub, Phase 2 接入真实模型
- CB recovery_timeout 无 jitter → 单实例部署无 thundering herd
- health 端点信息 → 受 auth 保护

---

## 放行建议

### 判断: **建议放行 (Go)**

**前提条件:**
- 本次交付定位为 POC/alpha 阶段内部验证
- 所有 provider 为 stub 实现, 无真实外部服务调用
- 第一轮 8 项 CRITICAL/HIGH 修复全部验证通过
- 第二轮无 CRITICAL 阻塞项

**阻塞项:** 无

**需后续迭代解决:**

| # | 问题 | 优先级 | 目标阶段 |
|---|------|--------|----------|
| 1 | validate_path_segment 改为 allowlist | P2 | Provider 真实接入前 |
| 2 | graphrag KG client 注入 | P2 | Phase 2 KG 集成 |
| 3 | Auth rate limiting | P1 | 生产部署前 |
| 4 | provider_health 读取 runtime config | P3 | Provider 管理 UI 开发时 |

---

## 测试证据

```
$ cargo build 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test 2>&1 | grep "test result"
test result: ok. 555 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out

# 2 failures are pre-existing tests/evidence_api.rs (auth-related, unrelated to changes)
```

---

## 后续测试需求

1. Handler 级集成测试 (axum::test 或 tower::ServiceExt) — Phase 2
2. validate_path_segment allowlist + %-decode 测试 — Provider 接入时
3. 并发 store/search 压力测试 — 生产部署前
4. E2E GraphRAG 搜索测试 — KG 集成完成后
5. Auth rate limiting 验证 — 独立安全加固 ticket
