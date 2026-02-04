# Roadmap

This project evolves in **architecture-first iterations**. Each version focuses on clarity, extensibility, and agent-readiness.

See [why-salvo-vs-axum.md](why-salvo-vs-axum.md) for web framework choice and [ARCHITECTURE.md](ARCHITECTURE.md) for design.

---

## v0.2 — Stable Rule-Based Adaptive Memory (Completed)

- **Backend**: Rule-based scheduler, analyzer, predictor, monitor, weight adjuster; SQLite (default); REST API.
- **Frontend**: Dashboard, task analysis, memory config, performance, resource monitor, weight history.
- **Docs**: Algorithm design, API spec, system visualization.

---

## v0.3 — Extensible & Agent-Ready (In Progress)

**Theme:** Make the system composable, explainable, and open for extension.

### Done

- **Agent-oriented core** — `MemoryAgent` trait (observe → decide → act); Analyzer, Predictor, and Scheduler implement it. See [ARCHITECTURE.md](ARCHITECTURE.md).
- **Strategy plugin system** — `WeightStrategy` trait; built-in strategies (MarginalBenefit, LinearDecay, SynergyAware); weight adjuster composes strategies. See [EXTENSION_GUIDE.md](EXTENSION_GUIDE.md).
- **Decision trace (API + UI)** — `POST /api/v1/memory/adaptive/trace` and Memory Decision Trace page for step-by-step pipeline inspection (analyzer → predictor → weight adjustment → result). No persistence yet.
- **Storage adapter declaration** — SQLite as default; `db/adapters` namespace and docs state PostgreSQL/MySQL as planned.
- **Documentation** — ARCHITECTURE, ROADMAP, USE_CASES, why-salvo-vs-axum; CONTRIBUTING and EXTENSION_GUIDE.

### Planned

- **Decision trace persistence** — DB table and model for traces; optional save when calling adaptive or trace endpoint.
- **Observability** — trace_id, decision span, OpenTelemetry-compatible export; metrics correlation.
- **Repository adapter trait** — Abstract persistence behind traits; runtime adapter selection (optional).

---

## v0.4 (Planned)

- **Optional LLM integration** — Pluggable LLM-driven analyzer or predictor behind `MemoryAgent`.
- **Optional Axum backend adapter** — Salvo remains default; Axum as alternative (see [why-salvo-vs-axum.md](why-salvo-vs-axum.md)).

---

## Database adapters

- **SQLite** — Default for local and demo.
- **PostgreSQL / MySQL** — Planned for production; adapter abstraction in a future release.
