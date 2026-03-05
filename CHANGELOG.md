# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Root `LICENSE` file (MIT) for the project.
- `SECURITY.md` with supported versions and vulnerability reporting instructions.
- `CHANGELOG.md` for release history.

### Changed

- Documentation: replaced hardcoded password/secret examples with placeholders in `docs/SYSTEM_USAGE_GUIDE.md`, `docs/API_USAGE_GUIDE.md`, and `docs/FRONTEND_USAGE_GUIDE.md`.
- Backend: database initialization (`db::init`) now returns `Result<(), DbInitError>` instead of panicking; startup failure is handled in `main`.
- Backend: template rendering in routers uses proper error handling (`map_err` to `AppError`) instead of `unwrap()`.
- Backend: 404 handler template render uses `unwrap_or_else` fallback instead of `unwrap()`.
- Backend: log config builder methods `rolling()` and `format()` use safe defaults and logging instead of `panic!()` for invalid values.

## [0.1.1] - 2024

### Added

- Backend: adaptive memory scheduler, analyzer, predictor, monitor, weight adjuster.
- Frontend: Dashboard, Task Analysis, Memory Config, Performance, Resource Monitor, Weight History.
- REST API for memory selection, task characteristics, performance prediction, resource monitoring, weights.
- SQLite migrations and repository layer.
- JWT authentication and user management.
- Documentation: ARCHITECTURE, ROADMAP, API spec, algorithm design, extension guide.

## [0.1.0] - 2024

- Initial implementation: Rust (Salvo) backend and React (Ant Design Pro) frontend scaffold.

[Unreleased]: https://github.com/adaptive-memory-system/adaptive-memory-system/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/adaptive-memory-system/adaptive-memory-system/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/adaptive-memory-system/adaptive-memory-system/releases/tag/v0.1.0
