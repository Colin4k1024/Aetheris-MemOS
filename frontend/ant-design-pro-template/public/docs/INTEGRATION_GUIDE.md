# Adaptive Memory System 集成指南

本文档为开发者提供自适应记忆系统的完整集成指南，涵盖快速开始、集成模式、核心使用场景、多租户集成、分布式部署和最佳实践。

---

## 1. 快速开始

### 1.1 前置条件

| 依赖 | 版本要求 | 说明 |
|------|----------|------|
| Docker | 20.10+ | 用于运行 PostgreSQL、Qdrant、Neo4j |
| Docker Compose | 2.0+ | 服务编排 |
| Rust | 1.89+ | 后端开发/编译 |
| Node.js | 20+ | 前端开发（可选） |
| PostgreSQL | 14+ | 通过 Docker 自动提供 |
| Qdrant | Latest | 向量数据库，必需 |
| Neo4j | 5 | 知识图谱，可选 |

### 1.2 一键启动

项目根目录提供 `docker-compose.yml`，包含所有必需服务：

```bash
# 克隆项目后，在项目根目录执行
docker compose up -d

# 查看服务状态
docker compose ps

# 查看日志
docker compose logs -f backend
```

**启动的服务：**

| 服务 | 端口 | 说明 |
|------|------|------|
| backend | 8008 | Rust Axum 后端 API |
| postgres | 5432 | PostgreSQL + pgvector |
| qdrant | 6333/6334 | Qdrant 向量数据库 |
| neo4j | 7474/7687 | Neo4j 图数据库（可选） |

### 1.3 健康检查

服务启动后，验证系统运行状态：

```bash
# 检查后端健康状态
curl http://localhost:8008/api/memory/health

# 响应示例
{
  "status": "healthy",
  "timestamp": "2026-03-29T10:00:00Z",
  "services": {
    "database": "connected",
    "qdrant": "connected",
    "neo4j": "not_configured"
  }
}

# 检查内存系统状态
curl http://localhost:8008/api/memory/adaptive/status

# 检查资源监控
curl http://localhost:8008/api/memory/monitor/resources
```

---

## 2. 集成模式

### 2.1 REST API 集成

REST API 是最常用的集成方式，适合所有语言和平台。

#### 认证

所有受保护的 API 需要 Bearer Token 认证：

```bash
# 登录获取 Token
curl -X POST http://localhost:8008/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# 响应
# {"token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."}

# 使用 Token 访问受保护 API
curl -X GET http://localhost:8008/api/memory/adaptive/status \
  -H "Authorization: Bearer <your_token>"
```

#### 核心 API 流程

**流程 1：自适应记忆选择**

根据任务上下文自动选择最优记忆配置：

```bash
curl -X POST http://localhost:8008/api/memory/adaptive/select \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "task_context": "用户询问如何学习Python编程，需要详细的学习路线和建议",
    "expected_complexity": "medium",
    "reasoning_depth": "deep"
  }'
```

**响应示例：**

```json
{
  "selection": {
    "config_id": "config_001",
    "weights": {
      "stm": 0.3,
      "ltm": 0.4,
      "kg": 0.3,
      "mm": 0.0
    },
    "use_stm": true,
    "use_ltm": true,
    "use_kg": true,
    "use_mm": false
  },
  "prediction": {
    "efficiency_gain": 0.75,
    "coherence_gain": 0.82,
    "resource_cost": 0.45,
    "confidence": 0.88
  }
}
```

**流程 2：记忆存储**

```bash
# 存储短期记忆（会话消息）
curl -X POST http://localhost:8008/api/memory/storage/stm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "role": "user",
    "content": "请介绍一下Python语言",
    "maxContextLength": 4096,
    "retentionHours": 24
  }'

# 响应
# {"sessionId": "session_01abc123...", "messageId": "msg_01def456..."}

# 存储长期记忆
curl -X POST http://localhost:8008/api/memory/storage/ltm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "sourceId": "doc_python_001",
    "sourceType": "document",
    "title": "Python语言简介",
    "content": "Python是一种高级编程语言...",
    "category": "编程语言",
    "domain": "计算机科学"
  }'

# 响应
# {"entryId": "ltm_01abc123..."}
```

**流程 3：记忆搜索**

```bash
# 向量搜索长期记忆
curl -X POST http://localhost:8008/api/memory/search/ltm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "query": "机器学习 神经网络",
    "topK": 5,
    "enableRerank": true,
    "minScore": 0.5
  }'

# 混合搜索（向量 + 关键词）
curl -X POST http://localhost:8008/api/memory/search/hybrid \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "query": "Python 编程 学习",
    "topK": 5,
    "keywordWeight": 0.3,
    "vectorWeight": 0.7
  }'
```

