# Evidence Graph

The evidence graph is the backend's audit-oriented persistence model for adaptive memory decisions. It records workflow runs, ordered evidence nodes, and directed edges in append-only storage so the system can replay integrity checks after a decision is made.

## Implemented Audit API

The live retrieval endpoint is:

- `GET /api/v1/workflows/{id}/evidence`

The response returns:

- `run`: workflow metadata for the latest stored run.
- `nodes`: ordered evidence nodes for analyzer, predictor, weight-adjustment, and final-result stages.
- `edges`: ordered transitions between those nodes.
- `verification`: chain replay output, including `verified`, `root_hash`, and any violations.

## Stored Evidence Model

Each evidence node stores the locked fields required for offline review:

- `timestamp`
- `attempt_id`
- `llm_input_hash`
- `llm_output_hash`
- `tool_invocations`
- `context_snapshot`

Edges connect the ordered node sequence and carry their own hash-linked metadata so stage transitions remain inspectable.

## Integrity Guarantees

- Evidence storage is append-only once records are written.
- Verification replays canonical SHA-256 hashes over ordered node payloads and checks previous-hash linkage.
- The export contract is deterministic for offline review.

The exported snapshot carries these fields:

- `schema_version`
- `hash_algorithm`
- `workflow_id`
- `attempt_id`
- `root_hash`
- `chain_verified`
- `nodes`
- `edges`
- `exported_at`

`exported_at` is intentionally excluded from the canonical export hash so repeated exports of the same stored workflow can produce the same hashed body even when the export time changes.

## EU AI Act Reporting Example

An operator preparing a technical record for an EU AI Act review can export a workflow snapshot and map it into an audit narrative:

1. Identify the reviewed decision with `workflow_id` and `attempt_id`.
2. Show the sequence of reasoning stages through `nodes` and `edges`.
3. Reference `tool_invocations` and `context_snapshot` to explain what the system observed at each stage.
4. Use `root_hash` and `chain_verified` to show that the stored evidence remained internally consistent at export time.

This is useful for audit preparation, incident review, and internal controls. It is not a standalone legal compliance determination.

## Limitations

- The evidence graph is tamper-evident for stored records, not a guarantee that upstream or external tools told the truth.
- Verification does not validate business-policy correctness, retention policy, or human review adequacy.
- The endpoint returns the latest stored run for a workflow id; broader export workflows or UI exploration are separate concerns.
