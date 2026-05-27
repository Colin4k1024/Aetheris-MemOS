# Delivery Plan: Memory Architecture Gaps

## Version Target

- **Milestone:** v1.2 — Memory Architecture Compliance
- **Scope:** Procedural Memory Layer + Multi-Backend Provider + GraphRAG Hybrid Search
- **Release Criteria:** All 3 user stories pass acceptance criteria; existing tests remain green; no breaking API changes

---

## Requirement Challenge Session Log

### Challenge Group A: Architecture Layer (tech-lead + architect)

**Core Assumption Challenged:** MemoryBackend trait should be a sub-trait of MemoryLayer.

| Item | Content |
|------|---------|
| Assumption | External backends (Mem0/Zep) can implement the full MemoryLayer interface |
| Challenge | External backends don't support eviction, stats, or layer-level lifecycle control; they operate at a different abstraction level |
| Alternative Path | Create `MemoryProvider` as a parallel trait at Kernel level with smaller interface |
| Conclusion | **Accepted alternative.** MemoryProvider is parallel to MemoryLayer, not nested. Kernel routes based on provider config |
| Blocking Condition | None — design is compatible with existing MemoryLayer trait |

### Challenge Group B: Data Model (tech-lead + backend-engineer)

**Core Assumption Challenged:** ProceduralMemory needs a new MemoryContent variant.

| Item | Content |
|------|---------|
| Assumption | Need `MemoryContent::Procedural(ProceduralEntry)` enum variant |
| Challenge | Adding enum variant breaks serde compatibility for existing clients deserializing MemoryContent |
| Alternative Path | Store as `MemoryContent::Json(Value)` with schema validation in ProceduralMemoryLayer |
| Conclusion | **Accepted alternative.** Json storage with layer-level validation. No MemoryContent enum change |
| Blocking Condition | None |

### Challenge Group C: Search Strategy (tech-lead + backend-engineer)

**Core Assumption Challenged:** Hybrid search requires MemoryLayer trait changes for timeout support.

| Item | Content |
|------|---------|
| Assumption | MemoryLayer.search() needs timeout/cancellation token parameter |
| Challenge | Changing the trait signature is a breaking change affecting all 4 existing layer implementations |
| Alternative Path | Handle timeout at the HybridSearchService orchestration level using tokio::select! per-call |
| Conclusion | **Accepted alternative.** Timeout handled at orchestration layer. MemoryLayer trait unchanged |
| RRF k value | `k=60` as configurable default (standard literature value) |

---

## Design Decisions Summary

| Decision | Chosen | Rationale |
|----------|--------|-----------|
| Provider vs Backend naming | `MemoryProvider` trait | Clearer semantic distinction from Layer |
| Provider position | Parallel to Layer chain | External systems have different lifecycle |
| Procedural storage | Json variant + validation | No breaking serde change |
| Procedural steps | Linear Vec (v1) | User confirmed; DAG deferred |
| Timeout handling | Orchestration-level | No trait breaking change |
| RRF smoothing constant | k=60, configurable | Standard value per literature |
| Provider resilience | Circuit breaker + timeout | Prevents cascade failures |

---

## Work Breakdown

### Slice 1: Kernel Type Extensions (Foundation)

| Item | Detail |
|------|--------|
| **Goal** | Add Procedural variant, ProceduralEntry types, MemoryProvider trait |
| **Owner** | backend-engineer |
| **Dependencies** | None |
| **Deliverables** | Modified `kernel/types.rs`, new `kernel/provider.rs`, new `models/procedural.rs` |
| **Acceptance** | `cargo build` passes; existing tests green; new types are importable |
| **Effort** | S |

Tasks:
1. Add `LayerType::Procedural` variant with serde rename
2. Define `ProceduralEntry` and `ProceduralStep` structs
3. Define `MemoryProvider` trait + `ProviderCapabilities` + `ProviderHealth`
4. Define `HybridSearchResult` and `FusionStrategy` types
5. Unit tests for serialization/deserialization compatibility

### Slice 2: Procedural Memory Layer (US-1)

| Item | Detail |
|------|--------|
| **Goal** | Implement ProceduralMemoryLayer with store/retrieve/search/versioning |
| **Owner** | backend-engineer |
| **Dependencies** | Slice 1 |
| **Deliverables** | `layers/procedural_layer.rs`, DB migration, API endpoint |
| **Acceptance** | Can store/retrieve/search procedural entries; version evolution works |
| **Effort** | M |

Tasks:
1. Implement `ProceduralMemoryLayer` (MemoryLayer trait)
2. Add PostgreSQL migration for procedural memory table
3. Implement version tracking (same name → new version)
4. Implement search by task_type, tool name, semantic similarity
5. Add REST endpoint: `POST /api/v1/memory/procedural`
6. Register layer in `create_layers()`
7. Integration tests

### Slice 3: Memory Provider Framework (US-2 Foundation)

| Item | Detail |
|------|--------|
| **Goal** | Implement BuiltinProvider and provider selection logic |
| **Owner** | backend-engineer |
| **Dependencies** | Slice 1 |
| **Deliverables** | `providers/` module, `BuiltinProvider`, config integration |
| **Acceptance** | Provider can be selected via config; Builtin delegates to internal layers |
| **Effort** | M |