#### Python 示例

```python
import requests

BASE_URL = "http://localhost:8008/api"

class AdaptiveMemoryClient:
    def __init__(self, username: str, password: str):
        self.token = self._login(username, password)
        self.headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.token}"
        }

    def _login(self, username: str, password: str) -> str:
        resp = requests.post(
            f"{BASE_URL}/login",
            json={"username": username, "password": password}
        )
        resp.raise_for_status()
        return resp.json()["token"]

    def select_memory_config(self, task_context: str, **kwargs):
        """自适应选择记忆配置"""
        resp = requests.post(
            f"{BASE_URL}/memory/adaptive/select",
            json={"task_context": task_context, **kwargs},
            headers=self.headers
        )
        resp.raise_for_status()
        return resp.json()

    def store_stm(self, user_id: str, agent_id: str, role: str, content: str):
        """存储短期记忆"""
        resp = requests.post(
            f"{BASE_URL}/memory/storage/stm",
            json={
                "userId": user_id,
                "agentId": agent_id,
                "sessionType": "conversation",
                "role": role,
                "content": content,
                "maxContextLength": 4096,
                "retentionHours": 24
            },
            headers=self.headers
        )
        resp.raise_for_status()
        return resp.json()

    def store_ltm(self, source_id: str, title: str, content: str):
        """存储长期记忆"""
        resp = requests.post(
            f"{BASE_URL}/memory/storage/ltm",
            json={
                "sourceId": source_id,
                "sourceType": "document",
                "title": title,
                "content": content
            },
            headers=self.headers
        )
        resp.raise_for_status()
        return resp.json()

    def search_ltm(self, query: str, top_k: int = 5):
        """搜索长期记忆"""
        resp = requests.post(
            f"{BASE_URL}/memory/search/ltm",
            json={"query": query, "topK": top_k},
            headers=self.headers
        )
        resp.raise_for_status()
        return resp.json()

    def health_check(self):
        """健康检查"""
        resp = requests.get(f"{BASE_URL}/memory/health")
        resp.raise_for_status()
        return resp.json()


# 使用示例
client = AdaptiveMemoryClient("admin", "admin123")

# 1. 自适应选择
config = client.select_memory_config(
    "用户询问如何学习Python",
    expected_complexity="medium",
    reasoning_depth="deep"
)
print(f"选择的配置: {config['selection']['weights']}")

# 2. 存储会话
session = client.store_stm("user_001", "agent_001", "user", "请介绍一下Python")
print(f"会话ID: {session['sessionId']}")

# 3. 搜索知识
results = client.search_ltm("编程语言 学习路线")
for r in results["results"]:
    print(f"  - {r['title']} (score: {r['score']:.2f})")
```

#### 错误处理

```python
import requests
from requests.exceptions import HTTPError

def safe_api_call(func):
    def wrapper(*args, **kwargs):
        try:
            return func(*args, **kwargs)
        except HTTPError as e:
            if e.response.status_code == 401:
                raise Exception("认证失败，请重新登录")
            elif e.response.status_code == 400:
                error_detail = e.response.json()
                raise Exception(f"参数错误: {error_detail.get('message', str(e))}")
            elif e.response.status_code == 503:
                raise Exception("服务不可用，请检查依赖服务(Qdrant/Neo4j)是否运行")
            else:
                raise Exception(f"API错误: {e}")
    return wrapper

@safe_api_call
def store_stm_safe(client, **kwargs):
    return client.store_stm(**kwargs)
```

### 2.2 嵌入式集成

可将记忆系统作为 Rust 库嵌入到其他 Rust 项目中。

#### 添加依赖

```toml
# Cargo.toml
[dependencies]
adaptive-memory = { path = "../adaptive-memory-system/backend" }
tokio = { version = "1", features = ["full"] }
```

#### 直接使用服务

