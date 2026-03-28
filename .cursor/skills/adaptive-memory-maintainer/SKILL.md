---
name: adaptive-memory-maintainer
description: Maintain the Adaptive Memory System with an issue-driven GitHub workflow. Use when working in the adaptive-memory-system repository and needs to pick up a GitHub issue, implement code changes, run validation (cargo fmt, clippy, cargo test for Rust; npm lint, tsc for frontend), create commits, push changes, close the completed issue, and continue to the next iteration.
---

# Adaptive Memory System Maintainer

## Overview

Run the Adaptive Memory System as a tight maintenance loop: select the active GitHub issue, implement only the issue-sized change, validate the workspace, create commits with meaningful messages, push changes, close the completed issue, and then continue to the next issue.

This is a monorepo with:
- **Backend**: Rust (Axum) in `backend/`
- **Frontend**: React (Ant Design Pro) in `frontend/ant-design-pro-template/`

## Workflow

### 1. Sync Issue Context

- Use `gh issue list --state open --limit 20` to find candidate issues.
- Issues use labels for prioritization: `priority:P0` (critical), `priority:P1` (high), `priority:P2` (medium)
- Issue titles follow format: `[Type] Short Description` where Type is `Feature`, `Bug`, `Security`, `Documentation`, `Architecture`
- Use `gh issue view <number>` to read the full issue details. Treat the issue body, labels, and comments as the source of truth for scope.
- Check branch state with `git status --short --branch` before changing anything.
- Verify GitHub access with `gh auth status` before using `gh issue` commands.

### 2. Plan the Change

- Restate the issue in concrete engineering terms before editing code.
- Identify which part of the codebase the issue affects:
  - Rust backend → `backend/` directory
  - React frontend → `frontend/ant-design-pro-template/` directory
- Read only the modules touched by the issue. Use `rg` (ripgrep) to find symbols and call sites quickly.
- Create an issue-specific branch such as `codex/issue-<number>-<slug>` when a new branch is needed.
- Check `docs/` for any existing documentation on the feature area.

### 3. Implement Narrowly

- Change only what the issue requires. Avoid opportunistic refactors unless they unblock the issue directly.
- For Rust changes:
  - Follow the existing patterns in `backend/src/`
  - Use SQLx macros with compile-time checks
  - Use `AppError` from `backend/src/error.rs` for structured error responses
- For Frontend changes:
  - Follow Umi 4 + Ant Design Pro 6.0 patterns
  - Use the existing API service files in `src/services/`
- Preserve unrelated user changes already present in the worktree.

### 4. Validate Changes

Run the appropriate validation commands based on what was changed:

**For Rust (backend) changes:**
```bash
cd backend
cargo fmt --all
cargo clippy --all
cargo test
```

**For Frontend (frontend/ant-design-pro-template) changes:**
```bash
cd frontend/ant-design-pro-template
npm run lint
npx tsc --noEmit
npm test
```

**For both:**
Run both validation sequences above.

Start with formatting checks, then run tests.

### 5. Create and Push Commit

- Create a meaningful commit message following the format:
  ```
  <type>(<scope>): <short description>

  [Optional] Longer explanation if needed

  Fixes #<issue-number>
  ```
  Where type is: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`
  Where scope is: `backend`, `frontend`, `api`, `config`, etc.

- Push the branch:
  ```bash
  git push -u origin HEAD
  ```

### 6. Close the Issue and Loop

- Leave a short issue comment summarizing what was done.
- Close the issue using `gh issue close <number>`
- Return to `gh issue list --state open` and continue with the next issue.

## Operating Rules

- Do not close an issue if validation fails.
- Do not make unrelated changes or opportunistic refactors.
- Always run validation before claiming work is complete.
- Surface blocked steps immediately when the workflow needs clarification or user approval.
- Prefer narrow commands while iterating: `rg`, targeted tests, file-targeted reads.
- Keep one explicit issue status visible while working: `in progress`.

## References

- Read [project-map.md](references/project-map.md) to understand the codebase structure.
- Read [issue-types.md](references/issue-types.md) for understanding different issue types and their validation requirements.
