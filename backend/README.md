# 自适应记忆系统 (Adaptive Memory System)

江海浮天一叶舟，久困囚居终有时。初入 Rust Web，可能需适应，但熟练后如鱼得水，必将安然入眠，性能可靠，亦少错漏。
祝君不负韶华，道阻且长，行则将至。

## 项目简介

自适应记忆系统是一个基于 Rust 和 Salvo 框架开发的智能记忆管理系统，具有以下核心功能：

- 多模态记忆存储和检索
- 知识图谱构建和查询
- 记忆自动转移（从短期记忆到长期记忆）
- 自适应记忆选择和优化
- 混合搜索（关键词+向量）
- 性能监控和预测
- 动态权重调整

## 技术栈

- **Web 框架**: Salvo
- **ORM**: SQLx
- **数据库**: SQLite (支持扩展到其他数据库)
- **向量存储**: Qdrant
- **嵌入模型**: Ollama
- **语言模型**: Ollama
- **认证**: JWT
- **日志**: Tracing
- **缓存**: Moka

## 项目架构

```
├── src/
│   ├── config/          # 配置管理
│   ├── db/             # 数据库仓库
│   │   ├── stm.rs      # 短期记忆
│   │   ├── ltm.rs      # 长期记忆
│   │   ├── kg.rs       # 知识图谱
│   │   ├── mm.rs       # 多模态记忆
│   │   ├── memory.rs   # 记忆配置
│   │   └── weights.rs  # 权重历史
│   ├── hoops/          # 中间件
│   ├── models/         # 数据模型
│   ├── routers/        # API 路由
│   ├── services/       # 业务服务
│   │   ├── analyzer.rs          # 任务特征分析器
│   │   ├── embedding.rs         # 嵌入服务
│   │   ├── llm.rs               # 语言模型服务
│   │   ├── memory_search.rs     # 记忆搜索服务
│   │   ├── memory_storage.rs    # 记忆存储服务
│   │   ├── memory_transfer.rs   # 记忆转移服务
│   │   ├── monitor.rs           # 资源监控服务
│   │   ├── predictor.rs         # 性能预测模型
│   │   ├── scheduler.rs         # 自适应调度器
│   │   └── weight_adjuster.rs   # 动态权重调整器
│   └── utils/          # 工具函数
├── migrations/         # 数据库迁移
├── data/               # 数据文件
├── config.toml         # 配置文件
└── Cargo.toml          # 依赖管理
```

## 快速开始

### 运行项目

```shell
// 运行项目
cargo run

// 运行测试
cargo test

// 构建发布版本
cargo build --release
```

### 访问 API 文档

项目启动后，可以通过以下地址访问 API 文档：

- **Swagger UI**: http://localhost:8008/scalar
- **OpenAPI JSON**: http://localhost:8008/api-doc/openapi.json

### 初始账号

开源版本不包含预设账号。请通过 `POST /api/register` 注册首个用户，或部署后按需创建管理员账号。

## 主要功能模块

### 1. 记忆管理

- **短期记忆 (STM)**: 存储会话上下文，具有自动过期机制
- **长期记忆 (LTM)**: 存储持久化的知识条目
- **知识图谱 (KG)**: 构建实体和关系网络
- **多模态记忆 (MM)**: 支持文本、图像、音频、视频等多种模态

### 2. 记忆搜索

- **STM 搜索**: 基于会话和时间范围的短期记忆检索
- **LTM 搜索**: 基于向量相似性的长期记忆检索
- **混合搜索**: 结合关键词和向量的混合搜索
- **实体搜索**: 基于知识图谱的实体关联搜索

### 3. 自适应记忆系统

- **任务特征分析**: 分析任务的复杂度、模态需求等特征
- **性能预测**: 预测不同记忆配置的性能表现
- **资源监控**: 实时监控系统资源使用情况
- **动态权重调整**: 根据任务特征和资源状况调整记忆权重
- **记忆自动转移**: 当会话达到阈值时自动转移到长期记忆

## 配置文件

主要配置文件为 `config.toml`，包含以下配置项：

- **服务器配置**: 监听地址、端口
- **数据库配置**: 连接字符串、连接池设置
- **JWT 配置**: 密钥、过期时间
- **LLM 配置**: Ollama 地址、模型名称
- **嵌入配置**: 嵌入模型、维度
- **Qdrant 配置**: 向量数据库连接信息
- **Rerank 配置**: 重排序模型设置
- **记忆转移配置**: 检查间隔、阈值

## API 端点

### 认证 API

- `POST /api/login`: 用户登录
- `POST /api/login/account`: 使用 token 登录
- `GET /api/currentUser`: 获取当前用户信息

### 记忆 API

#### 自适应记忆
- `POST /api/v1/memory/adaptive`: 选择记忆配置
- `GET /api/v1/memory/adaptive`: 获取记忆状态

#### 任务特征分析
- `POST /api/v1/memory/analyzer/task-characteristics`: 分析任务特征
- `POST /api/v1/memory/analyzer/batch-characteristics`: 批量分析任务特征

#### 性能预测
- `POST /api/v1/memory/predictor/performance`: 预测性能
- `GET /api/v1/memory/predictor/baselines`: 获取基线数据

#### 资源监控
- `GET /api/v1/memory/monitor/resources`: 获取资源状态
- `POST /api/v1/memory/monitor/cost-benefit`: 计算成本效益比
- `POST /api/v1/memory/monitor/optimize`: 优化资源使用

#### 权重调整
- `POST /api/v1/memory/weights/adjust`: 调整权重
- `GET /api/v1/memory/weights/history`: 获取权重历史

#### 记忆存储
- `POST /api/v1/memory/storage/stm`: 存储短期记忆
- `GET /api/v1/memory/storage/stm/{session_id}`: 获取会话消息
- `POST /api/v1/memory/storage/ltm`: 存储长期记忆
- `POST /api/v1/memory/storage/transfer`: 手动转移记忆
- `POST /api/v1/memory/storage/batch-ltm`: 批量存储长期记忆

#### 记忆搜索
- `POST /api/v1/memory/search/stm`: 搜索短期记忆
- `POST /api/v1/memory/search/ltm`: 搜索长期记忆
- `GET /api/v1/memory/search/ltm/{entry_id}`: 获取长期记忆条目
- `POST /api/v1/memory/search/hybrid`: 混合搜索
- `POST /api/v1/memory/search/entity`: 实体搜索

## 开发指南

### 数据库迁移

使用 SQLx CLI 进行数据库迁移：

```shell
# 安装 sqlx-cli
cargo install sqlx-cli

# 创建新迁移
sqlx migrate add <migration_name>

# 运行迁移
sqlx migrate run

# 回滚迁移
sqlx migrate revert
```

### 代码风格

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量

## 关于 Salvo

你可以在 https://salvo.rs/ 📖查看 Salvo 的文档以及更多例子，如果我们的工具帮到你，欢迎 star [salvo](https://github.com/salvo-rs/salvo) 和 [salvo-cli](https://github.com/salvo-rs/salvo-cli),这将给我们很大激励。❤️️

## 许可证

MIT License