```rust
use adaptive_memory::services::{
    MemoryStorageService, MemorySearchService,
    Scheduler, Analyzer, Predictor,
};
use adaptive_memory::db::{stm::STMRepository, ltm::LTMRepository};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化服务
    let storage = MemoryStorageService;
    let search = MemorySearchService::new()?;
    let scheduler = Scheduler::new();
    let analyzer = Analyzer::new();
    let predictor = Predictor::new();

    // 1. 存储短期记忆
    let (session_id, message_id) = storage
        .store_stm(
            "user_001",
            "agent_001",
            "conversation",
            "user",
            "请介绍一下Python",
            4096,
            24,
        )
        .await?;

    println!("STM存储成功: session={}, message={}", session_id, message_id);

    // 2. 存储长期记忆
    let entry_id = storage
        .store_ltm(
            "doc_001",
            "document",
            "Python是一种高级编程语言...",
            Some("Python语言简介"),
        )
        .await?;

    println!("LTM存储成功: entry={}", entry_id);

    // 3. 搜索长期记忆
    let results = search
        .search_ltm("编程语言 学习", 5, 0.5, true)
        .await?;

    for r in results {
        println!("  - {} (score: {:.2})", r.title, r.score);
    }

    // 4. 自适应选择
    let task_context = "用户询问如何学习Python编程";
    let characteristics = analyzer.analyze(task_context).await?;
    let prediction = predictor.predict(&characteristics).await?;
    let selection = scheduler.select(&characteristics, &prediction).await?;

    println!("选择的权重: {:?}", selection.weights);

    Ok(())
}
```

#### 无 HTTP 的内存模式

对于高性能场景，可以直接调用服务层而不启动 HTTP 服务器：

```rust
use adaptive_memory::services::MemoryOrchestrator;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let orchestrator = MemoryOrchestrator::new().await?;

    // 单次任务执行：分析 -> 存储 -> 检索
    let result = orchestrator.execute_task(
        "user_001",
        "agent_001",
        "用户询问Python的问题",
    ).await?;

    println!("任务执行完成: {:?}", result);
    Ok(())
}
```

### 2.3 LLM Agent 集成

LLM Agent 通过 Observe-Decide-Act 循环与记忆系统交互。

#### Agent 循环架构

```
┌─────────────────────────────────────────────────────────────┐
│                      LLM Agent                              │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐               │
│  │ Observe │ -> │  Decide │ -> │   Act   │               │
│  └────┬────┘    └────┬────┘    └────┬────┘               │
│       │               │               │                    │
│       v               v               v                    │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐               │
│  │ 查询记忆 │    │ 生成策略 │    │ 存储结果 │               │
│  └────┬────┘    └────┬────┘    └────┬────┘               │
│       │               │               │                    │
└───────┼───────────────┼───────────────┼────────────────────┘
        │               │               │
        v               v               v
┌───────────────────────────────────────────────────────────────┐
│                   Adaptive Memory System                      │
│  STM (短期记忆)  │  LTM (长期记忆)  │  KG (知识图谱)  │  MM  │
└───────────────────────────────────────────────────────────────┘
```

#### Agent 代码示例

```python
import requests
from typing import List, Dict, Optional

class LLMAgent:
    def __init__(self, agent_id: str, memory_client: AdaptiveMemoryClient):
        self.agent_id = agent_id
        self.memory = memory_client
        self.session_id: Optional[str] = None

    def observe(self, task_context: str) -> Dict:
        """观察阶段：分析任务并查询相关记忆"""
        # 1. 自适应选择记忆配置
        config = self.memory.select_memory_config(task_context)
        weights = config["selection"]["weights"]

        # 2. 根据配置权重查询不同记忆
        results = {
            "stm": [],
            "ltm": [],
            "kg": [],
            "config": config["selection"]
        }

        if weights.get("stm", 0) > 0:
            # 查询短期记忆（当前会话上下文）
            if self.session_id:
                stm_results = self.memory.search_stm(task_context)
                results["stm"] = stm_results.get("results", [])

        if weights.get("ltm", 0) > 0:
            # 查询长期记忆（向量搜索）
            ltm_results = self.memory.search_ltm(task_context, top_k=5)
            results["ltm"] = ltm_results.get("results", [])

        if weights.get("kg", 0) > 0:
            # 查询知识图谱（实体关系）
            kg_results = self.memory.search_kg(task_context)
            results["kg"] = kg_results.get("entities", [])

        return results

    def decide(self, observation: Dict, available_tools: List[str]) -> str:
        """决策阶段：决定下一步行动（通常由 LLM 执行）"""
        # 这里可以调用 LLM 进行决策
        # 返回要执行的工具/动作
        pass

    def act(self, action: str, context: Dict) -> Dict:
        """行动阶段：执行动作并存储结果"""
        # 执行动作...

        # 存储执行结果到记忆
        if self.session_id:
            self.memory.store_stm(
                user_id="system",
                agent_id=self.agent_id,
                role="assistant",
                content=f"执行了动作: {action}, 结果: {context}"
            )

        return {"action": action, "status": "success"}

    def run_session(self, messages: List[Dict]) -> str:
        """运行完整的多轮对话会话"""
        # 创建会话
        session = self.memory.create_session(self.agent_id)
        self.session_id = session["sessionId"]

        for msg in messages:
            # 1. 观察
            observation = self.observe(msg["content"])

            # 2. 决策（这里简化处理）
            decision = self.decide(observation, [])

            # 3. 行动
            result = self.act(decision, observation)

            # 4. 存储回复
            self.memory.store_stm(
                user_id=msg.get("user_id", "user"),
                agent_id=self.agent_id,
                role=msg.get("role", "user"),
                content=msg["content"]
            )

        return self.session_id


# 使用示例
agent = LLMAgent("agent_001", memory_client)

messages = [
    {"role": "user", "content": "我想学习Python编程"},
    {"role": "user", "content": "应该从哪里开始？"},
    {"role": "user", "content": "推荐一些学习资源"},
]

session_id = agent.run_session(messages)
print(f"会话完成: {session_id}")
```

