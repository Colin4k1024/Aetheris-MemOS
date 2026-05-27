# Phase 1: Evidence Graph & Decision Snapshots Completeness - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning
**Source:** Latest GitHub issue `#74` (Architecture)

<domain>
## Phase Boundary

This phase closes the implementation and documentation gap for Evidence Graph and Decision Snapshots.

In scope:
- Evidence graph storage and query capability in decision trace path.
- Hash-chain integrity computation and verification for decision nodes.
- Snapshot completeness and serialization/export behavior.
- Audit-facing API surface for evidence retrieval.
- Architecture documentation for data model, integrity guarantees, and compliance reporting examples.

Out of scope:
- Broader security-hardening epics from phase-2 issues.
- MemOS deep-fusion feature work from phase-3 issues.

</domain>

<decisions>
## Implementation Decisions

### Locked decisions from issue #74
- Evidence graph must be represented as a tamper-evident directed structure in the event/trace store path.
- Node fields must include `timestamp`, `attempt_id`, `llm_input_hash`, `llm_output_hash`, `tool_invocations[]`, and `context_snapshot`.
- Hash-chain integrity should support post-hoc tamper detection.
- Decision snapshots should be point-in-time and serializable for offline analysis.
- API exposure target is `GET /api/v1/workflows/{id}/evidence`.
- Docs must include compliance-oriented explanation and examples.

### the agent's Discretion
- Exact schema evolution strategy and migration path.
- Hashing algorithm selection and verification endpoint/CLI placement.
- API response shape and pagination strategy.
- Test granularity and benchmark scope.

</decisions>

<canonical_refs>
## Canonical References

### Existing code touchpoints
- `backend/src/db/decision_trace.rs` - current decision trace storage and repository logic.
- `backend/src/routers/` - API route registration and handlers for new evidence endpoint.
- `backend/src/services/` - integrity verification, snapshot serialization, and orchestration logic.
- `backend/src/models/` - model structs for trace/evidence payloads.

### Documentation touchpoints
- `docs/ARCHITECTURE.md` - baseline architecture narrative for synchronization.
- `docs/` - location for architecture addenda (evidence graph and snapshot format).

### Source issue
- `https://github.com/Colin4k1024/Aetheris-MemOS/issues/74`

</canonical_refs>

<specifics>
## Specific Ideas

- Keep implementation incremental: schema/model first, then API, then docs and validation.
- Ensure docs describe both guarantees and limitations to avoid over-claims.
- Add deterministic verification criteria for hash-chain correctness.

</specifics>

<deferred>
## Deferred Ideas

- Cross-workflow signaling and multi-agent orchestration enhancements (covered by later issues/phases).
- Full policy/governance automation for enterprise compliance.

</deferred>

---

*Phase: 01-evidence-graph-decision-snapshots-completeness*
*Context gathered: 2026-03-26 via issue-driven planning bootstrap*

