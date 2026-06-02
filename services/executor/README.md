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

This keeps the execution semantics testable before adding persistence, queues, async workers, or the gRPC transport.

Run:

```bash
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 cargo run -p velara-executor
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
