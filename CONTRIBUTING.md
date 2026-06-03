# Contributing to Trigix

Thanks for your interest in contributing! This document explains how to set up
your environment, the conventions we follow, and how to get a change merged.

## Code of Conduct

This project adheres to a [Code of Conduct](CODE_OF_CONDUCT.md). By participating,
you are expected to uphold it. Please report unacceptable behaviour to
managecode@gmail.com.

## Project layout

```text
apps/web                 React web console (Vite + React Flow)
services/platform-rs     Rust platform API (Axum, JWT, multi-tenant)
services/executor        Rust execution engine (DAG, parallel, retries)
services/ai-runtime      Python AI runtime (FastAPI)
crates/workflow-core     Shared WorkflowGraph model + DAG validation
crates/execution-core    Shared ExecutionStatus types
infra/postgres           PostgreSQL migrations
charts/trigix            Kubernetes Helm chart
```

## Local development

Prerequisites: a recent **stable Rust** toolchain, **Node.js 20+**, **Python 3.12**,
and **Docker** (for Postgres/Redis).

```bash
# 1. Start local infrastructure
docker compose up -d

# 2. Backend API
DATABASE_URL=postgres://trigix:trigix@localhost:35432/trigix \
PLATFORM_HTTP_ADDR=127.0.0.1:38080 \
cargo run -p trigix-platform

# 3. Execution engine
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 cargo run -p trigix-executor

# 4. Web console
cd apps/web && npm install && npm run dev   # http://localhost:3100
```

## Before you open a pull request

Run the same checks CI runs, locally:

```bash
# Rust
cargo fmt --all
cargo clippy --workspace --all-targets   # advisory — keep new code warning-free
cargo test --workspace

# Web
cd apps/web && npm ci && npm run build
```

CI (`.github/workflows/ci.yml`) enforces formatting, the full Rust test suite,
and the web production build (`tsc -b && vite build`) as hard gates. Clippy
runs in advisory mode for now — please keep new code warning-free so we can
promote it to a hard gate later.

## Commit and branch conventions

- Branch off `master`; keep branches focused on a single change.
- Use [Conventional Commits](https://www.conventionalcommits.org/) for messages,
  e.g. `feat: add CSV export node`, `fix: correct retry backoff`, `docs: ...`,
  `chore: ...`, `refactor: ...`, `test: ...`.
- **Keep commits small and reviewable.** Avoid mixing unrelated changes or
  bundling large generated files into feature commits.
- Reference the issue you are addressing in the PR description (`Closes #123`).

## Adding a node type

New workflow nodes touch a few places consistently:

1. Add the variant to `NodeType` in `crates/workflow-core/src/graph.rs`.
2. Implement `execute_<node>` in `services/executor`.
3. Add a config panel and palette entry in `apps/web/src`.
4. Add at least one executor test covering the happy path and one failure path.

## Reporting bugs and requesting features

Use the issue templates under **Issues → New issue**. Please include
reproduction steps, expected vs. actual behaviour, and your environment.

## Security

Do **not** open public issues for security vulnerabilities. See
[SECURITY.md](SECURITY.md) for private disclosure instructions.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).
