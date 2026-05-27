# Cancel Execution

## Status

done

## Category

enhancement

## What to build

Add the ability to cancel a running or waiting_approval execution. Cancellation is "soft" — the status is updated immediately to `cancelled`; any in-progress node completes normally but the record won't be overwritten when the task finishes.

## Acceptance criteria

- [x] `cancel(tenant_id, execution_id)` added to `ExecutionStore` trait.
- [x] `MemoryExecutionStore::cancel` — sets status to `Cancelled` + `finished_at = Some(unix_now())` only if status is `Running` or `WaitingApproval`; idempotent on already-terminal records.
- [x] `MemoryExecutionStore::complete` and `fail` guard against overwriting a `Cancelled` status (return early if already cancelled).
- [x] `PostgresExecutionStore::cancel` — UPDATE WHERE status IN ('running', 'waiting_approval').
- [x] `PlatformExecutionStore` dispatch arm for `cancel`.
- [x] `PlatformExecutorClient::Noop(NoopExecutorClient)` variant added (needed for tests that require an execution to stay Running).
- [x] `ExecutionService::cancel(tenant_id, execution_id)`.
- [x] `action::EXECUTION_CANCELLED = "execution.cancelled"` audit constant.
- [x] `POST /v1/executions/:id/cancel` route + `cancel_execution` handler (body: `{ tenant_id }`); emits audit event.
- [x] Unit test `cancel_execution_sets_cancelled_status` in execution.rs.
- [x] Integration test `cancel_execution_over_http` in http.rs (uses noop executor to keep execution Running).
- [x] `api.cancelExecution(tenantId, executionId)` in `client.ts`.
- [x] `ExecutionDetailPage`: `cancelling` state, `handleCancel` async handler, "✕ Cancel" button (btn-danger) shown only when `isLive`.
- [x] 89 Rust tests (30 executor + 55 platform + 4 workflow-core), 0 TypeScript errors.
