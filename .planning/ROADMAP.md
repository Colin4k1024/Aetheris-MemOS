# Roadmap: Aetheris MemOS

## Overview

This roadmap aligns implementation and documentation with the 2026 architecture direction. It converts high-level architecture issues into executable phases with verifiable outcomes.

## Phases

- [x] **Phase 1: Evidence Graph & Decision Snapshots Completeness** - Close the gap between architecture claims and implementation for auditable decision traces.
- [ ] **Phase 2: Security Hardening for MCP and Multi-Tenant Runtime** - Build stronger supply-chain, isolation, and runtime safeguards.
- [ ] **Phase 3: MemOS Deep Fusion and Self-Healing Runtime** - Deliver memory graph fusion and autonomous resilience capabilities.

## Phase Details

### Phase 1: Evidence Graph & Decision Snapshots Completeness
**Goal**: Deliver a verifiable evidence graph model, hash-chain integrity checks, snapshot serialization guarantees, and audit-ready API and docs for decision traceability.
**Depends on**: Nothing (first phase)
**Requirements**: [EVID-01, EVID-02, EVID-03, EVID-04, EVID-05, COMP-01]
**Success Criteria** (what must be TRUE):
  1. Users can retrieve workflow evidence graph records through a stable API endpoint.
  2. Decision trace records include tamper-evident hash-chain fields and verification logic.
  3. Snapshot payloads are serializable and sufficiently complete for offline audit workflows.
  4. Architecture docs explicitly describe data model, integrity guarantees, and EU AI Act reporting usage.
**Plans**: 3 plans

Plans:
- [x] 01-01-PLAN.md — Define evidence graph contracts, append-only schema, and Wave 0 tests.
- [x] 01-02-PLAN.md — Implement append-only evidence persistence and hash-chain verification.
- [x] 01-03-PLAN.md — Expose live evidence API, deterministic export, and compliance docs.

### Phase 2: Security Hardening for MCP and Multi-Tenant Runtime
**Goal**: Reduce runtime and supply-chain risk for MCP integrations and tenant-isolation boundaries.
**Depends on**: Phase 1
**Requirements**: [SEC-01, SEC-02, SEC-03]
**Success Criteria** (what must be TRUE):
  1. Runtime trust boundaries and signing rules are enforceable and documented.
  2. Input validation and sandbox controls are testable and enabled on critical paths.
  3. Multi-tenant isolation failure modes have explicit mitigations and tests.
**Plans**: 4 plans (Wave 0: Foundation, Wave 1: MCP Signing + Input Validation, Wave 2: Multi-Tenant Isolation)

Plans:
- [x] 02-01-PLAN.md — Auth foundation: JWT cookie security, route-level auth middleware, tenant context propagation (Wave 0)
- [x] 02-02-PLAN.md — MCP component signing verification with SHA-256 hash validation (Wave 1)
- [x] 02-03-PLAN.md — Input validation layer with schema-based validation and injection detection (Wave 1)
- [ ] 02-04-PLAN.md — Multi-tenant isolation enforcement with tenant-scoped repositories and monitoring hooks (Wave 2)

### Phase 3: MemOS Deep Fusion and Self-Healing Runtime
**Goal**: Implement global memory graph fusion and resilient recovery primitives with measurable operational gains.
**Depends on**: Phase 2
**Requirements**: [MEM-01, MEM-02, MEM-03]
**Success Criteria** (what must be TRUE):
  1. Cross-memory graph fusion is observable through runtime flows and APIs.
  2. Semantic evolution and decay strategies produce measurable retrieval-quality improvements.
  3. Self-healing workflows can detect and recover from selected fault classes.
**Plans**: TBD

Plans:
- [ ] TBD

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Evidence Graph & Decision Snapshots Completeness | 3/3 | Complete | 01-01, 01-02, 01-03 |
| 2. Security Hardening for MCP and Multi-Tenant Runtime | 3/4 | In Progress|  |
| 3. MemOS Deep Fusion and Self-Healing Runtime | 0/0 | Not started | - |
