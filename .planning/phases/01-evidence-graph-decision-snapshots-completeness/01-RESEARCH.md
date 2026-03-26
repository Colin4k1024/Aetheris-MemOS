# Phase 1: Evidence Graph & Decision Snapshots Completeness - Research

**Researched:** 2026-03-26
**Domain:** Rust/Axum evidence persistence, hash-chain integrity, snapshot export, and audit APIs
**Confidence:** MEDIUM

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Evidence graph must be represented as a tamper-evident directed structure in the event/trace store path.
- Node fields must include `timestamp`, `attempt_id`, `llm_input_hash`, `llm_output_hash`, `tool_invocations[]`, and `context_snapshot`.
- Hash-chain integrity should support post-hoc tamper detection.
- Decision snapshots should be point-in-time and serializable for offline analysis.
- API exposure target is `GET /api/v1/workflows/{id}/evidence`.
- Docs must include compliance-oriented explanation and examples.

### Claude's Discretion
- Exact schema evolution strategy and migration path.
- Hashing algorithm selection and verification endpoint/CLI placement.
- API response shape and pagination strategy.
- Test granularity and benchmark scope.

### Deferred Ideas (OUT OF SCOPE)
- Cross-workflow signaling and multi-agent orchestration enhancements (covered by later issues/phases).
- Full policy/governance automation for enterprise compliance.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EVID-01 | Decision trace storage must represent workflow decisions as graph-compatible nodes and relationships. | Add append-only evidence node/edge tables and keep graph assembly in a dedicated service/repository boundary. |
| EVID-02 | Every decision node must store integrity-critical fields (`timestamp`, `attempt_id`, `llm_input_hash`, `llm_output_hash`, `tool_invocations`, `context_snapshot`). | Use typed evidence payload structs persisted as JSON plus indexed scalar columns for audit queries. |
| EVID-03 | Hash-chain verification must detect tampering by validating previous-hash linkage across node sequences. | Reuse SHA-256 hashing and compute per-node `prev_hash`/`node_hash` over canonical bytes. |
| EVID-04 | Workflow evidence must be queryable through an API suitable for audit tooling. | Implement the endpoint on the live `axum_routers` stack, expose pagination/filtering, and document it through `utoipa`. |
| EVID-05 | Decision snapshots must be serializable and exportable for offline compliance review. | Persist versioned snapshot/export payloads and guarantee deterministic serialization for export + re-hash. |
| COMP-01 | Architecture docs must define the evidence graph model, integrity guarantees, and EU AI Act reporting examples. | Update `docs/ARCHITECTURE.md` and add evidence-specific documentation with guarantee/limitation language and reporting examples. |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- Backend work belongs in the Rust monorepo layout under `backend/src/{db,services,routers,models}`.
- Frontend work belongs in `frontend/ant-design-pro-template/` if the phase touches UI clients or pages.
- New API endpoints should use Axum typed extractors (`Json`, `Query`, `Path`, `Extension`); however the running server currently boots `backend/src/axum_routers`, so Phase 1 must land on the live router tree or switch the app to the richer router in the same phase.
- Database operations should use SQLx patterns and compile-time checked queries/macros where feasible.
- HTTP errors should use `AppError` from `backend/src/error.rs`.
- Standard validation commands remain `cd backend && cargo build && cargo test` and `cd frontend/ant-design-pro-template && npm test && npm run lint`.
- Environment assumptions from project docs are Rust 1.89+, Node.js 20+, PostgreSQL 14+, Qdrant via Docker, and optional Neo4j.
- No repository-local `.claude/skills/` or `.agents/skills/` directories were present, so there are no extra project skill rules beyond the mapped codebase docs.

## Summary

Phase 1 is not a greenfield evidence implementation. The repository already has three adjacent pieces that matter: a scheduler-level `DecisionTrace` struct, a `decision_trace` table that stores the entire trace as opaque `TEXT`, and an Oris snapshot subsystem that models point-in-time task state in memory. None of those pieces currently satisfy the phase goal by themselves. The live backend (`backend/src/main.rs`) serves `backend/src/axum_routers`, and that live memory router still returns placeholder trace responses, so adding evidence features only under `backend/src/routers` would not ship anything real.

