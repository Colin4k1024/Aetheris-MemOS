# 自适应记忆管理系统

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Adaptive Memory Management System for Agent & LLM Workloads**

基于自适应记忆管理算法设计文档，使用 Rust (Axum) 实现后端 API 服务，使用 React (Ant Design Pro) 实现前端管理界面。本项目采用 MIT 许可证对外开源，欢迎参与贡献与二次开发。

- **许可证**: [LICENSE](LICENSE)（MIT）
- **安全**: 漏洞反馈请参见 [SECURITY.md](SECURITY.md)
- **变更记录**: [CHANGELOG.md](CHANGELOG.md)

## 项目结构

```
adaptive-memory-system/
├── backend/                    # Rust + Axum 后端服务
│   ├── src/
│   │   ├── agent/             # 智能体模块
│   │   ├── config/            # 配置模块
│   │   ├── db/                # 数据库操作模块
│   │   │   ├── adapters/      # 数据库适配器
│   │   │   ├── memory.rs      # 记忆配置仓库
│   │   │   ├── performance.rs # 性能指标仓库
│   │   │   ├── weights.rs     # 权重历史仓库
│   │   │   ├── stm.rs        # 短期记忆仓库
│   │   │   ├── ltm.rs        # 长期记忆仓库
│   │   │   ├── kg.rs         # 知识图谱仓库
│   │   │   ├── mm.rs         # 多模态记忆仓库
│   │   │   ├── neo4j.rs      # Neo4j 图数据库
│   │   │   └── decision_trace.rs # 决策追踪
│   │   ├── services/          # 核心服务层
│   │   │   ├── scheduler.rs   # 自适应记忆调度器
│   │   │   ├── analyzer.rs    # 任务特征分析器
│   │   │   ├── predictor.rs   # 性能预测模型
│   │   │   ├── monitor.rs     # 资源监控器
│   │   │   ├── weight_adjuster.rs # 动态权重调整器
│   │   │   ├── weight_strategy.rs # 权重策略
│   │   │   ├── agent.rs       # 智能体接口
│   │   │   ├── embedding.rs   # 嵌入模型服务
│   │   │   ├── llm.rs         # LLM 服务
│   │   │   ├── memory_search.rs # 记忆搜索服务
│   │   │   ├── memory_storage.rs # 记忆存储服务
│   │   │   ├── memory_transfer.rs # 记忆转移服务
│   │   │   ├── multimodal_memory.rs # 多模态记忆服务
│   │   │   ├── qdrant.rs      # Qdrant 向量数据库
│   │   │   └── rerank.rs      # 重排序服务
│   │   ├── routers/           # API 路由处理器
│   │   │   ├── memory.rs      # 记忆管理端点
│   │   │   ├── auth.rs        # 认证端点
│   │   │   ├── user.rs       # 用户管理端点
│   │   │   ├── knowledge_graph.rs # 知识图谱端点
│   │   │   ├── memory_search.rs # 记忆搜索端点
│   │   │   ├── memory_storage.rs # 记忆存储端点
│   │   │   └── multimodal.rs # 多模态端点
│   │   ├── models/            # 数据模型
│   │   ├── hoops/              # 中间件 (CORS, JWT)
│   │   ├── kernel/             # 内核模块
│   │   ├── layers/             # 层级模块
│   │   ├── policy/             # 策略模块
│   │   ├── protocol/           # 协议模块
│   │   ├── runtime/            # 运行时模块
│   │   ├── tenant/             # 租户模块
│   │   ├── utils/             # 工具模块
│   │   ├── error.rs           # 错误处理
│   │   └── main.rs            # 应用入口
│   ├── config.toml            # 配置文件
│   └── Cargo.toml             # Rust 依赖配置
├── frontend/                   # React + Ant Design Pro 前端应用
│   └── ant-design-pro-template/
│       ├── src/
│       │   ├── pages/         # 页面组件
│       │   │   ├── Dashboard/ # 仪表盘
│       │   │   ├── TaskAnalysis/ # 任务分析
│       │   │   ├── MemoryConfig/ # 记忆配置
│       │   │   ├── MemoryDecisionTrace/ # 决策追踪
│       │   │   ├── MemoryManagement/ # 记忆管理
│       │   │   ├── Performance/ # 性能监控
│       │   │   ├── ResourceMonitor/ # 资源监控
│       │   │   └── WeightHistory/ # 权重历史
│       │   └── services/      # API 服务封装
│       └── package.json       # 前端依赖配置
└── docs/                       # 设计文档
    ├── ARCHITECTURE.md        # 架构说明
    ├── SYSTEM_USAGE_GUIDE.md  # 系统使用指南
    ├── API_USAGE_GUIDE.md     # API 使用指南
    ├── FRONTEND_USAGE_GUIDE.md # 前端使用指南
    ├── EXTENSION_GUIDE.md     # 扩展指南
    ├── USE_CASES.md           # 使用场景
    └── adaptive_memory_*.md    # 算法设计与 API 规范
```

## 技术栈

### 后端

