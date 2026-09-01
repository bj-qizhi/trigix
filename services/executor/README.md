# Executor

Rust service for reliable Workflow Execution.

Responsibilities:

- DAG validation
- Node scheduling
- Concurrent execution
- Timeout and cancellation
- Retry policy
- Event publishing
- Checkpointing
- Calling Python AI Runtime for Agent and RAG Nodes

## Current Slice

The Executor currently includes a small Rust runtime core and HTTP service boundary:

- `WorkflowGraph::validate` checks node and edge integrity.
- `scheduler::schedule` returns a topological execution order.
- `runtime::run_workflow` runs nodes through a `NodeExecutor` trait.
- `ExecutionReport` captures final Execution status and per-node reports.
- `POST /v1/executions:run` runs a Workflow Graph and returns an `ExecutionReport`.

The Platform can invoke this runtime inline, through the Executor HTTP API, or
through a Redis Streams worker. PostgreSQL stores Execution and Node Execution
state; Redis queue delivery is at-least-once, with stale-pending reclamation,
terminal-state idempotency, bounded recovery attempts, and a dead-letter stream.

Node runtime policy is resolved through the registry in
`src/executor/registry.rs`. It classifies local, external, Approval, and Wait
Nodes so dry-run and suspension behaviour have one source of truth. Handler
implementations remain grouped by domain under `src/executor/`.

Run:

```bash
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 cargo run -p trigix-executor
```

Suggested stack:

```text
Tokio
Axum
tonic
serde
sqlx
tracing
OpenTelemetry
```
