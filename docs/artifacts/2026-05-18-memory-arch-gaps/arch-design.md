# Architecture Design: Memory Architecture Gaps

## System Boundary

### In Scope (MemOS Responsibility)
- Memory storage, retrieval, and search across all layer types
- Procedural Memory as a new first-class layer
- External memory backend integration via provider abstraction
- Hybrid search orchestration (GraphRAG)
- REST API for all new capabilities

### Out of Scope
- Agent Runtime (reasoning, orchestration, skill execution)
- User profiling / emotional modeling
- Frontend changes (this iteration)

### External Dependencies
| Service | Role | Required For |
|---------|------|-------------|
| PostgreSQL 14+ | Persistence, metadata | All layers |
| Qdrant | Vector similarity search | LTM, Hybrid Search |
| Neo4j | Graph traversal, KG queries | KG, Hybrid Search |
| Redis | STM cache, session state | STM Layer |
| Mem0 API | External memory backend | US-2 (provider) |
| Zep API | External memory backend | US-2 (provider) |

---

## Component Architecture

### Trait Hierarchy (Design Decision)

```
MemoryKernel (orchestrator)
  ├── MemoryLayer (internal storage layers)
  │     ├── StmMemoryLayer
  │     ├── LtmMemoryLayer
  │     ├── KgMemoryLayer
  │     ├── MmMemoryLayer
  │     └── ProceduralMemoryLayer  ← NEW
  │
  ├── MemoryProvider (external backends)  ← NEW, parallel to Layer
  │     ├── BuiltinProvider (wraps internal layers)
  │     ├── Mem0Provider (HTTP client)
  │     ├── ZepProvider (HTTP client)
  │     └── LettaProvider (stub)
  │
  └── HybridSearchService (search orchestration)  ← NEW
        ├── VectorSearch (Qdrant)
        └── GraphMemory (Neo4j)
```

**Key Decision: MemoryProvider is parallel to MemoryLayer, not nested.**

Rationale from challenge session:
- `MemoryLayer` represents internal storage layers with full lifecycle control
- `MemoryProvider` represents external memory systems accessed via HTTP API
- External backends don't support eviction, direct stats, or layer-level control
- The Kernel selects whether to delegate to internal layers or external providers based on configuration

---

## US-1: Procedural Memory Layer

### Data Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralEntry {
    pub name: String,
    pub description: String,
    pub task_type: String,
    pub steps: Vec<ProceduralStep>,
    pub preconditions: Vec<String>,
    pub tools_used: Vec<String>,
    pub success_rate: f64,
    pub execution_count: u32,
    pub version: u32,
    pub context: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralStep {
    pub order: u32,
    pub action: String,
    pub tool: Option<String>,
    pub parameters: HashMap<String, Value>,
    pub expected_output: Option<String>,
    pub fallback: Option<String>,
}
```

**Storage strategy:** Uses `MemoryContent::Json(Value)` at the kernel level. `ProceduralMemoryLayer` validates JSON against the `ProceduralEntry` schema on store/retrieve. This avoids adding a new `MemoryContent` variant (no serde breaking change).

### LayerType Extension

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    Stm,
    Ltm,
    Kg,
    Mm,
    Procedural,  // NEW - backward compatible via serde rename
}
```

### Search Capabilities

ProceduralMemoryLayer search supports:
- By `task_type` (exact match via metadata tags)
- By `tool name` (filter by tools_used field)
- By semantic similarity (embedding of description + step summaries)
- Results sorted by `success_rate * recency_weight`

### Version Evolution

Same-name procedures create new versions. Query returns highest-success-rate version by default, with option to list all versions.

---

## US-2: Memory Provider Abstraction

### Trait Definition

```rust
#[async_trait::async_trait]
pub trait MemoryProvider: Send + Sync {
    fn provider_name(&self) -> &str;

    fn capabilities(&self) -> ProviderCapabilities;

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId>;

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry>;

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>>;

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()>;

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()>;

    async fn health_check(&self) -> MemoryResult<ProviderHealth>;
}

#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub supports_vector_search: bool,
    pub supports_graph: bool,
    pub supports_metadata_filter: bool,
    pub supports_eviction: bool,
    pub max_entry_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub status: HealthStatus,
    pub latency_ms: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unavailable,
}
```

### Provider Selection (Config-Driven)

```toml
[memory.provider]
active = "builtin"  # builtin | mem0 | zep | letta

[memory.provider.mem0]
api_url = "http://localhost:8080"
api_key_env = "MEM0_API_KEY"
timeout_ms = 5000

[memory.provider.zep]
api_url = "http://localhost:8000"
api_key_env = "ZEP_API_KEY"
timeout_ms = 5000
```

### Circuit Breaker

Each external provider wraps calls with:
- Timeout (configurable, default 5s)
- Retry with exponential backoff (max 3 attempts)
- Circuit breaker: open after 5 consecutive failures, half-open after 30s

