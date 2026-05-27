# Execution Retry

## Status

done

## Category

enhancement

## What to build

Allow retrying a failed or cancelled execution from the detail page. Retry creates a new execution with the same workflow, version, graph, and input JSON.

## Acceptance criteria

- [x] `action::EXECUTION_RETRIED = "execution.retried"` audit constant.
- [x] `POST /v1/executions/:id/retry` route + `retry_execution` handler:
  - Fetches original execution by ID + tenant_id.
  - Starts a new execution via `ExecutionService::start()` with same `workflow_id`, `workflow_version_id`, `graph`, `input_json`.
  - Emits `EXECUTION_RETRIED` audit event with `{ "retried_from": original_id }` as detail.
  - Returns `201 Created` + new `ExecutionRecord`.
- [x] `RetryExecutionBody { tenant_id: String }` struct.
- [x] Integration test `retry_execution_creates_new_execution`: verifies new execution has different ID, same workflow_id and input_json, status is "running".
- [x] `api.retryExecution(tenantId, executionId): Promise<ExecutionRecord>` in `client.ts`.
- [x] `ExecutionDetailPage`:
  - `onRetry: (newExecutionId: string) => void` prop.
  - `retrying` state + `handleRetry` async handler.
  - "↺ Retry" button (btn-primary) shown when status is `failed` or `cancelled`.
  - On success: calls `onRetry(newExec.id)` which navigates to the new execution's detail page.
- [x] `App.tsx` wires `onRetry`: navigates to `{ name: 'execution', executionId: newId }` preserving `fromRuns` flag.
- [x] 92 Rust tests (32 executor + 56 platform + 4 workflow-core), 0 TypeScript errors.

## UX flow

ExecutionDetailPage (failed/cancelled) → click "↺ Retry" → POST /retry → navigate to new ExecutionDetailPage (running) → auto-polls until complete
