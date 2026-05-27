# Phase 3: MemOS Deep Fusion and Self-Healing Runtime - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning
**Mode:** Auto — all decisions at Claude's discretion

<domain>
## Phase Boundary

This phase implements global memory graph fusion across memory layers and resilient recovery primitives with autonomous self-healing.

In scope:
- Global memory graph fusion across STM, LTM, KG, MM layers (MEM-01)
- Semantic evolution and weight-decay behavior observable and tunable (MEM-02)
- Fault recovery workflow with autonomous diagnosis and constrained self-healing (MEM-03)

Out of scope:
- Evidence graph (Phase 1)
- Security hardening (Phase 2)

</domain>

<decisions>
## Implementation Decisions

### Memory Fusion
- **D-01:** Fusion uses a unified MemoryGraph that overlays STM→LTM→KG→MM as view layers
- **D-02:** Cross-layer queries fan out to all layers and merge results by relevance score
- **D-03:** Fusion is observable via a `/api/v1/memory/fusion/status` endpoint

### Semantic Evolution
- **D-04:** Weight-decay uses exponential decay: w(t) = w0 * e^(-λt) where λ is configurable
- **D-05:** Decay behavior exposed via `/api/v1/memory/weights/status` showing current weights and decay rates
- **D-06:** Tuning via config.toml: `[memory_evolution] decay_lambda = 0.01`

### Self-Healing
- **D-07:** Fault detection uses health checks on all memory layers
- **D-08:** Recovery strategies: restart layer, clear stale entries, reload from backup
- **D-09:** Self-healing actions are constrained — max 3 attempts per fault, exponential backoff

### Claude's Discretion
- Exact fusion algorithm (weighted sum vs. cascading)
- Specific health check intervals
- Backup/rollback strategy details

</decisions>

<canonical_refs>
## Canonical References

### Existing code touchpoints
- `backend/src/services/memory_orchestrator.rs` — existing memory orchestration
- `backend/src/db/stm.rs`, `db/ltm.rs`, `db/kg.rs`, `db/mm.rs` — existing repositories
- `backend/src/models/` — existing model types

### Requirements
- `REQUIREMENTS.md` §MEM-01, MEM-02, MEM-03

</canonical_refs>

<specifics>
## Specific Ideas

- Memory fusion should merge semantic search results from LTM with exact matches from STM
- Weight decay should be observable via metrics endpoint
- Self-healing should log all recovery attempts

</specifics>

<deferred>
## Deferred Ideas

- Multi-agent orchestration enhancements
- Enterprise governance workflows

</deferred>

---

*Phase: 03-memos-deep-fusion-and-self-healing-runtime*
*Context gathered: 2026-03-28 via autonomous mode*