### Builtin Provider

Wraps the existing `MemoryLayer` chain. Delegates store/retrieve/search to the appropriate internal layer based on `MemoryEntry.layer`.

---

## US-3: GraphRAG Hybrid Search

### HybridSearchService

```rust
pub struct HybridSearchService {
    vector_search: Arc<dyn VectorSearch>,
    graph_memory: Arc<dyn GraphMemory>,
    config: HybridSearchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchConfig {
    pub default_strategy: FusionStrategy,
    pub vector_weight: f64,     // default 0.5
    pub graph_weight: f64,      // default 0.5
    pub rrf_k: u32,             // default 60
    pub max_results: usize,     // default 20
    pub timeout_ms: u64,        // default 3000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FusionStrategy {
    VectorFirst,
    GraphFirst,
    ReciprocalRankFusion,
}
```

### RRF Algorithm

```
RRF_score(d) = sum over all ranking lists R:
    1 / (k + rank_R(d))
```

- `k = 60` (configurable): smoothing constant, prevents top-ranked items from dominating
- Items not present in a ranking list receive rank = infinity (score contribution = 0)

### Parallel Execution

Vector search and graph traversal execute concurrently via `tokio::join!`. Individual timeout per source (default 3s). If one source fails/times out, results from the other source are still returned with degraded provenance annotation.

### Result Schema

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub entry: MemoryEntry,
    pub score: f64,
    pub provenance: SearchProvenance,
    pub vector_rank: Option<u32>,
    pub graph_rank: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchProvenance {
    VectorOnly,
    GraphOnly,
    Both,
}
```

### REST API

```
POST /api/v1/memory/search/hybrid

Request:
{
  "query": "how to deploy service to k8s",
  "strategy": "rrf",           // optional, default from config
  "vector_weight": 0.6,        // optional override
  "graph_weight": 0.4,         // optional override
  "limit": 20,                 // optional
  "filters": { ... }           // standard MemoryFilters
}

Response:
{
  "results": [HybridSearchResult],
  "metadata": {
    "strategy_used": "rrf",
    "vector_count": 15,
    "graph_count": 8,
    "fused_count": 20,
    "vector_latency_ms": 45,
    "graph_latency_ms": 120
  }
}
```

---

## Key Data Flows

### Store Procedural Memory
```
Client → POST /api/v1/memory/procedural
  → ProceduralMemoryLayer.store()
    → Validate ProceduralEntry schema
    → Generate embedding from description + steps
    → Store in PostgreSQL (metadata + JSON content)
    → Upsert vector in Qdrant
    → Return MemoryId
```

### Hybrid Search
```
Client → POST /api/v1/memory/search/hybrid
  → HybridSearchService.search()
    → tokio::join!(
        vector_search.search_by_vector(),
        graph_memory.traverse()
      )
    → Apply FusionStrategy (RRF/VectorFirst/GraphFirst)
    → Annotate provenance
    → Return fused results
```

### Provider Delegation
```
Client → POST /api/v1/memory/store
  → MemoryKernel.store()
    → Check active provider config
    → If builtin: route to appropriate MemoryLayer
    → If external: route to MemoryProvider (Mem0/Zep)
      → HTTP request with timeout + circuit breaker
    → Return MemoryId
```

---

## Technology Choices

| Choice | Rationale |
|--------|-----------|
| `async_trait` for MemoryProvider | External HTTP calls need object safety (dyn dispatch) |
| RPITIT for MemoryLayer | Internal layers are monomorphized, no dyn dispatch needed |
| `reqwest` for HTTP clients | Proven async HTTP client, connection pooling |
| `serde_json::Value` for procedural storage | Avoids MemoryContent enum breaking change |
| `tokio::join!` for parallel search | Simple, no extra runtime; degrades gracefully |
| TOML config for provider selection | Consistent with existing project config patterns |

---

## Risk & Constraints

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Mem0/Zep API version drift | Provider calls fail | Version-pin API endpoints, health check at startup |
| LayerType::Procedural serde compat | Old clients send unknown variant | `#[serde(rename_all = "lowercase")]` already in place; old clients ignore unknown |
| Hybrid search timeout cascade | Slow response | Per-source timeout + partial results with provenance |
| Circuit breaker false positive | Provider unnecessarily disabled | Half-open state with single probe request |
| Embedding model unavailable | Procedural search degrades | Fallback to keyword/tag search when embedding fails |

---

## Non-Breaking Change Confirmation

- `MemoryLayer` trait: **unchanged** (no timeout parameter added to trait signature)
- `MemoryContent` enum: **unchanged** (Procedural uses existing `Json` variant)
- `LayerType` enum: **additive only** (new `Procedural` variant, serde-compatible)
- `MemoryProvider`: **new trait**, no existing code affected
- `HybridSearchService`: **new service**, no existing search logic modified
