# Async Execution + Polling + History

## Status

done

## Category

enhancement

## What to build

Make workflow execution non-blocking: the API returns immediately with `running` status, execution completes in a background task, and the Web Console polls for completion. Add execution history to the editor sidebar.

## Acceptance criteria

- [x] `ExecutionService::start()` returns the record immediately with `status: running`.
- [x] A `tokio::spawn` background task drives the executor to completion and calls `store.complete()`.
- [x] Executor failures call `store.fail()` to mark the execution as `failed`.
- [x] `ExecutionStore` gains a `fail()` method on all implementations (Memory, Postgres, PlatformEnum).
- [x] `ExecutionStore` trait methods return `impl Future + Send` for tokio::spawn compatibility.
- [x] `GET /v1/executions?workflow_id=` filter supported for workflow-scoped history.
- [x] All 46 Rust tests pass (HTTP tests updated to expect `running` on start response).
- [x] Frontend: `getExecution` and `listExecutions` added to API client.
- [x] Frontend: `ExecutionPanel` shows a spinner/running badge while polling, auto-updates on completion.
- [x] Frontend: `WorkflowEditor` polls every 1s while execution status is `running`.
- [x] Frontend: Execution history panel in the right sidebar (shows when no node selected).
- [x] TypeScript zero errors, Vite production build successful.

## Notes

- `#[allow(async_fn_in_trait)]` removed from `ExecutionStore` and `ExecutorClient` since we now use explicit `impl Future + Send` return types.
- History panel lists up to 100 recent executions for the current workflow; clicking loads details into the bottom panel.
