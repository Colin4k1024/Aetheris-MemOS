---
phase: 01
slug: evidence-graph-decision-snapshots-completeness
status: ready
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-26
---

# Phase 01 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` |
| **Config file** | `backend/Cargo.toml` |
| **Quick run command** | `cd backend && cargo test --test evidence_graph -- --list` |
| **Feedback run command** | `cd backend && cargo test --test evidence_graph --test hash_chain` |
| **Full suite command** | `cd backend && cargo test` |
| **Estimated runtime** | Quick: ~5-15s, feedback: ~20-40s, full suite: project-dependent |

---

## Sampling Rate

- **After every task commit:** Run that task's `<automated>` command from the plan; if it exceeds ~45s, first run the quick command from the table above before the full task verify.
- **After Wave 1 and Wave 2:** Run `cd backend && cargo test --test evidence_graph --test hash_chain`
- **After Wave 3:** Run `cd backend && cargo test --test evidence_graph --test hash_chain --test evidence_api --test snapshot_export`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01-01 | 1 | EVID-01, EVID-02 | compile/schema | `cd backend && cargo check --lib` | created in task | ⬜ pending |
| 01-01-02 | 01-01 | 1 | EVID-01, EVID-02, EVID-03 bootstrap | integration scaffold | `cd backend && cargo test --test evidence_graph --test hash_chain` | creates Wave 0 tests | ⬜ pending |
| 01-02-01 | 01-02 | 2 | EVID-01, EVID-02, EVID-03 | integration | `cd backend && cargo test --test evidence_graph --test hash_chain` | uses 01-01 Wave 0 | ⬜ pending |
| 01-02-02 | 01-02 | 2 | EVID-01, EVID-02, EVID-03 | integration | `cd backend && cargo test --test evidence_graph --test hash_chain` | uses 01-01 Wave 0 | ⬜ pending |
| 01-03-01 | 01-03 | 3 | EVID-04 | integration | `cd backend && cargo test --test evidence_api` | created in task | ⬜ pending |
| 01-03-02 | 01-03 | 3 | EVID-05 | integration | `cd backend && cargo test --test snapshot_export` | created in task | ⬜ pending |
| 01-03-03 | 01-03 | 3 | COMP-01 | docs/manual | `rg -n "Evidence Graph|GET /api/v1/workflows/\\{id\\}/evidence|tamper-evident|EU AI Act|limitations" docs/ARCHITECTURE.md docs/evidence_graph.md` | docs updated in task | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `backend/tests/evidence_graph.rs` — tests for EVID-01/EVID-02
- [x] `backend/tests/hash_chain.rs` — tamper-detection tests for EVID-03

Wave 0 is satisfied by `01-01` Task 2, which creates both scaffolds before Plans `01-02` and `01-03` consume them.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Evidence API payload supports audit review narrative | EVID-04, COMP-01 | Requires human inspection of semantic clarity and docs consistency | Call `GET /api/v1/workflows/{id}/evidence`, compare fields with docs in `docs/ARCHITECTURE.md` and phase docs. |
| EU AI Act reporting example is technically accurate and not over-claiming | COMP-01 | Legal/compliance framing requires human review | Review docs wording for guarantees vs limitations and verify examples map to implemented fields. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Plan/task IDs match the actual phase plans (`01-01`, `01-02`, `01-03`)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency <= 45s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** ready for execution
