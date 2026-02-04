# Contributing

Thank you for your interest in contributing to the Adaptive Memory Management System. This document covers how to build, test, and submit changes, and where to extend the system.

## Documentation

- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)** — System design, layer diagram, and decision pipeline.
- **[ROADMAP.md](docs/ROADMAP.md)** — Version plan and what's done vs planned.
- **[USE_CASES.md](docs/USE_CASES.md)** — LLM agent memory, multi-modal, cost-aware routing.
- **[EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md)** — How to add new strategies and agents.

## Building and testing

### Backend (Rust)

```bash
cd backend
cargo build
cargo test
```

- **Rust**: 1.89+
- **Database**: SQLite (default); see [ROADMAP.md](docs/ROADMAP.md) for adapter plans.

### Frontend (React)

```bash
cd frontend/ant-design-pro-template
npm install
npm start
npm test
```

- **Node**: 20+

## Submitting changes

1. **Fork** the repository and create a branch from `main` or `dev`.
2. **Implement** your change; keep commits focused.
3. **Test** — run `cargo test` and frontend tests.
4. **Open a Pull Request** with a clear title and description. Reference any related issue.

## Extension points

The project is designed for extension without refactoring core orchestration:

- **Weight strategies** — Implement the `WeightStrategy` trait and add your strategy to the adjuster chain or use `DynamicWeightAdjuster::with_strategies`. See [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md).
- **Agents** — Implement the `MemoryAgent` trait for new or replacement analyzer/predictor/scheduler behavior. See [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md).
- **Decision trace** — The trace API and UI are in place; persistence and correlation are planned (see ROADMAP).

If you are unsure where a change belongs or how to add a new strategy/agent, open an issue or refer to the extension guide.

## Code and conduct

- Follow existing style and patterns in the codebase.
- Keep the router layer free of business logic; keep services thin and agent/strategy logic in the appropriate modules.
- This project is provided under the MIT License.