#### 会话管理

```python
class SessionManager:
    """管理 Agent 的多轮对话会话"""

    def __init__(self, memory_client: AdaptiveMemoryClient):
        self.client = memory_client
        self.sessions: Dict[str, str] = {}  # agent_id -> session_id

    def get_or_create_session(self, agent_id: str, user_id: str) -> str:
        """获取或创建会话"""
        if agent_id not in self.sessions:
            # 创建新会话
            session = self.client.store_stm(
                userId=user_id,
                agentId=agent_id,
                sessionType="conversation",
                role="system",
                content="[会话开始]"
            )
            self.sessions[agent_id] = session["sessionId"]
        return self.sessions[agent_id]

    def add_message(self, agent_id: str, role: str, content: str):
        """添加消息到会话"""
        session_id = self.get_or_create_session(agent_id, "default_user")
        self.client.store_stm(
            userId="default_user",
            agentId=agent_id,
            sessionType="conversation",
            role=role,
            content=content
        )

    def get_context(self, agent_id: str, max_messages: int = 10) -> List[Dict]:
        """获取最近的消息上下文"""
        session_id = self.sessions.get(agent_id)
        if not session_id:
            return []

        messages = self.client.get_session_messages(session_id)
        return messages[-max_messages:]

    def finalize_session(self, agent_id: str):
        """结束会话，触发 STM -> LTM 转移"""
        if agent_id in self.sessions:
            session_id = self.sessions[agent_id]
            # 将会话内容转移到长期记忆
            self.client.transfer_stm_to_ltm(session_id)
            del self.sessions[agent_id]
```

---

## 3. 核心使用场景

### 3.1 短期记忆 (STM) - 会话上下文

#### 何时使用 STM

- 多轮对话的上下文保持
- 当前任务的状态存储
- 临时计算结果的缓存
- 用户的即时请求和响应

#### 存储会话消息

```bash
# 存储用户消息
curl -X POST http://localhost:8008/api/memory/storage/stm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "role": "user",
    "content": "请帮我写一个快速排序算法",
    "maxContextLength": 4096,
    "retentionHours": 24
  }'

# 存储助手回复
curl -X POST http://localhost:8008/api/memory/storage/stm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "role": "assistant",
    "content": "当然可以，这里是一个快速排序的实现..."
  }'
```

#### 会话生命周期

```python
class SessionLifecycle:
    def __init__(self, client: AdaptiveMemoryClient):
        self.client = client

    def create_session(self, user_id: str, agent_id: str) -> str:
        """创建新会话"""
        result = self.client.store_stm(
            userId=user_id,
            agentId=agent_id,
            sessionType="conversation",
            role="system",
            content="[会话开始]"
        )
        return result["sessionId"]

    def get_session_messages(self, session_id: str) -> List[Dict]:
        """获取会话消息"""
        resp = requests.get(
            f"{BASE_URL}/memory/storage/stm/{session_id}",
            headers=self.headers
        )
        return resp.json().get("messages", [])

    def close_session(self, session_id: str):
        """关闭会话（触发自动过期或转移到 LTM）"""
        # 可选：手动转移到 LTM
        self.client.transfer_stm_to_ltm(session_id)
```

#### 自动过期行为

STM 消息根据 `retentionHours` 参数自动过期：