The safest planning direction is additive and live-router-first: keep the existing decision trace shape for compatibility, introduce a new evidence graph persistence model with immutable nodes and edges, compute a full SHA-256 hash chain over canonical bytes, and expose `GET /api/v1/workflows/{id}/evidence` from the router tree the server actually boots. Snapshot export should be versioned and deterministic. If any exported field remains arbitrary JSON or `HashMap` data, canonicalization must be explicit; relying on `serde_json::to_string` over unordered maps is not a serialization guarantee.

Documentation work is part of the feature, not cleanup. The current docs describe decision traces and separate snapshot endpoints, but not the audit model promised by the phase. The architecture update needs to explain what is tamper-evident, what is merely inspectable, what the verification endpoint checks, and how the payload supports compliance reporting without overstating legal guarantees.

**Primary recommendation:** Implement Phase 1 as an append-only evidence subsystem on the live Axum router, with typed node/edge storage, canonical snapshot hashing, and docs that explicitly separate guarantees from limitations.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | repo pin `0.8` / docs.rs current `0.8.8` | Live HTTP endpoint and extractors for the audit API | Already powers the running backend; official extractor patterns fit the required `Path` + `Query` evidence endpoint. |
| `sqlx` | repo pin `0.8` / docs.rs current `0.8.6` | Migrations, typed row mapping, JSON payload persistence | Already owns DB init and migrations; `sqlx::types::Json<T>` avoids blob-only persistence. |
| `serde` + `serde_json` | repo pin `1` / docs.rs current `1.0.149` | Snapshot serialization, export payloads, DTOs | Existing serialization layer across backend models; compatible with typed evidence payloads. |
| `sha2` + `hex` | repo pins `0.10` and `0.4` | Full SHA-256 node hashing and verification output | Already used by `information_guard`; no new crypto primitive is needed. |
| `utoipa` | repo pin `5` / docs.rs current `5.4.0` | OpenAPI generation for evidence API and docs alignment | The running Axum router already emits OpenAPI via `utoipa`. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `ulid` | repo pin `1` / docs.rs current `1.2.1` | Sortable identifiers for evidence nodes, snapshots, and attempts | Use for append-only evidence IDs and page cursors. |
| `chrono` | repo pin `0.4` | UTC timestamps in evidence rows and exports | Use for explicit audit timestamps and verification windows. |
| `serde_canonical_json` | docs.rs current `1.0.0` | RFC-style canonical JSON serialization | Use only if `context_snapshot` or tool payloads remain arbitrary JSON objects or unordered maps. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| New work only in `backend/src/routers` | Keep building on the legacy richer router tree | Wrong shipping surface unless `main.rs` is switched in the same phase. |
| Opaque `trace_json TEXT` storage | Keep the existing blob-only repository | Faster to code but fails EVID-01/EVID-04 because relationships and integrity fields are not queryable. |
| Ad hoc JSON hashing | Raw `serde_json::to_string` over current structs | Works only while all fields are ordered and typed; breaks once unordered maps or free-form JSON enter the hash path. |
| Extra graph database for evidence | Neo4j-backed evidence graph | Adds infrastructure and migration risk without need; Phase 1 only needs graph-compatible storage, not graph-db dependency. |

**Installation:**
```bash
# Baseline Phase 1 can reuse the existing backend stack.
# Add this only if arbitrary JSON canonicalization is required.
cargo add serde_canonical_json
```

**Version verification:** Verified against current docs.rs pages on 2026-03-26: `axum 0.8.8` (published 3 months earlier), `sqlx 0.8.6`, `utoipa 5.4.0` (published 9 months earlier), `serde_json 1.0.149`, `ulid 1.2.1`, and `serde_canonical_json 1.0.0`.

## Architecture Patterns

### Recommended Project Structure
```text
backend/src/
├── models/evidence.rs          # Evidence node, edge, snapshot, export DTOs
├── db/evidence.rs              # SQLx repositories for evidence rows and queries
├── services/evidence.rs        # Hash-chain creation, verification, export assembly
├── axum_routers/evidence.rs    # Live GET /api/v1/workflows/{id}/evidence endpoint
├── services/information_guard.rs
│                               # Reuse/extend SHA-256 helpers here
└── migrations/
    └── *_evidence_graph.sql    # Additive schema for workflow evidence

docs/
├── ARCHITECTURE.md             # Update main architecture narrative
└── EVIDENCE_GRAPH.md           # Optional focused audit/integrity doc
```

