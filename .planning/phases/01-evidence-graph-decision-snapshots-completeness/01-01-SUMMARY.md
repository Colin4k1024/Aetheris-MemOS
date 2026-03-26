---
phase: 01-evidence-graph-decision-snapshots-completeness
plan: 01
subsystem: database
tags: [rust, sqlx, postgres, sqlite, evidence-graph, hash-chain, testing]
requires: []
provides:
  - Typed workflow evidence contracts for runs, nodes, edges, verification, responses, and exports
  - Append-only PostgreSQL and SQLite evidence graph schema for workflow audit storage
  - Wave 0 evidence graph and hash-chain integration test scaffolds
affects: [01-02, 01-03, evidence-api, snapshot-export]
tech-stack:
  added: []
  patterns: [ordered-btreemap-json, append-only-evidence-tables, direct-contract-integration-tests]
key-files:
  created:
    - backend/src/lib.rs
    - backend/src/models/evidence.rs
    - backend/migrations/20260326000100_workflow_evidence_graph.sql
    - backend/migrations_sqlite/20260326000100_workflow_evidence_graph.sql
    - backend/tests/evidence_graph.rs
    - backend/tests/hash_chain.rs
  modified:
    - backend/src/models/mod.rs
key-decisions:
  - "Evidence contracts use BTreeMap-backed JSON payloads to preserve deterministic serialization for later export hashing."
  - "Workflow evidence tables are append-only at the database layer via update/delete-blocking triggers in both PostgreSQL and SQLite."
  - "A minimal backend library target was added so the plan's cargo check --lib verification and direct contract integration tests can run."
patterns-established:
  - "Pattern 1: Evidence DTOs expose locked audit fields with ordered free-form payloads."
  - "Pattern 2: Evidence storage migrations mirror contract field names and enforce append-only semantics."
  - "Pattern 3: Wave 0 integration tests import contract types directly from the backend library target."
requirements-completed: [EVID-01, EVID-02]
duration: 4min
completed: 2026-03-26
---

# Phase 01 Plan 01: Evidence Contract and Schema Baseline Summary

**Typed workflow evidence contracts, append-only Postgres/SQLite schema, and Wave 0 hash-chain test baselines for audit-ready decision traces**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-26T13:56:53Z
- **Completed:** 2026-03-26T14:00:37Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added a dedicated evidence contract module covering workflow runs, nodes, edges, verification state, API responses, and deterministic exports.
- Created paired PostgreSQL and SQLite migrations for `workflow_evidence_runs`, `workflow_evidence_nodes`, and `workflow_evidence_edges` with locked audit fields and append-only triggers.
- Added Wave 0 integration tests that lock serialization, sequence ordering, and hash-chain tamper detection expectations before repository/API work begins.

## Task Commits

Each task was committed atomically:

1. **Task 1: Define evidence graph contracts and append-only schema** - `6810b64` (feat)
2. **Task 2: Add Wave 0 evidence graph and hash-chain test scaffolds** - `7d18408` (test)

## Files Created/Modified

- `backend/src/lib.rs` - Minimal library target so `cargo check --lib` and integration tests can import contracts directly.
- `backend/src/models/evidence.rs` - Evidence graph DTOs and ordered JSON payload types.
- `backend/src/models/mod.rs` - Re-exports the evidence contracts through the shared models module.
- `backend/migrations/20260326000100_workflow_evidence_graph.sql` - PostgreSQL append-only evidence graph schema and mutation-blocking triggers.
- `backend/migrations_sqlite/20260326000100_workflow_evidence_graph.sql` - SQLite-compatible append-only evidence graph schema and mutation-blocking triggers.
- `backend/tests/evidence_graph.rs` - Serialization and append-only sequence baseline tests.
- `backend/tests/hash_chain.rs` - Hash-chain tamper detection baseline tests for broken links, reordered sequence numbers, and mutated context snapshots.

## Decisions Made

- Used ordered `BTreeMap<String, serde_json::Value>` payloads for free-form evidence metadata so later export hashing has deterministic key order.
- Kept the legacy `decision_trace` table untouched and added a parallel evidence graph schema instead of repurposing existing trace storage.
- Enforced append-only storage in migrations instead of relying on repository discipline alone, so later plans inherit the integrity baseline.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added a minimal backend library target**
- **Found during:** Task 1 (Define evidence graph contracts and append-only schema)
- **Issue:** The plan's required verification command was `cd backend && cargo check --lib`, but the package had no `lib.rs`, so the command would not have been runnable.
- **Fix:** Added `backend/src/lib.rs` exposing the models module so the contracts and integration tests can compile as a library target.
- **Files modified:** `backend/src/lib.rs`
- **Verification:** `cd backend && cargo check --lib`
- **Committed in:** `6810b64`

**2. [Rule 3 - Blocking] Created the missing SQLite migration tree**
- **Found during:** Task 1 (Define evidence graph contracts and append-only schema)
- **Issue:** The plan required `backend/migrations_sqlite/20260326000100_workflow_evidence_graph.sql`, but the repository did not contain a `backend/migrations_sqlite/` directory yet.
- **Fix:** Added the directory and created the required SQLite migration with the same locked field names and append-only semantics as the PostgreSQL migration.
- **Files modified:** `backend/migrations_sqlite/20260326000100_workflow_evidence_graph.sql`
- **Verification:** `rg -n "CREATE TABLE IF NOT EXISTS workflow_evidence_runs|CREATE TABLE IF NOT EXISTS workflow_evidence_nodes|CREATE TABLE IF NOT EXISTS workflow_evidence_edges" backend/migrations/20260326000100_workflow_evidence_graph.sql backend/migrations_sqlite/20260326000100_workflow_evidence_graph.sql`
- **Committed in:** `6810b64`

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes were required to make the plan's specified verification and artifact paths executable. No scope creep beyond the baseline contract and schema work.

## Issues Encountered

- Parallel `git add` attempts hit `.git/index.lock`; staging was retried sequentially and no repository changes were lost.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan `01-02` can build repository logic and hash-chain verification against stable model and migration names.
- Plan `01-03` can expose the evidence API and export surface without redefining the contract layer.

## Self-Check: PASSED

---
*Phase: 01-evidence-graph-decision-snapshots-completeness*
*Completed: 2026-03-26*