| retentionHours | 适用场景 |
|----------------|----------|
| 1-6 | 短期问答、即时任务 |
| 24 | 单轮对话、日常使用 |
| 72 | 多天项目、长时间任务 |
| 168 | 周级别任务 |

### 3.2 长期记忆 (LTM) - 持久化知识

#### 何时使用 LTM

- 跨会话持久化知识
- 文档和知识库存储
- 用户偏好和历史记录
- 事实性知识和答案

#### 语义搜索设置（Qdrant）

```bash
# 配置 Qdrant 连接（在环境变量或配置文件中）
# QDRANT_URL=http://localhost:6334
# QDRANT_COLLECTION=memory_ltm

# 存储知识条目
curl -X POST http://localhost:8008/api/memory/storage/ltm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "sourceId": "doc_ai_001",
    "sourceType": "document",
    "title": "人工智能发展历史",
    "content": "人工智能经历了三个主要阶段：1950年代的符号主义，1980年代的机器学习，以及2010年至今的深度学习革命。",
    "category": "技术发展",
    "domain": "AI"
  }'
```

#### 批量存储

```bash
curl -X POST http://localhost:8008/api/memory/storage/batch-ltm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "entries": [
      {
        "sourceId": "doc_001",
        "sourceType": "document",
        "title": "Python基础",
        "content": "Python是一种高级编程语言..."
      },
      {
        "sourceId": "doc_002",
        "sourceType": "document",
        "title": "Python进阶",
        "content": "Python的装饰器和生成器是高级特性..."
      },
      {
        "sourceId": "doc_003",
        "sourceType": "document",
        "title": "Python Web开发",
        "content": "Flask和Django是Python的Web框架..."
      }
    ]
  }'
```

#### Embedding 模型配置

```bash
# 配置 Ollama embedding 服务
# OLLAMA_BASE_URL=http://localhost:11434
# EMBEDDING_MODEL=nomic-embed-text

# 搜索时自动使用配置的 embedding 模型
curl -X POST http://localhost:8008/api/memory/search/ltm \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "query": "Python Web框架比较",
    "topK": 5
  }'
```

### 3.3 知识图谱 (KG) - 实体关系

#### 何时使用 KG

- 结构化知识存储
- 实体关系推理
- 复杂查询和多跳关系
- 推荐系统和关系分析

#### Neo4j 设置（可选）

```bash
# Neo4j 通过 docker-compose 自动启动
# 连接信息
# NEO4J_URI=bolt://localhost:7687
# NEO4J_USER=neo4j
# NEO4J_PASSWORD=password

# 如不使用 Neo4j，知识图谱功能将回退到 PostgreSQL 存储
```

#### 创建实体和关系

```bash
# 创建实体
curl -X POST http://localhost:8008/api/kg/entities \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "entityName": "埃隆·马斯克",
    "entityType": "人物",
    "description": "企业家和工程师，特斯拉和SpaceX的创始人",
    "aliases": ["Elon Musk", "马斯克"]
  }'

# 创建另一个实体
curl -X POST http://localhost:8008/api/kg/entities \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "entityName": "特斯拉",
    "entityType": "公司",
    "description": "电动汽车和能源公司"
  }'

# 创建关系
curl -X POST http://localhost:8008/api/kg/relations \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "sourceEntityId": "entity_elon_musk",
    "targetEntityId": "entity_tesla",
    "relationType": "founded",
    "relationName": "创立",
    "weight": 0.95
  }'
```

#### 图查询模式

```bash
# 按名称搜索实体
curl -X GET "http://localhost:8008/api/kg/entities/by-name/马斯克" \
  -H "Authorization: Bearer <token>"

# 获取实体相关关系
curl -X GET "http://localhost:8008/api/kg/entities/{entity_id}/related" \
  -H "Authorization: Bearer <token>"

# 实体搜索（混合查询）
curl -X POST http://localhost:8008/api/kg/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "entityName": "人工智能",
    "topK": 10
  }'
```

### 3.4 多模态记忆 (MM) - 跨模态存储

#### 支持的模态

| 模态类型 | 说明 | 存储格式 |
|----------|------|----------|
| image | 图片 | Base64 / URL |
| audio | 音频 | Base64 / URL |
| video | 视频 | Base64 / URL |
| text | 文本 | Plain text |

#### 存储和检索