- **框架**: Axum
- **语言**: Rust 1.89+
- **数据库**: PostgreSQL (使用 SQLx)
- **异步运行时**: Tokio
- **序列化**: Serde
- **日志**: Tracing
- **认证**: JWT (jsonwebtoken)
- **配置**: Figment

### 外部服务

- **向量数据库**: Qdrant (长期记忆向量存储)
- **图数据库**: Neo4j (知识图谱存储)
- **LLM 服务**: Ollama (本地 LLM)
- **嵌入模型**: Ollama nomic-embed-text

### 前端

- **框架**: React 18+
- **UI库**: Ant Design Pro 6.0
- **图表库**: @ant-design/charts
- **构建工具**: Umi 4
- **状态管理**: React Hooks

## 核心功能

### 1. 自适应记忆调度

- 根据任务特征和资源约束自动选择最优记忆配置
- 支持短期记忆(STM)、长期记忆(LTM)、知识图谱(KG)、多模态记忆(MM)
- 动态权重调整机制
- 决策追踪 (Decision Trace) - 完整的决策链路可追溯

### 2. 任务特征分析

- 复杂度评估
- 模态需求检测
- 推理深度评估
- 上下文依赖度分析

### 3. 性能预测

- 基于研究数据的性能基准
- 边际效益递减补偿
- 协同效应计算
- 资源成本估算

### 4. 资源监控

- 实时资源使用监控
- 成本效益分析
- 资源优化建议
- 告警机制

### 5. 权重调整历史

- 记录所有权重调整操作
- 性能影响追踪
- 趋势分析

### 6. 记忆搜索与存储

- 语义搜索 (基于向量嵌入)
- 关键词搜索
- 混合搜索
- 记忆自动转移 (STM → LTM)
- 知识图谱查询

### 7. 多模态支持

- 文本、图像、音频、视频记忆存储
- 跨模态检索
- 多模态嵌入生成

## 快速开始

### 环境要求

- Rust: 1.89+
- Node.js: 20+
- PostgreSQL 14+ (可通过 Docker 启动)
- Qdrant (可通过 Docker 启动)
- Neo4j (可选，知识图谱功能需要)

### 后端开发

```bash
cd backend

# 启动 PostgreSQL 和 Qdrant (使用 Docker)
docker compose up -d postgres qdrant

# 安装依赖并编译
cargo build

# 运行开发服务器
cargo run

# 服务器将在 http://127.0.0.1:8008 启动
```

### 前端开发

```bash
cd frontend/ant-design-pro-template

# 安装依赖
npm install

# 启动开发服务器
npm start

# 应用将在 http://localhost:8000 启动
```

## API 使用示例

### 1. 自适应记忆选择

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/adaptive" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "task_id": "task_001",
      "task_type": "query",
      "complexity": 0.75,
      "modality_requirements": ["text", "image"],
      "temporal_scope": "medium",
      "reasoning_depth": "deep",
      "context_dependency": 0.6,
      "user_id": "user_1",
      "agent_id": "agent_1"
    },
    "resource_constraints": {
      "max_memory_usage_mb": 1024,
      "max_cpu_usage_percent": 80,
      "max_response_time_ms": 2000,
      "storage_quota_percent": 90
    },
    "preferences": {
      "prioritize_efficiency": true,
      "prioritize_coherence": false,
      "enable_multimodal": true,
      "enable_reasoning": true
    }
  }'
```

### 2. 分析任务特征

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/analyzer/task-characteristics" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "content": "请分析这个复杂的多模态数据",
      "modality": ["text", "image"],
      "context_history": []
    }
  }'
```

### 3. 获取资源状态

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/monitor/resources"
```

### 4. 获取权重调整历史

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/weights/history"
```

## 数据库架构

系统使用 PostgreSQL 数据库，主要表结构包括：

### 记忆配置表 (memory_configurations)

- 存储用户和智能体的记忆配置
- 支持多种配置类型（default, custom, optimized）
- 记录各记忆层的启用状态和参数

### 性能指标表 (performance_metrics)

- 记录系统性能指标
- 支持按时间范围查询
- 提供聚合统计功能

### 权重调整历史表 (weight_adjustment_history)

- 记录所有权重调整操作
- 包含调整前后的权重对比
- 记录性能影响和调整原因

### 决策追踪表 (decision_traces)

- 记录完整的决策链路
- 包含分析器、预测器、调度器的决策过程
- 支持决策回溯和解释

### 短期记忆表 (context_sessions / context_messages)

- 会话管理
- 消息历史存储

### 长期记忆表 (knowledge_entries)

- 向量嵌入存储 (Qdrant)
- 元数据和标签

### 知识图谱表 (entities / relations)

- Neo4j 图数据库存储
- 实体和关系管理

### 多模态记忆表 (multimodal_entries)

- 跨模态记忆存储
- 图像、音频、视频元数据

## 开发指南

### 添加新的 API 端点

1. 在 `backend/src/routers/memory.rs` 中添加新的处理函数
2. 使用 `#[endpoint]` 宏标记函数
3. 在 `backend/src/routers/mod.rs` 中注册路由

