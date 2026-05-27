# Why Axum

Technical note for the HTTP layer migration and how it fits the open-source roadmap.

---

## Current state: Axum

The backend now uses **Axum** (Rust) for routing, extractors, middleware, and server startup.

- **Why Axum now:** Wider Rust ecosystem adoption, stronger community examples, and easier onboarding for contributors.
- **Compatibility policy:** Keep existing API paths and operational endpoints stable (`/api/*`, `/scalar`, `/api-doc/openapi.json`) while improving internals.

---

## Migration principles

| Principle                   | Decision                                                    |
| --------------------------- | ----------------------------------------------------------- |
| API stability               | Keep public endpoints and default port unchanged            |
| Implementation consistency  | Use typed extractors and unified `AppError -> IntoResponse` |
| Middleware parity           | Preserve CORS, JWT auth, rate-limit behavior                |
| Incremental maintainability | Keep service/domain logic framework-agnostic                |

---

## Roadmap alignment

The framework migration is complete; next work focuses on architecture capabilities (decision trace persistence, observability, runtime integrations), not framework switching.

See [ROADMAP.md](ROADMAP.md) and [ARCHITECTURE.md](ARCHITECTURE.md) for current planning.
