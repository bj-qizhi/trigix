# Execution Detail Page

## Status

done

## Category

enhancement

## What to build

Add a dedicated execution detail page reachable from RunsPage. Clicking a row in RunsPage now navigates to a full-page view of that execution's node-by-node results, input JSON, and timing information.

## Acceptance criteria

- [x] `apps/web/src/components/ExecutionDetailPage.tsx` created:
  - Fetches `ExecutionRecord` via `api.getExecution(TENANT_ID, executionId)`.
  - Auto-polls every 1.5s while status is `running` or `waiting_approval`.
  - Topbar: back button (returns to RunsPage or list), execution ID (last 12 chars), status badge, "live" pulse when active, "Open Workflow →" button.
  - Summary grid: Status, Started, Finished, Duration (actual seconds from started_at/finished_at), Workflow ID, Version ID.
  - Input section: pretty-printed JSON in a scrollable `<pre>`.
  - Node results section: one card per node showing node_id, node_type, status badge, and either error text (red) or pretty-printed output JSON (scrollable, max-height 160px).
- [x] `App.tsx` updated: `{ name: 'execution'; executionId: string; fromRuns?: boolean }` page type, `ExecutionDetailPage` import and routing; back button returns to `runs` page when `fromRuns` is true.
- [x] `RunsPage.tsx` updated: prop renamed from `onOpenWorkflow` to `onOpenExecution`; row click navigates to execution detail (not workflow editor).
- [x] No backend changes required — `ExecutionRecord.node_results` and `started_at`/`finished_at` already supply all needed data.
- [x] 87 Rust tests unchanged, 0 TypeScript errors.

## UX flow

WorkflowList → (Runs button) → RunsPage → (click row) → ExecutionDetailPage → (Open Workflow →) → WorkflowEditor
                                                                              → (← back) → RunsPage
