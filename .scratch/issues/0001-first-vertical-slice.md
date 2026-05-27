# First Workflow Execution Vertical Slice

## Status

done

## Category

enhancement

## What to build

Build a minimal end-to-end Workflow Execution path that proves the service boundaries work together: the Platform accepts a Workflow run request, records an Execution, calls the Executor, and exposes health/status endpoints for local verification.

## Acceptance criteria

- [x] Platform service has a public API shape for starting an Execution.
- [x] Executor has a minimal execution state model for a single-node Workflow.
- [x] PostgreSQL schema supports Workflow, Workflow Version, Execution, and Node Execution records.
- [x] Local infrastructure can start PostgreSQL, Redis, and MinIO.
- [x] Documentation explains how to run and verify the slice.

## Notes

This is intentionally narrow. AI Runtime integration can be a following vertical slice after deterministic Execution works.

## Progress

- Rust Platform has a minimal Execution service using an in-memory Store.
- Rust Platform exposes the Execution service through Axum HTTP endpoints.
- Rust Platform accepts and validates shared Workflow Graph before starting an Execution.
- Rust Platform has an `ExecutorClient` boundary and calls it when starting an Execution.
- Rust Platform has a `PostgresExecutionStore` adapter behind `DATABASE_URL`.
- Local PostgreSQL has a dev seed for the first Tenant, Workflow, and Workflow Version.
- Executor has graph validation and topological scheduling tests.
- Executor has a runtime core that executes nodes through a `NodeExecutor` trait and returns per-node reports.
- `workflow-core` and `execution-core` share domain types between Platform and Executor.
- PostgreSQL schema exists for the later persistent Store adapter.