### 添加新的数据库操作

1. 在相应的仓库模块（`db/memory.rs`, `db/performance.rs`, `db/weights.rs`）中添加方法
2. 使用 SQLx 进行类型安全的查询
3. 添加适当的错误处理

### 前端页面开发

1. 在 `frontend/ant-design-pro-template/src/pages/` 下创建新页面
2. 使用 Ant Design Pro 组件和图表库
3. 在 `config/routes.ts` 中配置路由

## 测试

### 后端测试

```bash
cd backend
cargo test
```

### 前端测试

```bash
cd frontend/ant-design-pro-template
npm test
```

## 部署

### Docker 部署 (推荐)

```bash
# 启动所有服务 (PostgreSQL, Qdrant, 后端)
docker compose up -d

# 仅启动数据库服务
docker compose up -d postgres qdrant

# 查看日志
docker compose logs -f
```

### 手动部署

1. **启动 PostgreSQL**

   ```bash
   docker run -d --name postgres \
     -e POSTGRES_USER=memory \
     -e POSTGRES_PASSWORD=memory \
     -e POSTGRES_DB=memory \
     -p 5432:5432 postgres:14
   ```

2. **启动 Qdrant**

   ```bash
   docker run -d --name qdrant \
     -p 6333:6333 \
     -p 6334:6334 \
     qdrant/qdrant
   ```

3. **配置并启动后端**
   ```bash
   cp config.toml config.toml.bak
   # 编辑 config.toml 设置数据库连接
   cargo run --release
   ```

### 生产环境配置

1. 配置环境变量（数据库连接、JWT密钥、Neo4j密码等）
2. 设置日志级别
3. 配置 TLS/HTTPS
4. 设置资源限制
5. 配置 Neo4j (可选，用于知识图谱功能)

## 性能优化

- 数据库连接池已配置（最大10个连接）
- 前端图表使用虚拟滚动处理大数据量
- API 响应使用适当的缓存策略
- 日志记录使用结构化格式便于分析

## 故障排查

### 数据库连接失败

- 检查 PostgreSQL 服务是否运行: `docker ps`
- 确认数据库连接配置正确
- 查看日志中的详细错误信息

### Qdrant 连接失败

- 检查 Qdrant 服务是否运行: `docker ps`
- 确认端口 6334 (gRPC) 已开放
- 验证向量维度配置是否匹配

### Neo4j 连接失败 (知识图谱)

- 检查 Neo4j 服务是否运行
- 确认用户名和密码正确
- 验证数据库配置

### API 请求失败

- 检查后端服务是否运行: `curl http://127.0.0.1:8008/api/v1/memory/health`
- 确认 API 路径和端口正确
- 查看浏览器控制台和网络请求
- 检查 JWT 认证是否过期

### 前端构建错误

- 清除 node_modules 和重新安装: `rm -rf node_modules && npm install`
- 检查 Node.js 版本是否符合要求 (20+)
- 查看构建日志中的详细错误

## 贡献指南

详见 [CONTRIBUTING.md](CONTRIBUTING.md)：构建与测试、提交流程、扩展点（策略与 Agent）。扩展新策略或 Agent 请参考 [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md)。

## 许可证与开源

本项目采用 [MIT License](LICENSE) 开源。你可以在遵守许可证条款的前提下自由使用、修改和分发本软件。安全相关问题请通过 [SECURITY.md](SECURITY.md) 中说明的方式报告。

## 相关文档

- [v1.0 里程碑审计 (.planning/v1.0-MILESTONE-AUDIT.md)](.planning/v1.0-MILESTONE-AUDIT.md) — 版本完成状态与需求覆盖
- [变更记录 (CHANGELOG)](CHANGELOG.md) — 版本更新与改动说明
- [安全政策 (SECURITY)](SECURITY.md) — 漏洞报告与支持版本
- [行为准则 (CODE_OF_CONDUCT)](CODE_OF_CONDUCT.md) — 社区参与规范
- [开源审查清单 (OPEN_SOURCE_CHECKLIST)](docs/OPEN_SOURCE_CHECKLIST.md) — 开源就绪项与发布前待办
- [架构说明 (ARCHITECTURE)](docs/ARCHITECTURE.md) — Why adaptive / Why agent-like / 决策链路
- [路线图 (ROADMAP)](docs/ROADMAP.md) — 版本规划与生态对齐
- [使用场景 (USE_CASES)](docs/USE_CASES.md) — LLM Agent、多模态、成本敏感推理等
- [贡献指南 (CONTRIBUTING)](CONTRIBUTING.md) — 构建、测试、PR、扩展点
- [扩展指南 (EXTENSION_GUIDE)](docs/EXTENSION_GUIDE.md) — 新增 WeightStrategy / MemoryAgent
- [Axum 迁移说明](docs/why-axum.md)
- [算法设计文档](docs/adaptive_memory_algorithm_design.md)
- [API 规范文档](docs/adaptive_memory_api_specification.md)
- [算法可视化](docs/adaptive_memory_algorithm_visualization.md)
