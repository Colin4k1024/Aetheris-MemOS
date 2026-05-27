---
phase: 01-evidence-graph-decision-snapshots-completeness
plan: 02
subsystem: api
tags: [rust, axum, sqlx, sqlite, postgres, sha256, evidence-graph]
requires:
  - phase: 01-01
    provides: evidence graph contracts, storage schema, and baseline tests
provides:
  - append-only evidence repository for workflow runs, nodes, and edges
  - canonical hash-chain generation and tamper verification for decision traces
  - dual-write trace persistence that keeps legacy blobs while appending workflow evidence
affects: [decision-traces, evidence-api, auditability]
tech-stack:
  added: [sha2, sqlx]
  patterns: [append-only evidence persistence, canonical JSON hashing, legacy-and-new dual writes]
key-files:
  created: [backend/src/db/evidence_graph.rs, backend/src/services/evidence_graph.rs]
  modified: [backend/src/lib.rs, backend/src/db/mod.rs, backend/src/db/decision_trace.rs, backend/src/services/mod.rs, backend/src/services/memory_orchestrator.rs, backend/tests/evidence_graph.rs, backend/tests/hash_chain.rs]
key-decisions:
  - "Decision traces continue writing the legacy decision_trace blob before appending evidence graph records."
  - "Evidence verification uses canonicalized JSON bytes and SHA-256 replay over ordered seq_no nodes."
  - "Decision trace persistence supports both Postgres and SQLite so integration tests exercise the live path."
patterns-established:
  - "Evidence persistence: create workflow run, append ordered nodes, append directed edges, then verify the stored chain."
  - "Trace dual-write: preserve existing blob consumers while new evidence readers query by workflow_id and verification status."
requirements-completed: [EVID-01, EVID-02, EVID-03]
duration: 19 min
completed: 2026-03-26
---

# Phase 01 Plan 02: Evidence Graph Persistence Summary

**Append-only workflow evidence storage with canonical SHA-256 chain verification and legacy-compatible decision trace dual writes**

## Performance

- **Duration:** 19 min
- **Started:** 2026-03-26T22:13:25+08:00
- **Completed:** 2026-03-26T22:32:00+08:00
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Added a dedicated evidence repository that creates workflow runs, appends ordered nodes and edges, and lists evidence in stable sequence order.
- Implemented production evidence services that canonicalize trace snapshots, compute `prev_hash` and `node_hash`, and fail verification on tampered chain data.
- Wired live trace persistence to keep the legacy `decision_trace` blob path intact while appending evidence graph records for the same workflow.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement append-only evidence repository and canonical hash-chain service** - `fabf6d4` (test), `5cf31ec` (feat)
2. **Task 2: Wire trace persistence into the live decision path without breaking legacy storage** - `f893f1b` (feat)

## Files Created/Modified

- `backend/src/lib.rs` - Exposed shared backend modules so integration tests can exercise the production repository and service stack.
- `backend/src/db/evidence_graph.rs` - Added the append-only repository for workflow runs, evidence nodes, edges, and ordered evidence reads.
- `backend/src/db/mod.rs` - Registered the evidence graph repository module.
- `backend/src/db/decision_trace.rs` - Extended the legacy trace repository to work against both Postgres and SQLite pools.
- `backend/src/services/evidence_graph.rs` - Added decision-trace to evidence conversion, canonical hashing, chain verification, and evidence listing helpers.
- `backend/src/services/mod.rs` - Registered the evidence graph service module.
- `backend/src/services/memory_orchestrator.rs` - Dual-wired trace persistence to write the legacy blob and new evidence graph artifacts together.
- `backend/tests/evidence_graph.rs` - Reworked integration coverage to hit production persistence code, including live dual-write verification.
- `backend/tests/hash_chain.rs` - Reworked tamper-detection coverage to call the production verification path.

## Decisions Made

- Preserved the existing `decision_trace` insert path and appended evidence writes after it so `/api/v1/memory/traces` behavior stays unchanged while Phase 01 gains a live evidence source.
- Used canonicalized JSON plus SHA-256 replay over ordered evidence nodes so verification fails closed when `prev_hash`, `node_hash`, or snapshot bytes drift.
- Added runtime SQLite support to `DecisionTraceRepository` because the integration suite needs the real persistence path, not test-only scaffolding.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Expanded the backend library surface for integration tests**
- **Found during:** Task 1 (Implement append-only evidence repository and canonical hash-chain service)
- **Issue:** The backend library only exposed models, so the integration tests could not import the production config, db, error, and services modules required by the plan.
- **Fix:** Expanded `backend/src/lib.rs` to re-export the production modules and shared response/error types needed by the tests.
- **Files modified:** `backend/src/lib.rs`
- **Verification:** `cargo test --test evidence_graph --test hash_chain`
- **Committed in:** `5cf31ec`

**2. [Rule 3 - Blocking] Replayed SQLite migrations inside the evidence integration harness**
- **Found during:** Task 1 (Implement append-only evidence repository and canonical hash-chain service)
- **Issue:** Shared in-memory SQLite tests did not reliably retain the evidence schema across connections, which blocked production repository tests.
- **Fix:** Applied the SQLite evidence migration during test database initialization so each test run starts with the required workflow/node/edge tables.
- **Files modified:** `backend/tests/evidence_graph.rs`, `backend/tests/hash_chain.rs`
- **Verification:** `cargo test --test evidence_graph --test hash_chain`
- **Committed in:** `5cf31ec`

**3. [Rule 3 - Blocking] Made legacy decision trace persistence runtime-database aware**
- **Found during:** Task 2 (Wire trace persistence into the live decision path without breaking legacy storage)
- **Issue:** `DecisionTraceRepository` was Postgres-only, which prevented the live dual-write path from running under the SQLite integration harness.
- **Fix:** Added Postgres and SQLite execution branches for create and read operations, then verified the real orchestrator path writes both storage forms.
- **Files modified:** `backend/src/db/decision_trace.rs`, `backend/src/services/memory_orchestrator.rs`, `backend/tests/evidence_graph.rs`
- **Verification:** `cargo test --test evidence_graph --test hash_chain`
- **Committed in:** `f893f1b`

---

**Total deviations:** 3 auto-fixed (3 Rule 3 blocking issues)
**Impact on plan:** All deviations were required to exercise the planned production path and did not expand the intended feature scope.

## Issues Encountered

- Parallel staging hit `.git/index.lock` contention during task commits. I recovered by checking for the active git process and switching to sequential non-interactive staging.
- Sandboxed `git add` later failed with an `Operation not permitted` write to the git index. I reran the required staging and commit with escalated non-interactive git commands.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The backend now persists workflow evidence with verification metadata that later API plans can expose by `workflow_id`.
- Existing trace consumers still read the legacy blob store, so follow-on plans can ship query endpoints incrementally without a compatibility break.

## Self-Check: PASSED

- FOUND: `.planning/phases/01-evidence-graph-decision-snapshots-completeness/01-02-SUMMARY.md`
- FOUND: `fabf6d4`
- FOUND: `5cf31ec`
- FOUND: `f893f1b`