### Pattern 1: Append-Only Evidence Graph
**What:** Persist each workflow attempt as immutable evidence nodes plus directed edges, with scalar integrity fields indexed separately from the JSON payload.
**When to use:** For every persisted workflow decision path that must support post-hoc verification and audit retrieval.
**Example:**
```rust
use serde::{Deserialize, Serialize};
use sqlx::types::Json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceNode {
    pub node_id: String,
    pub workflow_id: String,
    pub attempt_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub llm_input_hash: String,
    pub llm_output_hash: String,
    pub tool_invocations: Vec<ToolInvocation>,
    pub context_snapshot: ContextSnapshotExport,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EvidenceNodeRow {
    pub node_id: String,
    pub workflow_id: String,
    pub seq_no: i32,
    pub prev_hash: Option<String>,
    pub node_hash: String,
    pub payload: Json<EvidenceNode>,
}
```
Source: `https://docs.rs/sqlx/latest/sqlx/types/`

### Pattern 2: Canonical Hash-Chain Verification
**What:** Compute `node_hash = sha256(canonical(previous_hash + node_payload_bytes))`, persist both `prev_hash` and `node_hash`, and verify by replaying the sequence in order.
**When to use:** On every persisted node and in the verification endpoint/CLI.
**Example:**
```rust
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct HashableNode<'a, T> {
    prev_hash: Option<&'a str>,
    payload: &'a T,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
```
Source: `https://docs.rs/crate/sha2/latest`

### Pattern 3: Live-Router Audit Endpoint
**What:** Put the evidence API on `backend/src/axum_routers/` or switch the application router in the same phase; do not strand the feature in the unused router tree.
**When to use:** For `GET /api/v1/workflows/{id}/evidence` and any verification endpoint.
**Example:**
```rust
use axum::{
    extract::{Path, Query},
    Json,
};

async fn get_workflow_evidence(
    Path(workflow_id): Path<String>,
    Query(query): Query<EvidenceQuery>,
) -> Result<Json<WorkflowEvidenceResponse>, crate::AppError> {
    // delegate to services::evidence
    todo!()
}
```
Source: `https://docs.rs/crate/axum/latest/source/src/docs/extract.md`

### Pattern 4: Versioned Snapshot Export Contract
**What:** Export a versioned, point-in-time snapshot object that includes schema version, hash algorithm, verification result, and the exact evidence nodes/edges or references needed for offline review.
**When to use:** For EVID-05 exports and any downloadable audit artifact.
**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEvidenceExport {
    pub schema_version: &'static str,
    pub hash_algorithm: &'static str,
    pub workflow_id: String,
    pub attempt_id: String,
    pub root_hash: String,
    pub chain_verified: bool,
    pub nodes: Vec<EvidenceNode>,
    pub edges: Vec<EvidenceEdge>,
    pub exported_at: chrono::DateTime<chrono::Utc>,
}
```
Source: `https://www.rfc-editor.org/info/rfc8785`

### Anti-Patterns to Avoid
- **Implementing only in `backend/src/routers`:** `main.rs` serves `axum_routers::create_router()`, so the feature will not be live.
- **Hashing unordered maps directly:** `serde_json::Map` is only deterministic by default when backed by `BTreeMap`; current Oris snapshot metadata uses `HashMap`.
- **Replacing the existing trace blob without a compatibility plan:** The frontend already calls `/api/v1/memory/adaptive/trace` and `/api/v1/memory/traces`.
- **Using Neo4j for Phase 1 evidence storage:** The feature does not require a separate graph service and local Neo4j is unavailable.
- **Compliance theater:** Docs must say what integrity checks detect and what they do not detect, especially around retention, provenance, and external tool outputs.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Canonical bytes for hashing | String concatenation or manual JSON key sorting | Typed `serde` structs plus ordered maps, or `serde_canonical_json` when free-form JSON is unavoidable | Hash stability is easy to get subtly wrong. |
| Cryptographic digesting | Custom short hashes or reversible IDs | Existing `sha2` + hex encoding | The repo already uses SHA-256 in `information_guard`; reuse the same primitive. |
| OpenAPI payload docs | Hand-written JSON/YAML documents detached from handlers | `utoipa` derives on the live router | Keeps docs in sync with the actual endpoint. |
| Graph query assembly | Regex/parsing over `trace_json` blobs | Node/edge tables plus service-layer assembly | Needed for EVID-01 and paginated audit retrieval. |
| Workflow verification | One-off SQL scripts only | Service-level verifier plus optional CLI wrapper | Centralizes hash-chain rules and makes tests straightforward. |

