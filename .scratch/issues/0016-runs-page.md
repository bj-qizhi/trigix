# Execution Run History Page

## Status

done

## Category

enhancement

## What to build

Add a global run history page showing all executions across all workflows, with timestamps and status. Also add `started_at` / `finished_at` fields to `ExecutionRecord` and `ExecutionSummary` for time-aware display and sorting.

## Acceptance criteria

- [x] `started_at: u64` (unix seconds) and `finished_at: Option<u64>` added to `ExecutionRecord`.
- [x] `started_at: u64` added to `ExecutionSummary`.
- [x] `unix_now() -> u64` helper in `execution.rs` (SystemTime::now since UNIX_EPOCH as secs).
- [x] `MemoryExecutionStore::create` sets `started_at: unix_now(), finished_at: None`.
- [x] `MemoryExecutionStore::complete` sets `finished_at = Some(unix_now())`.
- [x] `MemoryExecutionStore::fail` sets `finished_at = Some(unix_now())`.
- [x] `MemoryExecutionStore::list` sorts by `started_at` descending (newest first).
- [x] `execution_summary()` copies `started_at` from the record.
- [x] Postgres `create` / `try_into_record` / `try_into_summary` have `started_at: unix_now()` placeholders (noted TODO for real PG schema).
- [x] Unit test `execution_record_has_started_at_and_finished_at` verifies timestamps are set correctly and summaries carry `started_at`.
- [x] `ExecutionRecord` and `ExecutionSummary` TypeScript interfaces updated with `started_at: number` and `finished_at?: number`.
- [x] `apps/web/src/components/RunsPage.tsx` — full-page table: Time (localeString), Workflow ID (last 12 chars), Status badge, Age (relative since started_at); click row opens workflow editor.
- [x] `App.tsx` updated: `{ name: 'runs' }` page type, `RunsPage` import, routing.
- [x] `WorkflowList.tsx` updated: `onRuns` prop, "Runs" button in topbar.
- [x] 84 Rust tests (27 executor + 53 platform + 4 workflow-core), 0 TypeScript errors.
