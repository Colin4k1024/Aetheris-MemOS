# 2026+ Roadmap

This document captures the forward-looking architecture roadmap for Aetheris MemOS beyond the current shipped baseline. It complements [ROADMAP.md](ROADMAP.md), which mixes completed milestones with planned work for earlier versions.

## Executive Summary

Aetheris MemOS is evolving from an adaptive-memory framework into a local-first, enterprise-oriented AI agent runtime. The 2026+ roadmap focuses on four themes:

- hardening execution safety and tenant boundaries
- making agent decisions observable and replayable
- enabling multi-agent collaboration with human approval points
- building a stronger ecosystem contract for enterprise deployment and compliance

This roadmap is intentionally aspirational. Unless a capability is explicitly called out as implemented in code, it should be treated as planned work rather than production-ready behavior.

## Phase 1: Foundation And Observability

Target window: 2026 Q1-Q2

Primary objectives:

- Worker execution contract hardening
- Multi-tenant isolation hardening
- MCP host security baseline
- Time-travel debugging and trace export

Key initiatives:

1. Worker execution contract: epoch fencing, stronger interruption semantics, and clearer recovery boundaries.
2. MCP host sandboxing: isolate untrusted tool execution and establish a minimal security gate before richer integrations ship.
3. Time-travel debugging: improve trace correlation and lay the groundwork for replayable execution snapshots.
4. Multi-tenant hardening: move from application-level conventions toward fail-closed isolation and auditable tenant scoping.

Representative issues:

- #61 Worker Execution Contract
- #62 MCP Host Sandboxing
- #63 Time-Travel Debugging
- #70 Multi-Tenant Isolation Hardening
- #72 Prompt Injection Defense
- #74 Evidence Graph And Decision Snapshots

Success indicators:

- no unaudited cross-tenant read path in core memory queries
- trace artifacts are correlated across workflow, decision, and memory operations
- MCP execution path has a defined trust and isolation boundary

## Phase 2: Multi-Agent Collaboration And HITL

Target window: 2026 Q3

Primary objectives:

- native human-in-the-loop orchestration
- inter-workflow signaling
- planner dry-run and virtual execution support
- stronger MCP governance and supply-chain controls

Key initiatives:

1. HITL orchestration primitives: approval pause/resume, waiting states, and operator review checkpoints.
2. Inter-workflow signaling bus: message and event exchange between workflows with causal traceability.
3. Planner virtual sandbox: dry-run execution for plan validation without side effects.
4. MCP supply-chain security: signing, schema validation, and compliance artifacts.

Representative issues:

- #64 Native HITL Orchestration
- #65 Inter-Workflow Signaling Bus
- #66 Planner Agent Virtual Execution Sandbox
- #71 MCP Supply Chain Security

Success indicators:

- approval-required workflows can pause and resume safely
- workflow-to-workflow signals are traceable and recoverable
- external tool registration has a clear trust policy and validation path

## Phase 3: MemOS Deep Fusion And Memory Graphs

Target window: 2026 Q4

Primary objectives:

- global memory graph and richer metadata topology
- event-driven semantic memory evolution
- autonomous but constrained fault recovery

Key initiatives:

1. Global Memory Graph: unify memory relationships across STM, LTM, KG, multimodal, and workflow artifacts.
2. Semantic memory evolution: lifecycle-aware updates, decay, and consolidation driven by observed events.
3. LLM-driven self-healing: bounded diagnosis and recovery flows with rollback safety.

Representative issues:

- #67 Global Memory Graph
- #68 Semantic Memory Evolution
- #69 LLM-Driven Self-Healing

Success indicators:

- memory changes are explainable through graph links and lifecycle events
- automated recovery remains auditable and reversible
- global metadata improves retrieval quality without weakening safety boundaries

## Phase 4: Enterprise Hardening And Ecosystem

Target window: 2027+

Primary objectives:

- open-core boundary refinement
- ecosystem packaging and compliance posture
- broader partner and community integration

Key initiatives:

1. Refine the open-core split around enterprise runtime, observability, and governance extensions.
2. Improve SDK, integration, and operator documentation for external adopters.
3. Standardize compliance artifacts for regulated deployment contexts.

Success indicators:

- enterprise-only capabilities are clearly delineated from core features
- external integrators can onboard without relying on architecture audit reports
- compliance and audit narratives map to concrete repository artifacts

## Milestones And KPIs

Phase 1 KPIs:

- tenant isolation regressions blocked by tests
- security-sensitive execution paths documented and reviewed
- trace coverage expanded for workflow and memory decisions

Phase 2 KPIs:

- pause/resume flows demonstrated end-to-end
- signed or policy-approved MCP registrations enforced at the host boundary
- inter-workflow signaling observable in logs and trace outputs

Phase 3 KPIs:

- memory graph links support audit and retrieval diagnostics
- lifecycle events drive measurable retention and quality improvements
- self-healing actions are bounded by approval, policy, or rollback constraints

Phase 4 KPIs:

- open-core documentation aligns with shipped modules
- ecosystem integrations publish stable extension contracts
- compliance-oriented reporting can be generated from repository artifacts

## Alignment With Existing Documents

- [CHANGELOG.md](../CHANGELOG.md): the changelog remains the record of shipped work. This roadmap describes intended evolution and must not be read as release history.
- [CONTRIBUTING.md](../CONTRIBUTING.md): contributors should link issues and PRs to the relevant 2026 phase when proposing roadmap work.
- [README.md](../README.md): overview materials should reference this roadmap when describing the long-term direction of the project.
- [ROADMAP.md](ROADMAP.md): use the existing roadmap for versioned baseline history and this document for the 2026+ strategic plan.

## Current Reality Check

Several roadmap items refer to infrastructure that is only partially present today. In particular, event-store completeness, evidence graphs, Wasm sandboxing, strong MCP trust controls, and full tenant isolation should be treated as active roadmap work rather than current guarantees.