**Key insight:** The deceptively hard part of this phase is not hashing. It is making hashing, storage, export, and the live API all use the same canonical evidence representation.

## Common Pitfalls

### Pitfall 1: Fixing the wrong router
**What goes wrong:** The evidence endpoint is added to `backend/src/routers`, tests pass against `routers::root()`, but the running app still returns placeholder data from `backend/src/axum_routers/memory.rs`.
**Why it happens:** The codebase has two router trees, and the richer one is not the one served by `main.rs`.
**How to avoid:** Treat router selection as a Phase 1 decision. Either implement evidence routes under `axum_routers` or switch `main.rs` to the richer router as part of the same phase.
**Warning signs:** OpenAPI shows the route, but `cargo run` still returns `"[]"` or `"{}"` for trace endpoints.

### Pitfall 2: Non-deterministic snapshot hashing
**What goes wrong:** A stored snapshot re-serializes to different bytes later, causing false tamper alerts.
**Why it happens:** Arbitrary JSON objects and `HashMap` fields do not guarantee stable key order.
**How to avoid:** Hash typed exports only, replace unordered maps with `BTreeMap`, or canonicalize JSON before hashing.
**Warning signs:** Re-hashing the same logical payload yields different digests across runs.

### Pitfall 3: Blob storage masquerading as a graph
**What goes wrong:** The schema still stores only `trace_json TEXT`, so relationships cannot be filtered, paged, or verified independently.
**Why it happens:** Reusing the current repository is tempting because it already persists traces.
**How to avoid:** Keep the legacy blob only as compatibility/export if needed, but add first-class node and edge persistence.
**Warning signs:** Queries need full JSON deserialization just to answer basic audit questions.

### Pitfall 4: Undefined workflow identity
**What goes wrong:** The new API promises `/api/v1/workflows/{id}/evidence`, but the codebase only has `task_id` and no durable `workflow_id` concept.
**Why it happens:** Existing traces are task-centric and do not model retries/attempts explicitly.
**How to avoid:** Decide whether `task_id` becomes `workflow_id` or whether Phase 1 introduces a new workflow header row with one-to-many attempts.
**Warning signs:** Multiple retries for the same task overwrite or ambiguously share evidence.

### Pitfall 5: Compliance docs that over-claim
**What goes wrong:** Documentation implies legal sufficiency rather than technical support for auditability.
**Why it happens:** "Tamper-evident" and "EU AI Act ready" are easy to oversell.
**How to avoid:** Document exact guarantees: integrity of stored node sequence, export reproducibility, and example reporting fields. Also document limits: external system trust, retention policy, and operator process still matter.
**Warning signs:** Docs use words like "compliant" without naming the remaining organizational controls.

## Code Examples

Verified patterns from official sources:

### Axum Evidence Handler Skeleton
```rust
use axum::{
    extract::{Path, Query},
    Json,
};

#[derive(serde::Deserialize)]
struct EvidenceQuery {
    limit: Option<u32>,
    after: Option<String>,
    verify: Option<bool>,
}

async fn get_workflow_evidence(
    Path(workflow_id): Path<String>,
    Query(query): Query<EvidenceQuery>,
) -> Result<Json<WorkflowEvidenceResponse>, crate::AppError> {
    todo!()
}
```
Source: `https://docs.rs/crate/axum/latest/source/src/docs/extract.md`

### SQLx JSON Payload Row
```rust
#[derive(Debug, sqlx::FromRow)]
struct EvidenceSnapshotRow {
    workflow_id: String,
    attempt_id: String,
    root_hash: String,
    snapshot: sqlx::types::Json<WorkflowEvidenceExport>,
}
```
Source: `https://docs.rs/sqlx/latest/sqlx/types/`

