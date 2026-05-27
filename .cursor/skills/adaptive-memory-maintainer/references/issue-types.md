# Issue Types

## Issue Type Prefixes

| Prefix | Type | Description |
|--------|------|-------------|
| `Feature` | New functionality | Adding new capabilities |
| `Bug` | Bug fix | Fixing existing issues |
| `Security` | Security hardening | Vulnerability fixes, defenses |
| `Documentation` | Docs | README, doc comments, guides |
| `Architecture` | Design | System design, refactoring |

## Priority Labels

| Label | Priority | When to Address |
|-------|----------|-----------------|
| `priority:P0` | Critical | Immediate attention, block everything |
| `priority:P1` | High | Next sprint cycle |
| `priority:P2` | Medium | When capacity allows |

## Area Labels

| Label | Area | Description |
|-------|------|-------------|
| `area:backend` | Backend | Rust/Axum changes |
| `area:frontend` | Frontend | React/Ant Design changes |
| `area:docs` | Documentation | Docs-only changes |
| `area:security` | Security | Security-related |
| `area:architecture` | Architecture | Design/system changes |

## Validation Requirements by Issue Type

### Feature Issues
1. `cargo fmt` / `npm run lint`
2. Unit tests
3. Integration tests (if applicable)
4. Build succeeds

### Bug Fixes
1. `cargo fmt` / `npm run lint`
2. Reproduce bug, verify fix
3. Run related tests
4. Build succeeds

### Security Issues
1. `cargo fmt` / `npm run lint`
2. `cargo clippy` (Rust)
3. Security-specific validation
4. Build succeeds

### Documentation Issues
1. Check for broken links
2. Verify docs render correctly
3. Markdown lint if applicable

### Architecture Issues
1. `cargo fmt` / `npm run lint`
2. `cargo clippy` (Rust)
3. Review design implications
4. Update related docs
