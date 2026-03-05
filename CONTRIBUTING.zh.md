# 贡献指南

感谢您对自适应记忆管理系统项目感兴趣。本文档涵盖如何构建、测试和提交更改，以及如何在系统中进行扩展。

## 文档参考

- **[架构说明 (ARCHITECTURE.md)](docs/ARCHITECTURE.md)** — 系统设计、层级图和决策流程。
- **[路线图 (ROADMAP.md)](docs/ROADMAP.md)** — 版本规划以及已完成与计划中的功能。
- **[使用场景 (USE_CASES.md)](docs/USE_CASES.md)** — LLM 智能体记忆、多模态、成本感知路由。
- **[扩展指南 (EXTENSION_GUIDE.md)](docs/EXTENSION_GUIDE.md)** — 如何添加新的策略和智能体。

## 构建与测试

### 后端 (Rust)

```bash
cd backend
cargo build
cargo test
```

- **Rust**: 1.89+
- **数据库**: SQLite (默认)；适配器规划请参见 [ROADMAP.md](docs/ROADMAP.md)。

### 前端 (React)

```bash
cd frontend/ant-design-pro-template
npm install
npm start
npm test
```

- **Node**: 20+

## 提交更改

1. **Fork** 仓库并从 `main` 或 `dev` 创建分支。
2. **实现** 您的更改；保持提交专注于单一改动。
3. **测试** — 运行 `cargo test` 和前端测试。
4. **提交 Pull Request** 并提供清晰的标题和描述。引用任何相关的 Issue。

## 扩展点

本项目设计为无需重构核心编排即可进行扩展：

- **权重策略** — 实现 `WeightStrategy` 特质并将您的策略添加到调整器链中，或使用 `DynamicWeightAdjuster::with_strategies`。请参阅 [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md)。
- **智能体** — 为新的或替换的分析器/预测器/调度器行为实现 `MemoryAgent` 特质。请参阅 [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md)。
- **决策追踪** — 追踪 API 和 UI 已就绪；持久化和关联功能正在规划中（请参阅 ROADMAP）。

如果您不确定更改应该放在哪里，或者如何添加新的策略/智能体，请提交 Issue 或参阅扩展指南。

## 代码规范与行为准则

- 遵循代码库中现有的风格和模式。
- 保持路由层无业务逻辑；保持服务层精简，将智能体/策略逻辑放在相应的模块中。
- 本项目基于 MIT 许可证提供。