### Canonical Snapshot Hash Wrapper
```rust
use serde::Serialize;
use serde_json::Serializer;
use serde_canonical_json::CanonicalFormatter;

fn canonical_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    let mut serializer = Serializer::with_formatter(Vec::new(), CanonicalFormatter::new());
    value.serialize(&mut serializer).expect("serialize");
    serializer.into_inner()
}
```
Source: `https://docs.rs/serde_canonical_json`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Persist one opaque trace blob | Persist immutable evidence nodes/edges plus versioned export payload | Modern event/audit system practice; compatible with current 2025-2026 Rust stack | Makes audits, pagination, and verification possible without full blob parsing. |
| Hash arbitrary JSON text as produced | Hash canonical bytes or strictly typed ordered payloads | RFC 8785 published June 2020 | Eliminates false mismatches caused by serialization order. |
| Static/manual API docs | Code-first OpenAPI on live handlers | Established in current `utoipa` 5.x workflow | Keeps audit endpoint docs aligned with runtime. |

**Deprecated/outdated:**
- `decision_trace.trace_json TEXT` as the only source of truth for evidence: adequate for explainability demos, not for graph queries or tamper-evident audit retrieval.
- Implementing new evidence endpoints only under `backend/src/routers`: outdated in this repo until router unification happens.

## Open Questions

1. **What is the canonical `workflow_id`?**
   - What we know: existing persistence and UI flows key off `task_id`.
   - What's unclear: whether `task_id` is durable enough to serve as workflow identity across retries/attempts.
   - Recommendation: resolve this before schema design; if unsure, add a workflow header row and treat current `task_id` as an external reference.

2. **How much of `context_snapshot` must be embedded versus referenced?**
   - What we know: Oris snapshots currently hold in-memory task state, STM entries, and metadata.
   - What's unclear: whether offline audit exports require full embedded memory state or stable references plus hashes.
   - Recommendation: define a minimal export contract for Phase 1 and explicitly defer large memory replay payloads if not needed for requirements.

3. **Should legacy trace endpoints remain source-compatible?**
   - What we know: the frontend already uses `/api/v1/memory/adaptive/trace` and `/api/v1/memory/traces`.
   - What's unclear: whether Phase 1 must retrofit those endpoints or can ship the new workflow evidence endpoint beside them.
   - Recommendation: keep legacy endpoints working, but back them with the new evidence service or mark them as compatibility views over the new store.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Cargo | Backend build/tests | ✓ | `1.90.0` | — |
| Rust | Backend compile/tests | ✓ | `1.90.0` | — |
| Node.js | Frontend client/docs updates | ✓ | `v22.16.0` | — |
| npm | Frontend client/tests | ✓ | `10.9.2` | — |
| Docker CLI | Local PostgreSQL/Qdrant/Neo4j bring-up | ✓ | `28.4.0` | — |
| Docker daemon | Local infra execution | ✗ | — | Start Docker Desktop/daemon before integration verification |
| PostgreSQL CLI (`psql`/`pg_isready`) | DB inspection/readiness checks | ✗ | — | Use application-level tests only until CLI/server is available |
| Qdrant service | Full memory stack integration | ✗ | — | Not required for pure evidence/hash-chain unit tests |
| Neo4j service | Optional KG integration | ✗ | — | Skip; Phase 1 evidence graph should stay relational |

**Missing dependencies with no fallback:**
- Running local Postgres-backed integration checks through normal ops tooling is blocked until Docker daemon or a separate PostgreSQL instance is available.