```bash
# 存储多模态记忆
curl -X POST http://localhost:8008/api/mm/store \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "modalityType": "image",
    "content": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
    "metadata": {
      "description": "架构图示意",
      "source": "用户上传"
    }
  }'

# 获取多模态记忆
curl -X GET "http://localhost:8008/api/mm/entry/{entry_id}" \
  -H "Authorization: Bearer <token>"

# 按模态类型查询
curl -X GET "http://localhost:8008/api/mm/modality/image" \
  -H "Authorization: Bearer <token>"

# 获取会话的多模态记忆
curl -X GET "http://localhost:8008/api/mm/session/{session_id}" \
  -H "Authorization: Bearer <token>"
```

#### 跨模态检索

```python
def cross_modal_search(client: AdaptiveMemoryClient, query: str):
    """跨模态检索示例"""
    # 1. 文本查询
    text_results = client.search_ltm(query, top_k=5)

    # 2. 获取相关图片
    image_results = requests.get(
        f"{BASE_URL}/mm/modality/image",
        headers=client.headers
    ).json()

    # 3. 获取相关音频
    audio_results = requests.get(
        f"{BASE_URL}/mm/modality/audio",
        headers=client.headers
    ).json()

    return {
        "text": text_results,
        "images": image_results.get("entries", []),
        "audio": audio_results.get("entries", [])
    }
```

---

## 4. 多租户集成

### 4.1 租户隔离机制

所有数据通过 `tenant_id` 进行作用域隔离：

```bash
# 注册租户
curl -X POST http://localhost:8008/api/tenants/ \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "tenant_id": "tenant_001",
    "name": "示例公司",
    "max_stm_sessions": 100,
    "max_ltm_entries": 10000,
    "enable_cross_agent_sharing": true
  }'
```

### 4.2 租户上下文传播

每个 API 请求需要包含租户信息：

```bash
# 在请求头或请求体中指定 tenant_id
curl -X POST http://localhost:8008/api/memory/storage/stm \
  -H "Content-Type: application/json" \
  -H "X-Tenant-ID: tenant_001" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "tenantId": "tenant_001",
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "role": "user",
    "content": "消息内容"
  }'
```

### 4.3 配额管理

```bash
# 获取租户配额状态
curl -X GET "http://localhost:8008/api/memory/billing/quota/{tenant_id}" \
  -H "Authorization: Bearer <token>"

# 响应示例
{
  "tenant_id": "tenant_001",
  "quota": {
    "max_stm_sessions": 100,
    "max_ltm_entries": 10000,
    "used_stm_sessions": 15,
    "used_ltm_entries": 2340
  }
}
```

### 4.4 租户 API 认证

```python
class TenantAwareClient(AdaptiveMemoryClient):
    def __init__(self, tenant_id: str, username: str, password: str):
        super().__init__(username, password)
        self.tenant_id = tenant_id
        self.headers["X-Tenant-ID"] = tenant_id

    def store_stm(self, **kwargs):
        kwargs["tenantId"] = self.tenant_id
        return super().store_stm(**kwargs)

    def search_ltm(self, query: str, top_k: int = 5):
        resp = requests.post(
            f"{BASE_URL}/tenants/{self.tenant_id}/search",
            json={"query": query, "top_k": top_k},
            headers=self.headers
        )
        resp.raise_for_status()
        return resp.json()
```

---

## 5. 分布式部署

### 5.1 何时部署分布式

| 场景 | 推荐部署 |
|------|----------|
| 单应用、低并发 | 单机部署 |
| 多租户、高并发 | 水平扩展 |
| 大规模知识库 | 分片部署 |
| 高可用要求 | 集群部署 |

### 5.2 节点发现和集群形成

```bash
# 注册集群节点
curl -X POST http://localhost:8008/api/memory/enterprise/cluster/node \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "nodeId": "node_001",
    "host": "192.168.1.101",
    "port": 8008,
    "capacity": 100
  }'

# 获取集群节点列表
curl -X GET http://localhost:8008/api/memory/enterprise/cluster/nodes \
  -H "Authorization: Bearer <token>"

# 获取活跃节点
curl -X GET http://localhost:8008/api/memory/enterprise/cluster/active \
  -H "Authorization: Bearer <token>"

# 获取主节点
curl -X GET http://localhost:8008/api/memory/enterprise/cluster/leader \
  -H "Authorization: Bearer <token>"
```

### 5.3 复制和一致性

```bash
# 创建数据分片
curl -X POST http://localhost:8008/api/memory/enterprise/shards \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "shardKey": "tenant_001",
    "replicationFactor": 3
  }'

# 获取分片信息
curl -X GET http://localhost:8008/api/memory/enterprise/shards \
  -H "Authorization: Bearer <token>"

# 获取特定 key 的分片
curl -X GET http://localhost:8008/api/memory/enterprise/shards/tenant_001 \
  -H "Authorization: Bearer <token>"
```

