# docs/update — v0.3 Design and Review Notes

This folder contains **v0.3 design and architecture review documents** produced from an open-source / tech-lead perspective. They are **not** the single source of truth; the canonical docs are in the parent [docs/](.) directory and the repo root (e.g. [ARCHITECTURE.md](../ARCHITECTURE.md), [ROADMAP.md](../ROADMAP.md), [CONTRIBUTING.md](../../CONTRIBUTING.md), [EXTENSION_GUIDE.md](../EXTENSION_GUIDE.md)).

## What’s in here

| File             | Role                                                                                                                                 |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **update.md**    | Gap review: Agent trait, Strategy plugin, Decision Trace, Storage, Observability, Docs — with “未达标/半达标” and a short 必改 list. |
| **all.md**       | File-level alignment: Router/Service/Agent boundaries and “怎么改” suggestions.                                                      |
| **artic.md**     | Draft v0.3 ARCHITECTURE (layer diagram, layer responsibilities).                                                                     |
| **plan1.md**     | Agent Runtime sketch (agents/, decision/, strategies/; Agent trait).                                                                 |
| **plan2.md**     | Strategy plugin design (strategies/weight/, trait, registry idea).                                                                   |
| **plan3.md**     | v0.3 checklist (core/, agents/, strategies/, Decision Trace table + API).                                                            |
| **plan4.md**     | Target backend directory layout.                                                                                                     |
| **v3_rodmap.md** | v0.3 Roadmap text (theme and bullets).                                                                                               |

## Status vs current codebase

Part of what these docs called “未达标” has **already been implemented** in the main codebase:

- **Done:** Agent-oriented core (`MemoryAgent` in `backend/src/services/agent.rs`; analyzer, predictor, scheduler implement it).
- **Done:** Strategy plugin system (`WeightStrategy` in `backend/src/services/weight_strategy.rs`; MarginalBenefit, LinearDecay, SynergyAware; weight_adjuster composes strategies).
- **Done:** Decision Trace API + UI (`POST /api/v1/memory/adaptive/trace` and Memory Decision Trace page) with persistence support.
- **Done:** Storage baseline alignment (PostgreSQL + Qdrant; Neo4j optional).
- **Done:** ARCHITECTURE, ROADMAP, USE_CASES, axum-migration-notes; CONTRIBUTING and EXTENSION_GUIDE.

**Still open (from these docs):**

- **Observability**: trace_id, decision span, metrics correlation.
- Router thinning (single service entrypoint), if desired.
- Optional directory refactor (e.g. top-level `core/`, `agents/`, `strategies/`) — current layout under `services/` is already agent- and strategy-ready.

For up-to-date status and roadmap, see [ROADMAP.md](../ROADMAP.md) and [ARCHITECTURE.md](../ARCHITECTURE.md).