**Missing dependencies with fallback:**
- Qdrant and Neo4j are not needed for the core evidence graph phase if storage stays in PostgreSQL/SQLx.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework via `cargo test`; frontend Jest 30 for any client updates |
| Config file | Backend: none; frontend: `frontend/ant-design-pro-template/jest.config.ts` |
| Quick run command | `cd backend && cargo test evidence_hash_chain -- --exact --nocapture` |
| Full suite command | `cd backend && cargo test && cd ../frontend/ant-design-pro-template && npm test -- --runInBand` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EVID-01 | Evidence nodes and relationships persist in graph-compatible form | integration | `cd backend && cargo test evidence_graph_repository -- --exact` | ❌ Wave 0 |
| EVID-02 | Integrity-critical fields are stored and serialized completely | unit | `cd backend && cargo test evidence_node_serialization -- --exact` | ❌ Wave 0 |
| EVID-03 | Hash-chain verifier detects altered payloads and broken linkage | unit | `cd backend && cargo test evidence_hash_chain -- --exact` | ❌ Wave 0 |
| EVID-04 | `GET /api/v1/workflows/{id}/evidence` returns audit-friendly payloads and paging metadata | integration | `cd backend && cargo test workflow_evidence_api -- --exact` | ❌ Wave 0 |
| EVID-05 | Offline export snapshots round-trip and re-hash deterministically | unit | `cd backend && cargo test evidence_export_roundtrip -- --exact` | ❌ Wave 0 |
| COMP-01 | Docs describe evidence graph, integrity guarantees, and EU AI Act examples | smoke/manual | `rg -n "evidence graph|root_hash|EU AI Act|Article 20|Annex IV" docs/ARCHITECTURE.md docs/API_ENDPOINTS.md docs/EVIDENCE_GRAPH.md` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cd backend && cargo test evidence_hash_chain -- --exact`
- **Per wave merge:** `cd backend && cargo test`
- **Phase gate:** Backend tests green, evidence API route tested against the live router, and docs grep checks passing before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `backend/tests/evidence_api.rs` - live-router integration tests for `GET /api/v1/workflows/{id}/evidence`
- [ ] `backend/tests/evidence_repository.rs` - schema/query tests for node/edge persistence
- [ ] `backend/src/services/evidence.rs` - hash-chain and export round-trip unit tests
- [ ] `backend/src/models/evidence.rs` - serialization determinism tests
- [ ] `docs/EVIDENCE_GRAPH.md` or equivalent doc target - explicit source for COMP-01 grep checks

## Sources

### Primary (HIGH confidence)
- Local repository inspection:
  - `backend/src/main.rs`
  - `backend/src/axum_routers/mod.rs`
  - `backend/src/axum_routers/memory.rs`
  - `backend/src/routers/mod.rs`
  - `backend/src/routers/memory.rs`
  - `backend/src/db/decision_trace.rs`
  - `backend/migrations/20240101000009_decision_trace.sql`
  - `backend/src/services/memory_orchestrator.rs`
  - `backend/src/services/scheduler.rs`
  - `backend/src/services/context_snapshot.rs`
  - `backend/src/integrations/oris.rs`
  - `backend/src/services/information_guard.rs`
  - `docs/ARCHITECTURE.md`
  - `docs/API_ENDPOINTS.md`
- Axum extractor docs: `https://docs.rs/crate/axum/latest/source/src/docs/extract.md`
- SQLx migration docs: `https://docs.rs/sqlx/latest/sqlx/migrate/struct.Migrator.html`
- SQLx JSON type docs: `https://docs.rs/sqlx/latest/sqlx/types/`
- `serde_json::Map` docs on default `BTreeMap` backing: `https://docs.rs/serde_json/latest/serde_json/map/index.html`
- `utoipa` crate docs: `https://docs.rs/crate/utoipa/latest`
- RFC 8785 JSON Canonicalization Scheme: `https://www.rfc-editor.org/info/rfc8785`
- EU AI Act official text: `https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX%3A32024R1689`

### Secondary (MEDIUM confidence)
- `serde_canonical_json` crate docs for RFC-style canonicalization: `https://docs.rs/serde_canonical_json`
- Official proposal-stage EUR-Lex search result for automatically generated logs (useful framing, but final obligations should be checked against the final regulation text): `https://eur-lex.europa.eu/legal-content/EN-FR/TXT/?from=EN&uri=CELEX%3A52021PC0206`

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - The phase can reuse the repository's current Rust/Axum/SQLx/utoipa stack, and official docs confirm the recommended APIs.
- Architecture: MEDIUM - The required design is clear, but the router split and missing `workflow_id` concept create planning decisions that must be resolved early.
- Pitfalls: HIGH - The biggest risks are directly observable in the current codebase and docs.

**Research date:** 2026-03-26
**Valid until:** 2026-04-25