### 5.4 子 Agent 池

```bash
# 获取池状态
curl -X GET http://localhost:8008/api/v1/distributed/pool/status \
  -H "Authorization: Bearer <token>"

# 分配槽位
curl -X POST http://localhost:8008/api/v1/distributed/pool/allocate \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{"workflow_id": "workflow_001", "slots": 2}'

# 释放槽位
curl -X POST http://localhost:8008/api/v1/distributed/pool/release \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{"workflow_id": "workflow_001"}'
```

---

## 6. 最佳实践

### 6.1 性能优化

#### 连接池

```python
import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry

def create_session_with_pool():
    """创建带连接池的请求会话"""
    session = requests.Session()

    # 配置连接池
    adapter = HTTPAdapter(
        pool_connections=10,  # 连接池数量
        pool_maxsize=20,     # 每个池的最大连接数
        max_retries=Retry(
            total=3,
            backoff_factor=0.1,
            status_forcelist=[500, 502, 503, 504]
        )
    )

    session.mount("http://", adapter)
    session.mount("https://", adapter)

    return session

# 全局会话
http_session = create_session_with_pool()
```

#### 缓存策略

```python
from functools import lru_cache
import moka

# 使用 moka 缓存
cache = moka.Cache.AsyncTrieCache(
    max_size=1000,
    ttl=300  # 5分钟
)

@lru_cache(maxsize=128)
def get_config_cached(config_id: str):
    """缓存配置查询"""
    resp = requests.get(f"{BASE_URL}/memory/config/{config_id}")
    return resp.json()

# 对于频繁查询的记忆配置
async def get_memory_config_cached(agent_id: str):
    return await cache.get_or_insert(agent_id, load_config)
```

### 6.2 安全

#### 输入验证

```python
import re
from typing import Optional

def validate_input(content: str, max_length: int = 10000) -> Optional[str]:
    """验证和清理输入"""
    if not content:
        return None

    # 长度检查
    if len(content) > max_length:
        raise ValueError(f"内容长度超过限制: {len(content)} > {max_length}")

    # 移除潜在的危险字符（根据场景调整）
    # 这里不做过度清理，保持实用性
    return content.strip()

def validate_session_type(session_type: str) -> bool:
    """验证会话类型"""
    valid_types = {"conversation", "task", "query", "batch"}
    return session_type in valid_types
```

#### MCP Sandbox 使用

```bash
# 使用 MCP 安全执行提示
curl -X POST http://localhost:8008/api/v1/security/prompt-probe/check \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "prompt": "用户输入的提示内容",
    "context": {"user_id": "user_001", "agent_id": "agent_001"}
  }'
```

### 6.3 监控

#### 健康检查

```python
import time
from threading import Thread

class HealthMonitor:
    def __init__(self, client: AdaptiveMemoryClient, interval: int = 60):
        self.client = client
        self.interval = interval
        self.running = False

    def start(self):
        self.running = True
        self.thread = Thread(target=self._monitor)
        self.thread.daemon = True
        self.thread.start()

    def stop(self):
        self.running = False

    def _monitor(self):
        while self.running:
            try:
                health = self.client.health_check()
                print(f"[{time.ctime()}] Health: {health['status']}")

                # 检查各服务状态
                services = health.get("services", {})
                for name, status in services.items():
                    if status != "connected":
                        print(f"  WARNING: {name} is {status}")
            except Exception as e:
                print(f"[{time.ctime()}] Health check failed: {e}")

            time.sleep(self.interval)

# 使用
monitor = HealthMonitor(client, interval=30)
monitor.start()
```

#### 资源限制

```bash
# 监控资源使用
curl -X GET http://localhost:8008/api/memory/monitor/resources \
  -H "Authorization: Bearer <token>"

# 响应示例
{
  "cpu": {"usage_percent": 45.2, "cores": 8},
  "memory": {"used_mb": 2048, "total_mb": 8192, "usage_percent": 25.0},
  "storage": {"used_gb": 50, "total_gb": 500, "usage_percent": 10.0}
}
```

### 6.4 成本管理

#### Qdrant 资源管理

```bash
# Qdrant 默认配置（docker-compose 中）
# QDRANT_HOST=qdrant
# QDRANT_PORT=6334
# QDRANT_REST_PORT=6333

# 定期清理不需要的向量数据
# 监控 Qdrant 存储使用
curl -X GET "http://localhost:6333/collections/memory_ltm" | jq '.result'
```

