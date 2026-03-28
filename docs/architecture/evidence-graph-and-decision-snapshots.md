# Evidence Graph And Decision Snapshots

This document describes the intended architecture for Evidence Graphs and Decision Snapshots, and distinguishes it from what is currently implemented in the repository.

## Why This Matters

Evidence Graphs and Decision Snapshots are the backbone of explainability, auditability, and replay support for an enterprise AI agent runtime. They are especially important for:

- post-incident investigation
- regulated deployment and audit evidence
- debugging complex workflow decisions
- proving the causal chain behind tool calls and memory mutations

## Current Repository State

Today, the repository provides a baseline decision-trace persistence layer in [backend/src/db/decision_trace.rs](../../backend/src/db/decision_trace.rs):

- traces are stored as opaque JSON blobs
- lookup is available by `task_id` or by recent time range
- there is no native graph model, hash chain, or checkpoint reference schema in storage

This means the current implementation is useful for explainability at the trace level, but it does not yet satisfy the stronger Evidence Graph or Decision Snapshot claims implied by the 2026 architecture roadmap.

## Target Model

### Evidence Graph

The target Evidence Graph is a tamper-evident directed graph of workflow decisions.

Each node should represent a single decision point and include at least:

- `timestamp`
- `workflow_id`
- `attempt_id`
- `decision_id`
- `llm_input_hash`
- `llm_output_hash`
- `tool_invocations`
- `context_snapshot_ref`
- `previous_hash`
- `node_hash`

Each edge should encode a causal relationship, for example:

- planner step to worker execution
- model output to tool invocation
- tool invocation to memory mutation
- approval request to resumed execution

### Decision Snapshots

Decision Snapshots are point-in-time serialized captures of execution state. A full snapshot should be self-contained enough for offline analysis and bounded replay.

Expected snapshot contents:

- workflow state
- task context
- selected memory context or references
- tool invocation context
- policy and approval state
- runtime metadata needed for deterministic diagnosis

## Integrity Model

The intended integrity model is a hash chain across decision nodes:

1. every node stores a deterministic representation of the decision record
2. the node hash includes the previous node hash
3. verification can walk the chain and detect post-hoc mutation

This is not fully implemented today.

## Storage Responsibilities

Current baseline:

- Decision trace JSON persists in the relational store through [backend/src/db/decision_trace.rs](../../backend/src/db/decision_trace.rs)

Planned expansion:

- an event store or graph-aware persistence layer for decision nodes and edges
- checkpoint storage for snapshot blobs
- explicit references between decision nodes and snapshot records
- verification utilities in runtime or services code for chain integrity checks

## API Surface

Current baseline:

- decision-trace APIs exist for trace capture and inspection

Planned additions:

- workflow evidence query API such as `GET /api/v1/workflows/{id}/evidence`
- snapshot export endpoints for audit tooling
- integrity verification endpoints or offline verification tooling

## Documentation Boundaries

To avoid overstating implementation status, repository documentation should follow these rules:

- describe current decision-trace persistence as implemented baseline
- describe Evidence Graphs and Decision Snapshots as roadmap architecture unless code and storage schemas exist
- avoid implying tamper-evident guarantees before hash-chain verification is shipped

## Gap Summary

Implemented today:

- persisted decision traces
- retrieval by task and time range
- general explainability narrative in architecture docs

Not yet implemented end-to-end:

- graph-native evidence storage
- checkpoint-backed decision snapshots
- hash-chain integrity verification
- dedicated evidence APIs for workflow audit consumers

## Recommended Next Steps

1. extend decision trace storage with explicit node and edge records
2. define a stable snapshot schema and serialization contract
3. add hash-chain generation and verification utilities
4. expose evidence retrieval APIs and operator-facing documentation