# Latest Test Reports

> Updated: 2026-07-02  
> Focus: LangChain SDK integration, persistent agent memory, backend memory operation evidence

This page summarizes the newest validation evidence for Aetheris MemOS. It is optimized for the GitHub Pages documentation site; the full reports and raw backend logs remain linked below for auditability.

## Validation Snapshot

| Area | Status | Evidence |
|------|--------|----------|
| LangChain Tool integration | Validated | `AdaptiveMemoryTool` supports remember, search, and forget actions; split tools expose memory_store, memory_search, and memory_forget |
| LangChain Retriever integration | Validated | `AdaptiveMemoryRetriever` returns LangChain `Document` objects with memory metadata |
| LangChain ChatMessageHistory integration | Validated | Conversation turns persist through Aetheris MemOS STM APIs |
| Cross-session memory recall | Validated | Real 10-day developer journey demo recalls project facts across independent sessions |
| Backend operation trace | Captured | STM writes, hybrid search, embedding generation, Qdrant search, and SQLite lookup logs |

## Reliability Snapshot

> Updated: 2026-07-17 (P1 grounding — PR-1 through PR-6 merged)

| Area | Status | Evidence |
|------|--------|----------|
| Memory storage enterprise reliability review | Completed | [Memory Storage Reliability PRD](artifacts/2026-07-06-memory-storage-reliability/prd.md) and [Architecture Note](architecture/memory-storage-reliability.md) |
| Tenant isolation hardening | Implemented | [ADR-0001](adr/ADR-0001-memory-storage-tenant-isolation.md) — schema-level RLS on LTM/STM/KG/MM tables; `tenant_scope` GUC executor; tenant-scoped search paths |
| LTM/Qdrant consistency hardening | Implemented (outbox) | [ADR-0002](adr/ADR-0002-memory-vector-outbox-reconciliation.md) — DB+outbox single TX on PG; async worker with idempotent replay; reconciliation scanner still pending |
| Governance middleware | Implemented (scaffold) | [Governance Plan](artifacts/2026-07-16-enterprise-productionization/p1-outbox-and-governance-plan.md) — `auth→rate_limit→governance→handler` chain; audit async writer; quota/RBAC hooks exist but not yet enforcing |
| Operational readiness gate | Planned | [ADR-0003](adr/ADR-0003-memory-storage-operational-readiness.md) defines backup/restore, HA, alerting, and rollback evidence requirements |
| Reliability test plan | Planned | [Memory Storage Reliability Test Plan](artifacts/2026-07-06-memory-storage-reliability/test-plan.md) lists required schema, negative, transaction, outbox, reconciliation, and drill tests |
| Reliability execution team | Planned | [Team Execution Plan](artifacts/2026-07-06-memory-storage-reliability/team-execution-plan.md) consolidates tech-lead, architect, backend, QA, and DevOps execution responsibilities |
| Reliability release readiness | Blocked | [Deployment Context](artifacts/2026-07-06-memory-storage-reliability/deployment-context.md), [Release Plan](artifacts/2026-07-06-memory-storage-reliability/release-plan.md), and [Launch Acceptance](artifacts/2026-07-06-memory-storage-reliability/launch-acceptance.md) define release gates and current blockers |

Current reliability conclusion: P1 grounding has delivered schema-level tenant isolation (RLS), LTM→Qdrant durable outbox with async worker, and governance middleware scaffolding. Remaining blockers: quota/RBAC enforcement, reconciliation scanner, MCP signing/capability, backup/restore drills, and operational runbooks.

## Latest Highlights

### 1. Memory turns a stateless LangChain agent into a persistent assistant

The latest demo compares the same developer workflow across five independent sessions:

| Scenario | Without Memory | With Aetheris MemOS |
|----------|----------------|---------------------|
| Day 3 database debugging | Generic timeout advice | SQLAlchemy + PostgreSQL pool guidance with `pool_pre_ping=True` |
| Day 5 deployment planning | Generic blue/green or rolling guidance | Kubernetes rolling update YAML and FastAPI shutdown guidance |
| Day 8 testing strategy | Generic testing pyramid | `pytest` + `httpx.AsyncClient` + transaction rollback for FastAPI |
| Day 10 performance tuning | Generic indexing/caching advice | SQLAlchemy `joinedload()` / `selectinload()` N+1 remediation |

The practical difference is that the memory-enabled agent gives stack-aware answers in one exchange instead of asking users to repeat context.

### 2. Quantified productivity impact

The newest comparison report estimates:

| Metric | Without Memory | With Memory | Impact |
|--------|----------------|-------------|--------|
| Context questions asked by AI | 20+ | 0 | -100% |
| User repetition required | Every session | Never | -100% |
| Response specificity | Generic | Targeted | +300% |
| Time to actionable answer | 2-3 exchanges | 1 exchange | -60% |
| Cross-session continuity | Broken | Persistent | Major improvement |

Across a 30-day workflow with two sessions per day, the demo estimates roughly 3 hours saved by eliminating repeated context.

### 3. Backend memory path is backed by raw operation logs

The raw backend log captures real memory operations during the E2E flow:

- STM session creation and message append
- Hybrid search request handling
- Embedding service initialization
- Embedding cache miss and generation
- Qdrant vector search with tenant filtering
- SQLite lookup for returned memory IDs
- Search threshold filtering and final result count

These logs make the demo reproducible and auditable beyond screenshots or narrative claims.

## Reports

| Report | What it contains |
|--------|------------------|
| [LangChain Memory Demo Report](memory_demo_report_with_logs.md) | Full real-agent narrative, memory vs no-memory responses, implementation snippet |
| [Memory Comparison Report](memory_comparison_report.md) | Executive summary, 10-day journey, quantified productivity metrics, architecture notes |
| [Backend Demo Logs](demo_backend_logs.txt) | Raw backend traces from STM, hybrid search, embedding, Qdrant, and SQLite operations |
| [LangChain SDK PRD](artifacts/2026-07-02-langchain-framework-sdk/prd.md) | Scope, acceptance criteria, implemented interfaces, and validation checklist |

## Reproduce Locally

Run the LangChain integration test:

```bash
PYTHONPATH=sdks/python:sdks/python-langchain python e2e_langchain_agent_test.py
```

Run the full real-agent memory demonstration:

```bash
PYTHONPATH=sdks/python:sdks/python-langchain python examples/real_agent_memory_demo.py
```

Run the report generator:

```bash
PYTHONPATH=sdks/python:sdks/python-langchain python examples/memory_comparison_demo.py
```

The backend should be available at `http://localhost:8008`. For the full local LLM demonstration, the report used Qwen3.6-27B-4bit on local port `8000`, with PostgreSQL, Qdrant, and Ollama backing Aetheris MemOS.