Tasks:
1. Create `providers/mod.rs` module structure
2. Implement `BuiltinProvider` (wraps existing layer chain)
3. Add TOML config section for provider selection
4. Implement provider factory (config → concrete provider)
5. Wire provider into MemoryKernel routing
6. Unit tests for provider routing logic

### Slice 4: External Providers (US-2 Complete)

| Item | Detail |
|------|--------|
| **Goal** | Implement Mem0Provider, ZepProvider, LettaProvider stub |
| **Owner** | backend-engineer |
| **Dependencies** | Slice 3 |
| **Deliverables** | `providers/mem0.rs`, `providers/zep.rs`, `providers/letta.rs` |
| **Acceptance** | Mem0/Zep providers pass integration tests against live instances; circuit breaker works |
| **Effort** | M |

Tasks:
1. Implement `Mem0Provider` (HTTP client with reqwest)
2. Implement `ZepProvider` (HTTP client with reqwest)
3. Implement `LettaProvider` stub (`unimplemented!()`)
4. Add circuit breaker middleware (timeout + retry + open/half-open/closed)
5. Add health check endpoint: `GET /api/v1/memory/provider/health`
6. Integration tests (requires Mem0/Zep test instances)

### Slice 5: GraphRAG Hybrid Search (US-3)

| Item | Detail |
|------|--------|
| **Goal** | Implement HybridSearchService with 3 fusion strategies |
| **Owner** | backend-engineer |
| **Dependencies** | Slice 1 (types only) |
| **Deliverables** | `services/hybrid_search.rs`, REST endpoint |
| **Acceptance** | Single API call returns fused results with provenance; all 3 strategies work |
| **Effort** | M |

Tasks:
1. Implement `HybridSearchService` struct with VectorSearch + GraphMemory deps
2. Implement RRF fusion algorithm
3. Implement VectorFirst and GraphFirst strategies
4. Add parallel execution with per-source timeout
5. Add provenance annotation to results
6. Add REST endpoint: `POST /api/v1/memory/search/hybrid`
7. Unit tests for RRF scoring
8. Integration tests with Qdrant + Neo4j

---

## Dependency Graph

```
Slice 1 (Foundation)
  ├── Slice 2 (Procedural Layer)
  ├── Slice 3 (Provider Framework)
  │     └── Slice 4 (External Providers)
  └── Slice 5 (Hybrid Search)
```

Slices 2, 3, and 5 can proceed in parallel after Slice 1.

---

## Risk & Mitigation

| Risk | Impact | Likelihood | Mitigation | Owner |
|------|--------|------------|-----------|-------|
| Mem0/Zep API incompatibility | Provider unusable | Medium | Pin API version; abstract behind trait | backend-engineer |
| Neo4j unavailable in test | Hybrid search untestable | Low | Docker compose for test env | backend-engineer |
| LayerType::Procedural breaks old clients | Deserialization error | Low | serde `rename_all` + integration test | backend-engineer |
| RRF quality insufficient | Poor search results | Medium | Configurable k + multi-strategy | tech-lead |
| Circuit breaker false positive | Provider disabled unnecessarily | Low | Conservative thresholds; health probe | backend-engineer |

---

## Checkpoint Schedule

| Checkpoint | Criteria | Gate Type |
|-----------|----------|-----------|
| Slice 1 complete | Types compile, tests green | Pre-flight for Slices 2-5 |
| Slice 2 complete | Procedural CRUD + search works | Revision (review quality) |
| Slice 3 complete | Builtin provider delegates correctly | Pre-flight for Slice 4 |
| Slice 4 complete | External providers pass health check | Revision |
| Slice 5 complete | Hybrid search returns fused results | Revision |
| All slices | Full integration test suite passes | Release gate |

---

## Role Assignment

| Role | Responsibility |
|------|---------------|
| tech-lead | Decision authority, slice prioritization, acceptance |
| backend-engineer | Implementation of all 5 slices |
| architect | Trait design review, interface sign-off |

---

## Karpathy Guidelines Compliance

### Assumptions (Explicit)
1. Procedural Memory v1 only needs linear steps (confirmed by user)
2. Mem0 and Zep have stable HTTP APIs suitable for integration
3. RRF with k=60 provides acceptable fusion quality
4. No MemoryLayer trait signature changes are acceptable

### Simpler Alternative Paths (Considered & Rejected)
1. **Single backend only** — rejected because architecture requires 4-backend support
2. **Procedural as metadata extension** — rejected because it needs dedicated search semantics
3. **Sequential hybrid search** — rejected because parallel is simple enough with tokio::join!

### What We're NOT Doing (Explicit Non-Goals)
- DAG-based procedural steps (deferred to v2)
- Letta full implementation (stub only)
- Frontend changes
- Performance benchmarking
- MemoryLayer trait signature changes
- Hot-swap provider at runtime (config change requires restart for v1)

### Why This Scope Is Sufficient
- Closes all 3 identified architecture gaps
- Maintains 100% backward compatibility
- Provides extensible foundation (new providers, new fusion strategies) without over-engineering
