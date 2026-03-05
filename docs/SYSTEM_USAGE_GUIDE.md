# 自适应内存系统使用指南

## 1. 系统简介

自适应内存系统是一个智能记忆管理系统，旨在为智能体提供高效、自适应的记忆管理能力。该系统通过动态调整记忆权重、优化资源分配和智能调度记忆策略，实现了记忆系统的自适应优化，提高了智能体的性能和效率。

## 2. 架构设计

### 2.1 整体架构

系统采用前后端分离架构：

- **后端**：基于 Rust + Salvo 框架开发，提供 RESTful API 服务
- **前端**：基于 React + Ant Design Pro 开发，提供可视化操作界面
- **数据库**：
  - 主要数据：SQLite (可扩展到 PostgreSQL)
  - 向量存储：Qdrant
  - 知识图谱：Neo4j (计划中)

### 2.2 核心组件

1. **自适应记忆调度器**：根据任务上下文和资源约束选择最优记忆配置
2. **任务特征分析器**：分析任务特征，确定记忆需求
3. **性能预测模型**：预测特定记忆配置的性能表现
4. **资源监控与优化器**：监控系统资源使用情况，提供优化建议
5. **动态权重调整器**：动态调整各记忆层的权重
6. **记忆存储管理**：管理短期记忆(STM)和长期记忆(LTM)
7. **记忆搜索模块**：提供多种记忆搜索方式

## 3. 部署要求

### 3.1 硬件要求

- CPU：4 核以上
- 内存：8GB 以上
- 存储：100GB 以上
- 网络：稳定的网络连接

### 3.2 软件要求

#### 后端

- Rust 1.89+
- Cargo
- SQLite 3.0+
- Qdrant 1.7+
- Neo4j 4.0+ (可选)

#### 前端

- Node.js 16+
- npm 或 yarn
- 现代浏览器

## 4. 配置说明

### 4.1 后端配置

后端配置文件位于 `backend/config.toml`，主要配置项包括：

```toml
# 服务器配置
listen_addr = "127.0.0.1:8008"

# 数据库配置
[db]
url = "file:./data/sqlx.sqlite"

# JWT认证配置（生产环境请使用强随机密钥，勿使用示例值）
[jwt]
secret = "<your-jwt-secret>"
expiry = 3600

# 日志配置
[log]
file_name = "app.log"
rolling = "daily"

# LLM配置
[llm]
base_url = "http://localhost:11434"
model = "llama3"
timeout_seconds = 30

# 嵌入模型配置
[embedding]
base_url = "http://localhost:11434"
model = "nomic-embed-text"
dimension = 768
timeout_seconds = 30

# Qdrant向量数据库配置
[qdrant]
host = "localhost"
port = 6334
collection_name = "long_term_memory"
vector_dimension = 768
distance_type = "Euclid"

# Rerank重排序配置
[rerank]
base_url = "http://localhost:11434"
model = "bge-reranker-base"
enabled = true
candidate_multiplier = 2
min_score_threshold = 0.3
timeout_seconds = 30

# Neo4j图数据库配置
[neo4j]
host = "localhost"
port = 7687
username = "neo4j"
password = "<your-neo4j-password>"
database = "neo4j"

### 4.2 前端配置

前端配置文件位于 `frontend/ant-design-pro-template/config/config.ts`，主要配置项包括：

```ts
// API请求配置
export default {
  // 开发环境API地址
  dev: {
    baseURL: 'http://127.0.0.1:8008',
  },
  // 生产环境API地址
  test: {
    baseURL: 'https://api.example.com',
  },
  // 测试环境API地址
  pre: {
    baseURL: 'https://api.pre.example.com',
  },
};
```

## 5. 快速启动

### 5.1 后端启动

1. 进入后端目录：
   ```bash
   cd backend
   ```

2. 启动开发服务器：
   ```bash
   cargo run
   ```

3. 启动生产服务器：
   ```bash
   cargo build --release
   ./target/release/backend
   ```

### 5.2 前端启动

1. 进入前端目录：
   ```bash
   cd frontend/ant-design-pro-template
   ```

2. 安装依赖：
   ```bash
   npm install
   ```

3. 启动开发服务器：
   ```bash
   npm run dev
   ```

4. 构建生产版本：
   ```bash
   npm run build
   ```

## 6. 使用流程

### 6.1 基本使用流程

1. **启动服务**：启动后端和前端服务
2. **访问系统**：在浏览器中访问前端地址（默认：http://localhost:8000）
3. **登录系统**：使用默认管理员账号登录
4. **配置系统**：根据需求配置系统参数
5. **使用API**：通过前端界面或直接调用API使用系统功能
6. **监控性能**：通过前端界面监控系统性能

### 6.2 API调用流程

1. **获取认证Token**：通过登录API获取Bearer Token
2. **分析任务特征**：调用任务特征分析API分析任务
3. **选择记忆配置**：调用自适应记忆选择API获取最优配置
4. **执行任务**：使用选择的记忆配置执行任务
5. **监控资源**：调用资源监控API监控系统状态
6. **优化配置**：根据监控结果优化记忆配置

## 7. 常见问题

### 7.1 后端启动失败

- 检查依赖是否安装正确
- 检查配置文件是否正确
- 检查端口是否被占用

### 7.2 前端无法连接后端

- 检查后端服务是否正常运行
- 检查前端配置中的API地址是否正确
- 检查网络连接是否正常

### 7.3 Qdrant连接失败

- 检查Qdrant服务是否正常运行
- 检查Qdrant配置是否正确
- 检查防火墙设置

### 7.4 记忆搜索结果不准确

- 检查嵌入模型配置是否正确
- 检查向量维度是否匹配
- 尝试调整搜索参数

## 8. 联系方式

如有任何问题或建议，请联系系统管理员或开发团队。

---

**版本**: 1.0.0  
**最后更新**: 2025-12-30