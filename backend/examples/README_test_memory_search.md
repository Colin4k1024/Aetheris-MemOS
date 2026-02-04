# 记忆系统集成测试使用说明

## 概述

`test_memory_search.rs` 是一个集成测试脚本，用于验证记忆存储和搜索功能是否正常工作。

## 功能

1. **写入测试数据**：自动写入10条不同主题的记忆数据
2. **搜索测试**：使用不同的查询词进行搜索，验证是否能正确检索到写入的记忆
3. **结果验证**：验证搜索结果中包含预期的记忆条目

## 前置条件

### 1. 启动服务

首先需要启动后端服务：

```bash
cd /Users/jiafan/Desktop/work-code/adaptive-memory-system/backend
cargo run
```

服务默认运行在 `http://127.0.0.1:8008`

### 2. 确保依赖服务运行

测试需要以下服务运行：

- **Ollama** (LLM 和嵌入模型服务)
  - 默认地址: `http://localhost:11434`
  - 需要安装并运行 Ollama
  - 需要下载相应的模型（如 `llama2` 和 `nomic-embed-text`）

- **Milvus** (向量数据库)
  - 默认地址: `localhost:19530`
  - 需要安装并运行 Milvus

- **SQLite** (关系数据库)
  - 数据库文件: `./data/sqlx.sqlite`
  - 会自动创建

## 运行测试

### 方式1：使用默认配置

```bash
cd /Users/jiafan/Desktop/work-code/adaptive-memory-system/backend
cargo run --example test_memory_search
```

### 方式2：指定 API 地址

```bash
API_BASE_URL=http://localhost:8008 cargo run --example test_memory_search
```

## 测试流程

1. **服务检查**
   - 检查后端服务是否可用
   - 检查 Ollama 服务是否可用

2. **写入阶段**
   - 依次写入10条不同主题的记忆数据
   - 每条记忆包含：
     - Rust 编程语言
     - Python 编程语言
     - 数据库知识
     - 人工智能
     - Web 开发
     - 云计算
     - 网络安全
     - DevOps
     - 区块链
     - 微服务架构

3. **搜索测试**
   - 使用10个不同的查询词进行搜索
   - 验证每个查询是否能找到对应的记忆条目
   - 显示搜索结果和相似度分数

4. **结果验证**
   - 统计通过和失败的测试数量
   - 显示详细的测试结果

## 预期输出

```
=== 记忆系统集成测试开始 ===
API 基础 URL: http://127.0.0.1:8008
✓ 服务可用
✓ Ollama 服务可用
准备了 10 条测试记忆数据

=== 阶段1：写入记忆数据 ===
写入记忆 1/10: Rust 编程语言介绍
  ✓ 成功写入，entry_id: 01ARZ3NDEKTSV4RRFFQ69G5FAV
...

=== 阶段2：搜索测试 ===
测试查询: "Rust"
  找到 3 条结果
  ✓ 找到预期结果: entry_id=01ARZ3NDEKTSV4RRFFQ69G5FAV, score=0.8523
  ✓ 测试通过
    [0.8523] Rust 编程语言介绍 - Rust 是一种系统编程语言...

=== 测试总结 ===
总测试数: 10
通过: 10
失败: 0
=== 所有测试通过 ===
```

## 故障排查

### 问题1：服务不可用

**错误信息**：
```
服务不可用，请确保服务已启动: cargo run
```

**解决方案**：
1. 确保后端服务已启动：`cargo run`
2. 检查服务是否运行在正确的端口（默认 8008）
3. 检查防火墙设置

### 问题2：Ollama 服务不可用

**错误信息**：
```
⚠ Ollama 服务不可用: connection refused
```

**解决方案**：
1. 安装并启动 Ollama：https://ollama.ai/
2. 下载所需模型：
   ```bash
   ollama pull llama2
   ollama pull nomic-embed-text
   ```
3. 确保 Ollama 运行在 `http://localhost:11434`

### 问题3：写入失败

**错误信息**：
```
✗ 写入失败: 500 - Internal Server Error
```

**可能原因**：
1. LLM 服务不可用
2. 嵌入模型服务不可用
3. Milvus 连接失败
4. 数据库连接失败

**解决方案**：
1. 检查所有依赖服务是否正常运行
2. 查看后端服务日志获取详细错误信息
3. 检查配置文件 `config.toml`

### 问题4：搜索未找到结果

**错误信息**：
```
✗ 未找到预期的记忆条目
```

**可能原因**：
1. 数据还未完全处理（LLM 总结和向量化需要时间）
2. 向量相似度阈值过高
3. 查询词与内容不匹配

**解决方案**：
1. 增加等待时间（脚本中已设置5秒）
2. 检查向量数据库是否正常工作
3. 尝试使用更精确的查询词

## 自定义测试

### 修改测试数据

编辑 `test_memory_search.rs` 中的 `prepare_test_memories()` 函数，添加或修改测试记忆数据。

### 修改查询词

编辑 `run_tests()` 函数中的 `test_queries` 向量，添加或修改测试查询。

### 调整搜索参数

在 `SearchLTMRequest` 中修改：
- `top_k`: 返回结果数量
- `enable_rerank`: 是否启用重排序
- `min_score`: 最低相似度分数

## 注意事项

1. **测试数据会写入数据库**：测试会在数据库中创建真实的记忆条目，不会自动清理
2. **需要等待处理**：每条记忆写入后需要等待 LLM 总结和向量化完成
3. **网络依赖**：测试需要网络连接以访问 Ollama 服务
4. **资源消耗**：LLM 调用和向量化会消耗一定的计算资源

## 清理测试数据（可选）

如果需要清理测试数据，可以：

1. 手动删除数据库中的测试条目
2. 使用 Milvus 客户端删除向量数据
3. 或者重新初始化数据库（会删除所有数据）

## 相关文档

- [记忆系统集成指南](../docs/会话历史集成指南.md)
- [API 文档](http://localhost:8008/scalar)

