# Why Salvo (and optional Axum later)

Technical choice for the HTTP layer and how it fits the open-source roadmap.

---

## Current choice: Salvo

The backend uses **Salvo 0.84** (Rust) for routing, extraction, OpenAPI, and middleware.

- **Reasons for Salvo:** Solid Rust async web framework; built-in OpenAPI support; straightforward handler and extractor model. The project started with Salvo and the codebase is consistent with it.
- **Trade-off:** Salvo has a smaller ecosystem than **Axum** (tutorials, examples, and third-party integrations are more abundant for Axum). For an open-source project, “why not Axum?” is a fair question.

---

## Ecosystem comparison (short)

| Aspect           | Salvo        | Axum              |
| ---------------- | ------------ | ------------------ |
| Community size   | Smaller      | De facto standard  |
| Tutorials/samples| Fewer        | Many               |
| Agent/AI integration examples | Rare | Growing            |
| Enterprise adoption visibility | Lower | Higher             |

The **logic and design** of this project (adaptive memory, agents, strategies) are framework-agnostic. The HTTP layer is a thin wrapper around services and DB.

---

## Roadmap: optional Axum adapter

We are **not** migrating away from Salvo by default. To improve open-source credibility and alignment with the Rust web ecosystem:

- **v0.4** may introduce an **optional Axum backend adapter**: same APIs and service layer, different router and server entrypoint. Salvo remains the default; Axum becomes an alternative for teams that standardize on it.
- This keeps the core value (adaptive memory, agent abstraction, strategies) independent of the web framework.

See [ROADMAP.md](ROADMAP.md) for version planning.
