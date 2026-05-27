# Adaptive Memory System - Architecture Deep Dive

This document provides a comprehensive technical explanation of the Adaptive Memory Management System's design decisions, internal architecture, and implementation rationale. It is intended for engineers who need to understand, extend, or maintain this codebase.

---

## Table of Contents

1. [System Design Philosophy](#1-system-design-philosophy)
2. [Core Data Structures](#2-core-data-structures)
3. [Service Layer Architecture](#3-service-layer-architecture)
4. [Storage Architecture](#4-storage-architecture)
5. [Multi-Tenant Isolation](#5-multi-tenant-isolation)
6. [Distributed Architecture](#6-distributed-architecture-overview)
7. [Security Model](#7-security-model)
8. [Performance Characteristics](#8-performance-characteristics)
9. [Extensibility Points](#9-extensibility-points)

---

## 1. System Design Philosophy

### 1.1 Why Adaptive Memory?

Traditional AI agent systems use a fixed memory configuration regardless of task characteristics. This leads to inefficiency: simple queries are over-provisioned with heavy long-term memory and knowledge graph processing, while complex reasoning tasks are under-provisioned and lack necessary context.

The adaptive memory system solves this by **observing task characteristics at runtime** and **dynamically selecting the optimal memory configuration**. The system continuously monitors resource costs and performance gains, adjusting weights to maximize cost-benefit ratio.

The key insight is that **memory is not binary** (on/off) but exists on a spectrum with weighted contributions from multiple memory layers. A task with high complexity but low multi-modality should weight long-term memory heavily but skip multimodal processing. The system learns this through the observe-decide-act loop.

### 1.2 The Observe-Decide-Act Loop

The system implements a classic **agent control loop** (similar to the OODA loop in control theory):

```
┌─────────────────────────────────────────────────────────────┐
│                      OBSERVE                                 │
│  TaskCharacteristicAnalyzer observes task context            │
│  - Complexity assessment                                     │
│  - Modality detection                                       │
│  - Temporal scope analysis                                   │
│  - Reasoning depth evaluation                               │
│  - Context dependency measurement                            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                       DECIDE                                │
│  PerformancePredictionModel evaluates memory configs        │
│  - Synergy factor (multiple layers amplify each other)       │
│  - Decay factor (marginal benefit diminishes)                │
│  - Resource cost estimation                                  │
│  - Cost-benefit ratio calculation                            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                        ACT                                  │
│  AdaptiveMemoryScheduler selects and applies config          │
│  - DynamicWeightAdjuster applies strategy chain              │
│  - Resource constraints enforced                            │
│  - Decision trace recorded for audit                        │
└─────────────────────────────────────────────────────────────┘
```

**Why this loop matters:**
- **Observe**: Tasks arrive with implicit characteristics. The analyzer extracts explicit features (complexity score, modality count, reasoning depth) that drive later decisions.
- **Decide**: The predictor models how different memory configurations perform given task characteristics. This is not hardcoded - the model uses empirical baselines for each memory type's efficiency and coherence gains.
- **Act**: The scheduler combines predictions with real-time resource monitoring to select weights that maximize predicted performance while respecting constraints.

The loop is **traced** - every decision produces a `DecisionTrace` containing the full reasoning chain, enabling debugging and audit.

### 1.3 Multi-Layer Memory Architecture (STM/LTM/KG/MM)

The system manages four distinct memory layers, each with different tradeoffs:

| Layer | Purpose | Latency | Capacity | Cost |
|-------|---------|---------|----------|------|
| **STM** (Short-Term Memory) | Active conversation context | <1ms | 4,096 tokens | 0.2 units |
| **LTM** (Long-Term Memory) | Persistent knowledge | ~50ms | 10,000 entries | 0.4 units |
| **KG** (Knowledge Graph) | Structured relationships | ~30ms | 1,000 entities | 0.6 units |
| **MM** (Multimodal Memory) | Images, audio, video | ~100ms | 1,000 entries | 0.8 units |

**Design rationale for separate layers:**

1. **STM is always primary**: Working context is non-negotiable - without it, coherence breaks. STM weight is always 1.0.

2. **LTM provides coherence at cost**: The predictor's baseline shows LTM contributes 0.37 efficiency gain and 1.38 coherence gain (per unit of cost). This is the workhorse for complex tasks requiring historical context.

3. **KG enables deep reasoning**: When reasoning depth exceeds 0.7, knowledge graph connections provide structured context that improves logical deduction. The marginal contribution is 0.43 efficiency / 1.60 coherence but at higher cost (0.6).

4. **MM is optional by design**: Multimodal processing is expensive (0.8 cost) and only activates when `modality_count > 1`. This prevents waste on text-only tasks.

**Why weighted contributions instead of binary decisions:**

A naive system would say "use LTM or don't." The adaptive system says "use LTM at weight 0.8, KG at weight 0.5." This fine-grained control allows the optimizer to find configurations like "use LTM partially because full LTM would exceed the memory quota." The weights represent how much of each layer's output feeds into the final context.

---

## 2. Core Data Structures

### 2.1 MemoryEntry, MemoryContent, MemoryMetadata

These structures represent the fundamental unit of storage:

```rust
// In memory.rs
pub enum MemoryType { Stm, Ltm, Kg, Mm }

pub struct MemoryWeights {
    pub stm: f64,   // Always 1.0 for primary
    pub ltm: f64,   // 0.0-1.0, marginal contribution
    pub kg: f64,    // 0.0-1.0, marginal contribution
    pub mm: f64,    // 0.0-1.0, marginal contribution
}

pub struct MemoryConfig {
    pub primary_memory: MemoryType,
    pub secondary_memory: Vec<MemoryType>,
    pub memory_weights: MemoryWeights,
    pub reasoning_depth: String,  // "shallow", "medium", "deep"
    pub enable_multimodal: bool,
}
```

**Design decision - why weights at the config level:**

The weights are decoupled from the layer enable/disable state. A layer can be "enabled" (in `secondary_memory`) but with weight 0.1 - effectively minimally contributing. This allows gradual degradation under resource pressure rather than binary on/off.

### 2.2 TaskContext, TaskCharacteristics

```rust
// In task.rs
pub struct TaskContext {
    pub task_id: String,
    pub task_type: TaskType,  // Conversation, Task, Query
    pub complexity: f64,     // 0.0-1.0
    pub modality_requirements: Vec<Modality>,  // Text, Image, Audio, Video
    pub temporal_scope: TemporalScope,  // Short, Medium, Long
    pub reasoning_depth: ReasoningDepth,  // Shallow, Medium, Deep
    pub context_dependency: f64,  // How much this task depends on history
    pub user_id: String,
    pub agent_id: String,
}

pub struct TaskCharacteristics {
    pub complexity: f64,           // Derived from content analysis
    pub modality_count: usize,     // Count of required modalities
    pub temporal_scope: String,    // Time horizon of task relevance
    pub reasoning_depth: f64,      // 0.0-1.0, derived from keywords + complexity
    pub context_dependency: f64,    // 0.0-1.0, how much history is needed
}
```

**Why derived characteristics instead of raw input:**

`TaskContext` is the input from the caller. `TaskCharacteristics` is the system's internal feature representation. The analyzer transforms raw inputs into normalized features (0.0-1.0 range) that the predictor model was calibrated on. This separation allows:
- Changing the analyzer heuristics without retraining the predictor
- A/B testing different analysis algorithms
- Caching characteristics independently of raw context

### 2.3 MemoryWeights, WeightDelta

```rust
// In weight_strategy.rs
pub struct WeightStrategyMetrics<'a> {
    pub task_profile: &'a TaskCharacteristics,
    pub cost_benefit_ratio: f64,  // Performance per unit cost
    pub base_weights: &'a MemoryWeights,
}

pub struct WeightDelta {
    pub weights: MemoryWeights,
    pub reasons: AdjustmentReasons,  // Human-readable explanation
}
```

**Why separate WeightDelta from direct weight assignment:**

The `WeightDelta` pattern enables **strategy composition**. Each strategy in the chain produces a delta that modifies the base weights. This allows:
- Chaining strategies: `MarginalBenefitStrategy` sets initial weights, then `LinearDecayStrategy` scales them down if cost-benefit is poor
- Audit trails: Each strategy contributes reasons explaining its adjustments
- Testability: Each strategy can be unit tested in isolation

### 2.4 DecisionTrace, EvidenceGraph

```rust
// In scheduler.rs
pub struct DecisionTrace {
    pub task_id: String,
    pub analyzer: AnalyzerTraceStep,
    pub resource_status: CurrentResourceStatus,
    pub initial_memory_config: MemoryConfig,
    pub predictor: PredictorTraceStep,
    pub cost_benefit_ratio: f64,
    pub weight_adjustment: WeightAdjustmentTraceStep,
    pub final_result: MemorySelectionResult,
    pub memory_contributions: Vec<MemoryTypeContribution>,
}
```

**Why complete tracing:**

The decision trace is critical for:
1. **Debugging**: If a config performs poorly, engineers can replay the exact decision logic
2. **Audit**: Enterprise customers need to explain why certain memory configurations were selected
3. **Learning**: The trace reveals whether the predictor's predictions matched actual outcomes

The evidence graph (in `evidence.rs`) extends this with cryptographic verification of the workflow that produced the decision, enabling tamper-evident audit logs.

---

## 3. Service Layer Architecture

### 3.1 How Analyzer, Predictor, Monitor, Scheduler Interact

```
┌──────────────────────────────────────────────────────────────────────┐
│                     AdaptiveMemoryScheduler                          │
│                                                                      │
│  ┌───────────────┐    ┌─────────────────┐    ┌───────────────────┐ │
│  │ TaskCharacte- │───▶│ Performance     │───▶│ ResourceMonitor   │ │
│  │ risticAnalyzer│    │ PredictionModel │    │                   │ │
│  └───────────────┘    └─────────────────┘    └───────────────────┘ │
│          │                      │                       │           │
│          │                      │                       │           │
│          │    ┌─────────────────┴───────┐              │           │
│          │    │                         │              │           │
│          ▼    ▼                         │              ▼           │
│  ┌─────────────────────┐                │    ┌─────────────────┐  │
│  │ MemoryStrategy      │                │    │ CostBenefit     │  │
│  │ (primary +          │                │    │ RatioCalc        │  │
│  │  secondary)         │                │    │                 │  │
│  └──────────┬──────────┘                │    └────────┬────────┘  │
│             │                           │             │           │
│             │    ┌──────────────────────┴────────────┘           │
│             │    │                                                 │
│             ▼    ▼                                                 │
│  ┌─────────────────────────┐                                      │
│  │ DynamicWeightAdjuster    │                                      │
│  │ (Strategy Chain)         │                                      │
│  └──────────┬──────────────┘                                      │
│             │                                                      │
│             ▼                                                      │
│  ┌─────────────────────────┐                                      │
│  │ MemorySelectionResult    │                                      │
│  │ + DecisionTrace          │                                      │
│  └─────────────────────────┘                                      │
└──────────────────────────────────────────────────────────────────────┘
```

**Flow explanation:**

1. **Analyzer.analyze_task_characteristics()** takes `TaskContextInput` and produces:
   - `TaskCharacteristics`: normalized features (complexity, modality_count, etc.)
   - `MemoryStrategy`: which layers should be enabled (before weight optimization)
   - `confidence_score`: how confident the analyzer is in its assessment

2. **Monitor.get_current_status()** samples real-time system resources:
   - CPU usage percentage
   - Memory usage (used/total in MB)
   - Storage usage percentage
   - Response time (current latency)

3. **Predictor.predict_memory_performance()** takes the memory config and returns:
   - `PerformancePrediction`: efficiency_gain, coherence_gain, resource_cost, cost_benefit_ratio
   - `synergy_factor`: boost when multiple layers are active (1.0 + 0.1 per additional active layer)
   - `decay_factor`: penalty for layer transitions (STM→LTM: 0.495, LTM→KG: 0.470, KG→MM: 0.071)
   - `performance_breakdown`: per-layer contribution to efficiency

4. **WeightAdjuster.adjust_memory_weights()** runs the strategy chain:
   - Each strategy receives current weights + task profile + cost_benefit_ratio
   - Strategies produce deltas that accumulate
   - Final weights respect resource constraints (iterative reduction if exceeded)

**Why this pipeline over a single-pass model:**

A single-pass approach would require training a model that takes all inputs and directly outputs weights. This has problems:
- Requires large training dataset of (task, config, outcome) triplets
- Not interpretable - can't explain why a particular config was chosen
- Inflexible - changing one heuristic (e.g., "KG activates at depth > 0.7") requires retraining

The rule-based pipeline with learned baselines is:
- Interpretable: Every decision has a documented reason
- Debugable: Can isolate which stage produced unexpected results
- Extensible: Can add new strategies without removing old ones

### 3.2 The Weight Adjustment Pipeline

The `DynamicWeightAdjuster` maintains a **chain of responsibility**:

```rust
pub struct DynamicWeightAdjuster {
    strategies: Vec<Box<dyn WeightStrategy>>,
}

impl WeightStrategy for MarginalBenefitStrategy {
    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta {
        // Rule: High complexity → enable LTM
        if metrics.task_profile.complexity > 0.5 {
            weights.ltm = (metrics.task_profile.complexity * 0.8).min(0.8);
        }
        // Rule: Multi-modal → enable MM
        if metrics.task_profile.modality_count > 1 {
            weights.mm = (metrics.task_profile.modality_count as f64 * 0.3).min(0.6);
        }
        // Rule: Deep reasoning → enable KG
        if metrics.task_profile.reasoning_depth > 0.7 {
            weights.kg = (metrics.task_profile.reasoning_depth * 0.7).min(0.7);
        }
        // STM always 1.0
        weights.stm = 1.0;
        WeightDelta { weights, reasons }
    }
}
```

Then `LinearDecayStrategy` scales down all secondary memories if `cost_benefit_ratio < 1.0` (meaning the cost exceeds the expected performance gain).

Finally `SynergyAwareStrategy` provides a small boost (1.0 + 0.05 per active layer) when multiple secondary layers are simultaneously active, recognizing that layered memory access creates compound benefits.

**Why pluggable strategies:**

This design enables:
- **A/B testing**: Compare MarginalBenefit vs. a learned strategy
- **Domain-specific tuning**: A medical domain might have different complexity thresholds
- **Progressive deployment**: Add new strategies alongside existing ones
- **Hotfixing**: If a strategy has a bug, remove it from the chain without redeploying others

### 3.3 Why Strategies Are Pluggable

The `WeightStrategy` trait is defined as:

```rust
pub trait WeightStrategy: Send + Sync {
    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta;
    fn name(&self) -> &'static str;
}
```

The `Send + Sync` bounds ensure thread safety - strategies can be shared across async tasks. The `name()` method enables experiment tracking: when reviewing weight history, you can see which strategy chain produced which result.

**Adding a new strategy:**

```rust
pub struct CostSensitiveStrategy;

impl WeightStrategy for CostSensitiveStrategy {
    fn name(&self) -> &'static str { "CostSensitive" }

    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta {
        // If resource cost > 0.7, reduce all secondary weights by 50%
        let base_cost = metrics.base_weights.stm * 0.2 +
                        metrics.base_weights.ltm * 0.4 +
                        metrics.base_weights.kg * 0.6 +
                        metrics.base_weights.mm * 0.8;
        if base_cost > 0.7 {
            let mut weights = metrics.base_weights.clone();
            weights.ltm *= 0.5;
            weights.kg *= 0.5;
            weights.mm *= 0.5;
            WeightDelta { weights, reasons: /* ... */ }
        } else {
            WeightDelta { weights: metrics.base_weights.clone(), reasons: /* ... */ }
        }
    }
}

// Register in scheduler initialization:
strategies: vec![
    Box::new(MarginalBenefitStrategy),
    Box::new(LinearDecayStrategy),
    Box::new(CostSensitiveStrategy),  // New strategy
]
```

---

## 4. Storage Architecture

### 4.1 PostgreSQL for Relational Data

The system uses PostgreSQL as the primary relational store for:

- **Memory configurations**: `memory_configurations` table stores the final selected config per task
- **Session management**: `context_sessions` and `session_messages` for STM
- **Knowledge entries**: `knowledge_entries` for LTM metadata (not vectors)
- **Performance metrics**: `performance_metrics` for historical tracking
- **Weight history**: `weight_history` for auditing strategy decisions

**Why PostgreSQL over SQLite for production:**

- **Concurrency**: SQLite's write locking becomes a bottleneck under concurrent load. PostgreSQL's MVCC allows true concurrent reads with writers blocked only when modifying the same rows.
- **Connection pooling**: PgBouncer or built-in pool management handles thousands of connections efficiently
- **Replication**: Streaming replication enables read replicas for query scaling
- **JSON support**: PostgreSQL's JSONB columns store flexible metadata without schema migrations

**Repository pattern implementation:**

Each domain has a repository struct with static async methods:

```rust
pub struct MemoryConfigRepository;

impl MemoryConfigRepository {
    pub async fn create(user_id: &str, agent_id: &str, ...) -> Result<String, AppError> {
        let config_id = Ulid::new().to_string();
        sqlx::query!(
            "INSERT INTO memory_configurations (config_id, user_id, ...) VALUES ($1, $2, ...)"
        )
        .bind(&config_id)
        .bind(user_id)
        // ...
        .execute(pool())
        .await?;
        Ok(config_id)
    }
}
```

**Why static methods over trait objects:**

The repository uses static methods rather than a trait because:
- The database is the only implementation (no mocking in production)
- Static methods are simpler and compile faster than dynamic dispatch
- Tests can use a test database or SQLite in-memory

### 4.2 Qdrant for Vector/Embedding Storage

Long-term memory entries are stored as vectors in Qdrant:

```
┌─────────────────────────────────────────────────────────────┐
│                      Qdrant Collection                       │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Vector (384/768 dim)  │  Payload (JSON metadata)  │   │
│  ├─────────────────────────┼───────────────────────────┤   │
│  │  [0.123, -0.456, ...]  │  {entry_id, summary,     │   │
│  │                         │   entities, relations,    │   │
│  │                         │   tenant_id, source_type}  │   │
│  └─────────────────────────┴───────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Why a dedicated vector database:**

- **ANN indexing**: Qdrant uses HNSW (Hierarchical Navigable Small World) for approximate nearest neighbor search at scale. PostgreSQL's `pgvector` is adequate for <100k vectors but degrades beyond that.
- **Filtering**: Qdrant's payload filtering works on the HNSW index directly, not post-filtered.
- **Distributed**: Qdrant supports replication and can scale horizontally.
- **Dimension management**: Qdrant validates vector dimensions on write, preventing dimension mismatch errors at query time.

**Vector guard (Issue #59):**

Before any write or search, the system validates vector dimensions:

```rust
// In vector_guard.rs
pub fn validate_write(vectors: &[Vec<f32>]) -> Result<()> {
    let expected = get_expected_dimension()?;
    for vector in vectors {
        if vector.len() != expected {
            return Err(AppError::BadRequest(format!(
                "Vector dimension mismatch: expected {}, got {}",
                expected,
                vector.len()
            )));
        }
    }
    Ok(())
}
```

This prevents the "dimension changed silently" failure mode where a model swap invalidates all stored vectors.

### 4.3 Redis for STM Cache (v1.1 Feature)

When the `redis-stm` feature is enabled, the system can delegate STM operations to Redis:

```rust
#[cfg(feature = "redis-stm")]
use crate::db::adapters::redis_stm::RedisStmAdapter;

impl STMRepository {
    pub async fn create_session(...) -> Result<String, AppError> {
        #[cfg(feature = "redis-stm")]
        if RedisStmAdapter::is_available() {
            return RedisStmAdapter::create_session(...).await;
        }
        // Fall back to PostgreSQL
    }
}
```

**Why Redis for STM:**

- **Sub-millisecond latency**: Redis provides <1ms reads vs PostgreSQL's ~5-10ms
- **Automatic expiry**: Redis TTL naturally handles session expiration without cleanup jobs
- **Pub/sub**: Enables real-time session updates for collaborative features

**Design decision - feature flag over runtime selection:**

Using `#[cfg(feature = "redis-stm")]` means Redis is a compile-time choice. This eliminates runtime branching complexity and ensures the PostgreSQL path is always tested. Runtime selection would mean two code paths, one likely untested.

### 4.4 Neo4j for Knowledge Graphs (Optional)

Neo4j stores the structured knowledge graph relationships:

```
(entity:Entity)-[RELATION]->(entity:Entity)
     │                  │
     └── properties ────┘
         name, type, confidence, tenant_id
```

**Why separate from LTM:**

The knowledge graph is not just a metadata index - it represents entity relationships that enable:
- **Graph traversal queries**: "Find all entities related to X that connect to Y"
- **Inference**: "If A relates to B and B relates to C, find A-C paths"
- **Confidence propagation**: Relationship weights affect entity importance

Neo4j's Cypher query language is optimized for these traversals in ways PostgreSQL cannot match.

**When not to use Neo4j:**

For simpler deployments or when graph inference is not needed, the system degrades gracefully:
- `KGRepository` operations fall back to PostgreSQL equivalents
- `TripleHybridSearch` still works but treats KG results as entity-matched entries rather than graph traversals

### 4.5 The Repository Pattern with Adapters

The storage layer uses a repository pattern that allows pluggable backends:

```
┌──────────────────────────────────────────────────────────────┐
│                     Repository Layer                         │
├──────────────────────────────────────────────────────────────┤
│  STMRepository                                              │
│    ├── PostgreSQL implementation (default)                   │
│    └── Redis adapter (redis-stm feature)                     │
├──────────────────────────────────────────────────────────────┤
│  LTMRepository                                              │
│    ├── PostgreSQL for metadata                              │
│    └── Qdrant for vectors                                   │
├──────────────────────────────────────────────────────────────┤
│  KGRepository                                               │
│    ├── PostgreSQL (fallback)                                │
│    └── Neo4j (when configured)                              │
└──────────────────────────────────────────────────────────────┘
```

**Why adapters are compile-time feature flags:**

Each adapter is a separate implementation path controlled by `#[cfg(...)]`. This ensures:
- Only one implementation is compiled
- No runtime overhead from enum matching
- Easier testing (can compile without Redis feature and always use PostgreSQL)

---

## 5. Multi-Tenant Isolation

### 5.1 How tenant_id Scoping Works

Every database query that accesses tenant data includes a tenant filter:

```rust
// In STMRepository::get_session
let prefix = tenant_id.prefix();  // e.g., "t:user123"
let belongs_to_tenant = s.user_id.starts_with(&prefix)
    || s.user_id == tenant_id.as_str();  // MVP: each user is their own tenant
```

**Why prefix matching:**

The `TenantId.prefix()` method produces `t:{tenant_id}`. This creates a namespace:
- Tenant `acme` has users like `t:acme:alice`, `t:acme:bob`
- Tenant `globex` has users like `t:globex:charlie`

Query filtering uses `LIKE 't:acme:%'` to select all users belonging to the tenant.

### 5.2 TenantContext Propagation Through the Stack

```
┌─────────────────────────────────────────────────────────────┐
│                     HTTP Request                           │
│  Cookie: jwt_token=eyJ...                                  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  auth_middleware (hoops/jwt.rs)                            │
│  - Validates JWT signature                                  │
│  - Extracts uid claim                                       │
│  - Creates RequestTenantContext                             │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  RequestTenantContext in Extensions                         │
│  - tenant_id: TenantId                                      │
│  - user_id: String (from JWT)                              │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  Handler receives RequestTenantContext                      │
│  - Extracts from request.extensions()                       │
│  - Passes to repository methods                             │
└─────────────────────────────────────────────────────────────┘
```

**Why not middleware-scoped state:**

Axum's `RequestTenantContext` extractor handles extraction at the handler level:

```rust
async fn handler(tenant: RequestTenantContext) -> impl IntoResponse {
    // tenant.tenant_id is available here
}
```

This is cleaner than passing tenant_id through every function parameter.

### 5.3 Quota Enforcement

The `QuotaEnforcer` checks limits before write operations:

```rust
pub struct QuotaEnforcer;

impl QuotaEnforcer {
    pub async fn check_ltm_quota(tenant_id: &str) -> Result<(), AppError> {
        let cfg = get_tenant(tenant_id)?;  // From tenant registry
        let max = match cfg.max_ltm_entries {
            Some(m) => m,
            None => return Ok(()),  // No limit
        };
        let current = LTMRepository::count_entries(pool(), tenant_id)?;
        if current >= max {
            return Err(AppError::BadRequest(format!(
                "Tenant '{}' has reached LTM quota ({})",
                tenant_id, max
            )));
        }
        Ok(())
    }
}
```

**Why quota enforcement at the service layer:**

Enforcement is in `QuotaEnforcer` (service layer) rather than the repository layer because:
- Quotas may span multiple repositories (e.g., combined LTM + STM storage limit)
- Need access to tenant configuration registry
- May involve complex policy (e.g., soft vs. hard limits)

---

## 6. Distributed Architecture (Overview)

### 6.1 When to Scale Horizontally

The system is designed for **vertical scaling first**:

- Single-node deployment handles ~1000 concurrent sessions
- 16GB RAM supports ~10,000 LTM entries with Qdrant
- SQLite for single-node, PostgreSQL for multi-user

**Horizontal scaling is needed when:**

- Session count exceeds ~5,000 concurrent
- LTM entry count exceeds ~100,000 (Qdrant performance degrades on single node)
- Cross-datacenter replication is required for latency

### 6.2 Replication and Consensus Basics

The distributed node architecture (`distributed/node.rs`) implements a basic Raft-like model:

```rust
pub enum NodeRole {
    Leader,      // Primary - handles all writes
    Follower,    // Read replicas
    Candidate,   // Leader election in progress
    Learner,     // Async replica, no voting
}

pub struct MemoryNode {
    id: NodeId,
    info: RwLock<NodeInfo>,
    peers: RwLock<HashMap<NodeId, NodeInfo>>,
}
```

**Why not multi-Paxos from day one:**

Full distributed consensus adds significant complexity:
- Network partition handling
- Leader election
- Log replication
- Snapshot compaction

The MVP uses a single leader with read replicas. Full consensus is deferred until:
- Benchmarking proves single-node insufficient
- Cross-datacenter deployment is required
- Budget exists for distributed systems engineering

### 6.3 Sharding Strategy

When sharding is needed, the system uses **tenant-based sharding**:

```
Shard 0: Tenants A-D
Shard 1: Tenants E-H
Shard 2: Tenants I-L
...
```

**Why tenant-based over hash-based:**

- **Tenant isolation is natural**: Queries never cross shards within a tenant
- **Cross-tenant analytics**: Sharded views for super-admin queries
- **Quota management**: Shard assignment = resource quota
- **Failure isolation**: One shard's load doesn't affect others

**Routing:**

```rust
fn select_shard(tenant_id: &TenantId) -> ShardId {
    let tenant_num = hash_tenant_id(tenant_id) % NUM_SHARDS;
    ShardId(tenant_num)
}
```

---

## 7. Security Model

### 7.1 MCP Sandbox Isolation

MCP (Model Context Protocol) components must be signed and verified before loading (SEC-01):

```rust
// In mcp/signing.rs
pub struct ComponentSignature {
    pub component_id: String,
    pub sha256_hash: String,    // SHA-256 of component artifact
    pub issuer: String,         // e.g., "adaptive-memory-system"
    pub version: String,
    pub timestamp: i64,
    pub signature: String,       // HMAC-SHA256
}
```

**Why HMAC-SHA256 over asymmetric signatures:**

HMAC is simpler and faster. The threat model is:
- Component artifact tampering (detected by SHA-256 hash)
- Unauthorized component loading (detected by HMAC verification)

Asymmetric signatures (Ed25519) would be necessary if we needed non-repudiation (proving a third party signed the component). HMAC with a trusted key bundle is sufficient for integrity verification.

### 7.2 Signing Verification Flow

```
┌─────────────────────────────────────────────────────────────┐
│  Component Loading                                          │
│                                                              │
│  1. Load component artifact (binary/tool definition)        │
│  2. Extract ComponentSignature metadata                     │
│  3. Compute SHA-256 of artifact                             │
│  4. Verify SHA-256 matches signature.sha256_hash             │
│  5. Lookup issuer key in TrustedKeyBundle                   │
│  6. Compute HMAC-SHA256(artifact_hash + issuer + version)  │
│  7. Constant-time compare with signature.signature          │
│  8. If mismatch → reject load                               │
└─────────────────────────────────────────────────────────────┘
```

**Why constant-time comparison:**

Timing attacks can leak information about the expected signature. `constant_time_eq` ensures comparison takes the same number of cycles regardless of where the mismatch occurs.

### 7.3 Input Validation Layers

The system validates at multiple boundaries:

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: HTTP Request Validation                          │
│  - axum's Json extractor validates JSON schema              │
│  - Query/Path extractors validate types                    │
│  - Max request body size enforced                          │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: Business Logic Validation                        │
│  - TaskContextInput modality validation                     │
│  - ResourceConstraints minimum thresholds                   │
│  - TenantConfig quotas checked                              │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: Database Constraints                             │
│  - SQL CHECK constraints (e.g., weight 0.0-1.0)           │
│  - Foreign key relationships                                │
│  - NOT NULL columns                                         │
└─────────────────────────────────────────────────────────────┘
```

**Why three layers:**

Each layer catches different failure modes:
- Layer 1: malformed requests (client bugs, network corruption)
- Layer 2: semantically invalid requests (business rule violations)
- Layer 3: prevents data corruption from any source

### 7.4 JWT Authentication Flow

```
┌─────────────────────────────────────────────────────────────┐
│  Login Request                                              │
│  POST /api/v1/auth/login { username, password }            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  Token Generation                                           │
│  - Create JwtClaims { uid, exp }                           │
│  - Encode with HS256 using server secret                   │
│  - Set httpOnly cookie: jwt_token={token}                  │
│  - Set expiry from config (default: 24h)                  │
└─────────────────────────────────────────────────────────────┘
```

**Cookie over Authorization header:**

The system prefers `httpOnly` cookies for browser clients:
- **XSS protection**: JavaScript cannot access the cookie
- **CSRF protection**: SameSite=Strict cookie prevents cross-site requests

API clients (Brave, curl) can use `Authorization: Bearer {token}` as a fallback.

---

## 8. Performance Characteristics

### 8.1 Expected Latencies by Operation Type

| Operation | P50 | P95 | P99 | Notes |
|-----------|-----|-----|-----|-------|
| STM Read | <1ms | 3ms | 5ms | In-process cache or Redis |
| STM Write | 2ms | 10ms | 20ms | PostgreSQL write + session update |
| LTM Vector Search | 15ms | 50ms | 100ms | Qdrant ANN query |
| LTM Metadata Read | 5ms | 20ms | 40ms | PostgreSQL lookup |
| KG Traversal | 20ms | 80ms | 150ms | Neo4j Cypher or PostgreSQL fallback |
| Full Memory Selection | 50ms | 150ms | 300ms | Pipeline: analyze + predict + adjust |
| Triple Hybrid Search | 80ms | 200ms | 400ms | Parallel vector + keyword + KG |

**Why P99 matters:**

P50 is "happy path." P99 is where outliers live - the requests that timeout, retry, or cascade into failures. Systems must be designed so P99 latency is still within SLAs.

### 8.2 Connection Pooling Strategy

PostgreSQL connection pool settings:

```rust
PgPoolOptions::new()
    .max_connections(10)           // Cap total connections
    .min_idle(2)                   // Keep 2 warm connections
    .acquire_timeout(Duration::from_secs(30))
    .idle_timeout(Some(Duration::from_secs(600)))  // Reclaim idle after 10m
    .max_lifetime(Some(Duration::from_secs(1800))) // Rotate connections after 30m
```

**Why these numbers:**

- `max_connections=10`: PostgreSQL defaults to 100 connections, but each connection costs ~5MB memory. 10 connections = 50MB for connections alone.
- `acquire_timeout=30s`: Prevents indefinite waiting when pool is exhausted
- `idle_timeout=600s`: Frees connections that haven't been used, reducing resource footprint

**SQLite special handling:**

SQLite uses serialized writes through a single connection to avoid "database is locked" errors:

```rust
.after_connect(|conn| {
    sqlx::query("PRAGMA journal_mode=WAL")
    // Enable WAL for concurrent reads
    sqlx::query("PRAGMA busy_timeout=5000")
    // Wait up to 5s for locks
})
```

### 8.3 Caching Strategy (Moka)

The system uses Moka for in-memory caching:

- **LLM responses**: Cache summarization results keyed by content hash
- **Embedding lookups**: Cache generated embeddings for repeated queries
- **Tenant configurations**: Cache loaded tenant configs

**Why Moka over other caches:**

- **TTL-based expiration**: Automatic cleanup without manual invalidation
- **Async support**: `get_with_if()` prevents thundering herd
- **Thread-safe**: `Sync` bounds allow sharing across tasks
- **Memory-bounded**: `max_capacity` prevents unbounded growth

**Write-through vs. write-around:**

The cache uses **write-around**: writes go to the database first, then the cache is updated on read. This prevents stale writes but means first read after a write may miss cache.

---

## 9. Extensibility Points

### 9.1 Adding New WeightStrategies

The strategy pattern allows adding new strategies without modifying existing code:

1. Implement `WeightStrategy` trait
2. Add to `DynamicWeightAdjuster::with_strategies()` chain
3. Strategy receives current weights, task profile, and cost-benefit ratio

Example: A domain-specific strategy for medical tasks:

```rust
pub struct MedicalDomainStrategy;

impl WeightStrategy for MedicalDomainStrategy {
    fn name(&self) -> &'static str { "MedicalDomain" }

    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta {
        let mut weights = metrics.base_weights.clone();

        // Medical tasks need higher KG weight for drug interactions
        if let Some(domain) = detect_medical_domain(&metrics.task_profile) {
            if domain == "pharmacology" {
                weights.kg = (weights.kg * 1.3).min(1.0);
            }
        }

        WeightDelta { weights, reasons: /* ... */ }
    }
}
```

### 9.2 Adding New MemoryAgents

The `MemoryAgent` trait abstracts the observe-decide-act pattern:

```rust
pub trait MemoryAgent {
    type Context;
    type Observation;
    type Decision;
    type Action;

    fn observe(&self, context: &Self::Context) -> impl Future<Output = Self::Observation> + Send;
    fn decide(&self, observation: &Self::Observation) -> impl Future<Output = Self::Decision> + Send;
    fn act(&self, decision: &Self::Decision) -> impl Future<Output = Result<Self::Action, AppError>> + Send;
}
```

**Why async futures in the trait:**

This allows implementations to make blocking calls (database, network) without blocking the executor. The `Send` bound ensures the future can be spawned on another thread if needed.

**Example: LLM-driven agent:**

```rust
pub struct LlmDrivenAnalyzer;

impl MemoryAgent for LlmDrivenAnalyzer {
    type Context = TaskContextInput;
    type Observation = AnalyzerObservation;
    type Decision = AnalyzerDecision;
    type Action = AnalyzerAction;

    async fn observe(&self, context: &Self::Context) -> Self::Observation {
        // Call LLM to analyze task characteristics
        let llm_response = llm_service.analyze(context).await;
        parse_llm_response(llm_response)
    }

    async fn decide(&self, observation: &Self::Observation) -> Self::Decision {
        // LLM suggests memory strategy
        observation.clone()
    }

    async fn act(&self, decision: &Self::Decision) -> Result<Self::Action, AppError> {
        Ok(decision.clone())
    }
}
```

### 9.3 Custom Storage Adapters

The adapter pattern allows swapping storage backends:

```rust
#[cfg(feature = "redis-stm")]
mod redis_stm {
    pub struct RedisStmAdapter;

    impl RedisStmAdapter {
        pub async fn create_session(...) -> Result<String, AppError> {
            // Redis-specific implementation
        }
    }
}
```

**To add a new storage backend:**

1. Create `db/adapters/{backend}.rs`
2. Implement the repository interface
3. Add `#[cfg(feature = "{backend}")]`
4. Add feature flag to `Cargo.toml`

### 9.4 Protocol Extensions

The system supports multiple protocol entry points:

- **HTTP/REST**: Axum routers for browser and API clients
- **WebSocket**: Real-time session updates
- **gRPC**: High-performance internal communication (protocol/grpc.rs)
- **MCP**: Model Context Protocol for tool integration

**Adding a new protocol:**

1. Create protocol handler in `protocol/`
2. Implement `Registry` trait for service discovery
3. Add route registration in main.rs

---

## Summary

The Adaptive Memory System is designed around three core principles:

1. **Observability**: Every decision is traced and auditable
2. **Pluggability**: Every component can be swapped or extended
3. **Graceful Degradation**: The system works with minimal dependencies and scales up as needed

The architecture prioritizes:
- **Interpretability** over black-box optimization
- **Composability** over monolithic design
- **Reliability** over peak performance

These choices make the system suitable for enterprise deployment where audit trails, debuggability, and reliability are as important as raw performance.