#### Neo4j 资源管理

```bash
# Neo4j 资源限制（docker-compose 中配置）
# NEO4J_dbms_memory_heap_initial__size=512m
# NEO4J_dbms_memory_heap_max__size=2g

# 监控 Neo4j 查询性能
# Neo4j Browser: http://localhost:7474
```

---

## 7. API 参考速查

### 自适应记忆

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/api/memory/adaptive/select` | 选择最优记忆配置 |
| GET | `/api/memory/adaptive/status` | 获取记忆系统状态 |
| POST | `/api/memory/adaptive/trace` | 获取决策追踪 |
| POST | `/api/memory/analyzer/task-characteristics` | 分析任务特征 |
| POST | `/api/memory/predictor/performance` | 预测性能 |
| POST | `/api/memory/monitor/cost-benefit` | 计算成本效益 |
| POST | `/api/memory/weights/adjust` | 调整权重 |
| GET | `/api/memory/weights/history` | 获取权重历史 |

### 存储

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/api/memory/storage/stm` | 存储短期记忆 |
| GET | `/api/memory/storage/stm/{session_id}` | 获取会话消息 |
| POST | `/api/memory/storage/ltm` | 存储长期记忆 |
| POST | `/api/memory/storage/transfer` | STM 转移到 LTM |
| POST | `/api/memory/storage/batch-ltm` | 批量存储 LTM |
| GET | `/api/memory/storage/sessions` | 列出所有会话 |

### 搜索

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/api/memory/search/stm` | 搜索短期记忆 |
| POST | `/api/memory/search/ltm` | 搜索长期记忆（向量） |
| GET | `/api/memory/search/ltm/{entry_id}` | 获取 LTM 条目 |
| POST | `/api/memory/search/hybrid` | 混合搜索 |
| POST | `/api/memory/search/entity` | 实体搜索 |
| POST | `/api/memory/search/triple` | 三元组搜索 |

### 知识图谱

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/api/kg/entities` | 列出实体 |
| POST | `/api/kg/entities` | 创建实体 |
| GET | `/api/kg/entities/by-name/{name}` | 按名称查询实体 |
| GET | `/api/kg/entities/{id}/related` | 获取相关实体 |
| POST | `/api/kg/relations` | 创建关系 |
| POST | `/api/kg/search` | 搜索实体 |

### 多模态

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/api/mm/store` | 存储多模态记忆 |
| GET | `/api/mm/entry/{entry_id}` | 获取条目 |
| GET | `/api/mm/session/{session_id}` | 获取会话的多模态 |
| GET | `/api/mm/modality/{type}` | 按类型获取 |
| GET | `/api/mm/list` | 列出所有 |

### 系统

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/api/memory/health` | 健康检查 |
| GET | `/api/memory/v1/health` | 深度健康检查 |
| GET | `/api/memory/config` | 获取配置 |
| GET | `/api/memory/monitor/resources` | 资源监控 |
| GET | `/api/memory/baselines` | 获取性能基线 |

### 多租户

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/api/tenants/` | 列出租户 |
| POST | `/api/tenants/` | 注册租户 |
| POST | `/api/tenants/{id}/search` | 租户搜索 |
| GET | `/api/tenants/{id}/sessions` | 租户会话 |
| POST | `/api/tenants/access/check` | 访问检查 |

### 分布式

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/api/memory/enterprise/cluster/node` | 注册节点 |
| GET | `/api/memory/enterprise/cluster/nodes` | 获取节点列表 |
| POST | `/api/memory/enterprise/shards` | 创建分片 |
| GET | `/api/v1/distributed/pool/status` | 池状态 |
| POST | `/api/v1/distributed/pool/allocate` | 分配槽位 |

---

## 附录：错误代码参考

| 状态码 | 说明 | 常见原因 |
|--------|------|----------|
| 400 | 请求参数错误 | 缺少必填字段、格式不正确 |
| 401 | 认证失败 | Token 无效或过期 |
| 403 | 权限不足 | 租户无权限访问资源 |
| 404 | 资源不存在 | session_id、entry_id 不存在 |
| 429 | 请求过于频繁 | 触发限流 |
| 500 | 服务器错误 | 内部异常 |
| 503 | 服务不可用 | 依赖服务(Qdrant/Neo4j)未启动 |

---

*本文档版本: 1.0.0*
*最后更新: 2026-03-29*
