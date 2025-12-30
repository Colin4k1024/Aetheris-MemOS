# 自适应记忆管理系统

基于自适应记忆管理算法设计文档，使用 Rust (Salvo) 实现后端 API 服务，使用 React (Ant Design Pro) 实现前端管理界面。

## 项目结构

```
adaptive-memory-system/
├── backend/                    # Rust + Salvo 后端服务
│   ├── src/
│   │   ├── db/                # 数据库操作模块
│   │   │   ├── memory.rs      # 记忆配置仓库
│   │   │   ├── performance.rs # 性能指标仓库
│   │   │   └── weights.rs     # 权重历史仓库
│   │   ├── services/          # 核心服务层
│   │   │   ├── scheduler.rs   # 自适应记忆调度器
│   │   │   ├── analyzer.rs    # 任务特征分析器
│   │   │   ├── predictor.rs   # 性能预测模型
│   │   │   ├── monitor.rs     # 资源监控器
│   │   │   └── weight_adjuster.rs # 动态权重调整器
│   │   ├── routers/           # API 路由处理器
│   │   │   └── memory.rs      # 记忆管理相关端点
│   │   ├── models/            # 数据模型
│   │   └── main.rs            # 应用入口
│   ├── migrations/            # 数据库迁移文件
│   └── Cargo.toml             # Rust 依赖配置
├── frontend/                   # React + Ant Design Pro 前端应用
│   └── ant-design-pro-template/
│       ├── src/
│       │   ├── pages/         # 页面组件
│       │   │   ├── Dashboard/ # 仪表盘
│       │   │   ├── TaskAnalysis/ # 任务分析
│       │   │   ├── MemoryConfig/ # 记忆配置
│       │   │   ├── Performance/ # 性能监控
│       │   │   ├── ResourceMonitor/ # 资源监控
│       │   │   └── WeightHistory/ # 权重历史
│       │   └── services/      # API 服务封装
│       └── package.json       # 前端依赖配置
└── docs/                       # 设计文档
    ├── adaptive_memory_algorithm_design.md
    ├── adaptive_memory_api_specification.md
    └── adaptive_memory_algorithm_visualization.md
```

## 技术栈

### 后端
- **框架**: Salvo 0.84
- **语言**: Rust 1.89+
- **数据库**: SQLite (使用 SQLx)
- **异步运行时**: Tokio
- **序列化**: Serde
- **日志**: Tracing
- **认证**: JWT (jsonwebtoken)
- **配置**: Figment

### 前端
- **框架**: React 19+
- **UI库**: Ant Design Pro 6.0
- **图表库**: @ant-design/charts
- **构建工具**: Umi 4
- **状态管理**: Umi Max

## 核心功能

### 1. 自适应记忆调度
- 根据任务特征和资源约束自动选择最优记忆配置
- 支持短期记忆(STM)、长期记忆(LTM)、知识图谱(KG)、多模态记忆(MM)
- 动态权重调整机制

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

## 快速开始

### 环境要求

- Rust: 1.89+
- Node.js: 20+
- SQLite 3.x

### 后端开发

```bash
cd backend

# 安装依赖（首次运行）
cargo build

# 运行开发服务器
cargo run

# 服务器将在 http://127.0.0.1:8008 启动
```

### 前端开发

```bash
cd frontend/ant-design-pro-template

# 安装依赖（首次运行）
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

系统使用 SQLite 数据库，主要表结构包括：

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

### 其他表
- `context_sessions` - 短期记忆会话
- `context_messages` - 上下文消息
- `knowledge_entries` - 长期记忆条目
- `entities` - 知识图谱实体
- `relations` - 知识图谱关系
- `multimodal_entries` - 多模态记忆条目

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

### Docker 部署

```bash
# 构建镜像
docker-compose build

# 启动服务
docker-compose up -d
```

### 生产环境配置

1. 配置环境变量（数据库连接、JWT密钥等）
2. 设置日志级别
3. 配置 TLS/HTTPS
4. 设置资源限制

## 性能优化

- 数据库连接池已配置（最大10个连接）
- 前端图表使用虚拟滚动处理大数据量
- API 响应使用适当的缓存策略
- 日志记录使用结构化格式便于分析

## 故障排查

### 数据库连接失败
- 检查数据库文件路径是否正确
- 确认数据库文件权限
- 查看日志中的详细错误信息

### API 请求失败
- 检查后端服务是否运行
- 确认 API 路径和端口正确
- 查看浏览器控制台和网络请求

### 前端构建错误
- 清除 node_modules 和重新安装
- 检查 Node.js 版本是否符合要求
- 查看构建日志中的详细错误

## 贡献指南

1. Fork 项目
2. 创建功能分支
3. 提交更改
4. 推送到分支
5. 创建 Pull Request

## 许可证

MIT License

## 相关文档

- [算法设计文档](docs/adaptive_memory_algorithm_design.md)
- [API 规范文档](docs/adaptive_memory_api_specification.md)
- [算法可视化](docs/adaptive_memory_algorithm_visualization.md)
