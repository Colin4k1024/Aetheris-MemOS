---
phase: 01-evidence-graph-decision-snapshots-completeness
plan: 03
subsystem: api
tags: [rust, axum, utoipa, sha256, evidence-graph, audit-api, docs]
requires:
  - phase: 01-02
    provides: evidence persistence, hash-chain verification, and live workflow evidence reads
provides:
  - live Axum workflow evidence retrieval endpoint for audit tooling
  - deterministic workflow evidence export contract and canonical export hash
  - architecture documentation for evidence guarantees, limits, and EU AI Act reporting usage
affects: [auditability, evidence-api, snapshot-export, compliance-docs]
tech-stack:
  added: []
  patterns: [live-axum-router-only-audit-endpoints, deterministic-export-body-hashing, evidence-docs-with-explicit-limits]
key-files:
  created:
    - backend/tests/evidence_api.rs
    - backend/tests/snapshot_export.rs
    - docs/evidence_graph.md
  modified:
    - backend/src/axum_routers/memory.rs
    - backend/src/axum_routers/mod.rs
    - backend/src/models/evidence.rs
    - backend/src/services/evidence_graph.rs
    - docs/ARCHITECTURE.md
key-decisions:
  - "The audit endpoint ships only on backend/src/axum_routers because backend/src/main.rs boots axum_routers::create_router()."
  - "Deterministic export hashing excludes exported_at from the canonical export body so re-exporting stored evidence does not change the canonical hash."
  - "Evidence documentation explicitly describes tamper-evident storage and audit support without claiming legal compliance or external truth verification."
patterns-established:
  - "Live audit APIs: add handlers and OpenAPI registration only on the router tree that main.rs actually serves."
  - "Evidence export contract: keep exported_at outside the canonical hashed body while preserving explicit version/hash metadata in the payload."
requirements-completed: [EVID-04, EVID-05, COMP-01]
duration: 7min
completed: 2026-03-26
---

# Phase 01 Plan 03: Evidence API, Export, and Documentation Summary

**Live workflow evidence retrieval on the shipped Axum router, deterministic offline export snapshots, and audit-focused evidence graph documentation**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-26T22:36:06+08:00
- **Completed:** 2026-03-26T22:42:57+08:00
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added `GET /api/v1/workflows/{id}/evidence` to the live Axum router and OpenAPI document used by the running backend.
- Reworked the export contract so offline snapshots carry explicit workflow and verification fields with a canonical export hash that stays stable across re-exports.
- Updated the architecture docs with the implemented evidence model, integrity guarantees, EU AI Act reporting example, and explicit system limitations.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add the live workflow evidence endpoint and API contract tests** - `66b0578` (test), `d2f2f79` (feat)
2. **Task 2: Make evidence exports deterministic and prove offline snapshot completeness** - `d39a523` (feat)
3. **Task 3: Update architecture docs with guarantees, limits, and EU AI Act reporting examples** - `8585dbf` (docs)

## Files Created/Modified

- `backend/tests/evidence_api.rs` - Live-router integration coverage for the workflow evidence endpoint, not-found contract, and OpenAPI path registration.
- `backend/src/axum_routers/memory.rs` - Added the shipped workflow evidence handler using typed `Path` and `Query` extractors.
- `backend/src/axum_routers/mod.rs` - Registered the workflow evidence path in the live OpenAPI document.
- `backend/src/models/evidence.rs` - Replaced the export wrapper with an explicit offline snapshot contract.
- `backend/src/services/evidence_graph.rs` - Added deterministic export assembly plus canonical export body hashing helpers.
- `backend/tests/snapshot_export.rs` - Locked export stability, field completeness, and post-export re-hash behavior with integration tests.
- `docs/ARCHITECTURE.md` - Documented the shipped evidence graph model, guarantees, audit API, reporting example, and limits.
- `docs/evidence_graph.md` - Added focused evidence graph documentation for audit and offline review usage.

## Decisions Made

- Kept the API response contract and export contract separate: the API continues to return `run`, `nodes`, `edges`, and `verification`, while the export snapshot carries the explicit offline-review fields required by the plan.
- Used the existing evidence verification result to populate `root_hash` and `chain_verified` in exports instead of inventing a second verification source.
- Documented evidence as a technical audit aid rather than a standalone compliance claim, because the system cannot independently validate external tool truthfulness or legal sufficiency.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Hardened the RED test harness against poisoned mutex state**
- **Found during:** Task 1 (Add the live workflow evidence endpoint and API contract tests)
- **Issue:** The first failing RED run poisoned the shared test mutex, which hid the intended missing-route failures behind `PoisonError`.
- **Fix:** Added a small `test_guard()` helper in `backend/tests/evidence_api.rs` that recovers the mutex after a failed test so RED and GREEN runs report the actual endpoint contract status.
- **Files modified:** `backend/tests/evidence_api.rs`
- **Verification:** `cd backend && cargo test --test evidence_api`
- **Committed in:** `66b0578`

---

**Total deviations:** 1 auto-fixed (1 Rule 3 blocking issue)
**Impact on plan:** The auto-fix kept the TDD loop accurate and did not change scope.

## Issues Encountered

- Parallel `git add` calls briefly produced `.git/index.lock` contention while staging the task 1 implementation. I verified the lock was stale, retried staging sequentially, and completed the commit without losing changes.
- The GSD metadata commit helper later failed with a git index-lock permission error, so I recovered with plain non-interactive `git add` and `git commit` for the summary/state update commit as instructed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 01 now exposes a live audit retrieval surface, deterministic export payloads, and matching architecture docs, so the evidence-graph milestone is ready to close.
- Later work can build broader audit tooling or UI exploration on top of the stable `/api/v1/workflows/{id}/evidence` contract without redefining the evidence model.

## Self-Check: PASSED

- FOUND: `.planning/phases/01-evidence-graph-decision-snapshots-completeness/01-03-SUMMARY.md`
- FOUND: `66b0578`
- FOUND: `d2f2f79`
- FOUND: `d39a523`
- FOUND: `8585dbf`
