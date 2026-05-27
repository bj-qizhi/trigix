# Scheduled Triggers

## Status

done

## Category

enhancement

## What to build

Allow any workflow to run on a repeating schedule by setting `interval_secs` on its Trigger node config. When the workflow is published, the schedule is activated. A background task fires executions automatically at the configured interval.

## Acceptance criteria

- [x] `services/platform-rs/src/scheduler.rs` (NEW): `ScheduleEntry`, `ScheduleStore`, `ScheduleSummary`.
- [x] `ScheduleStore`: `register`, `unregister`, `list(tenant_id)`, `take_due()` (atomically returns due entries and bumps their `next_run_at`).
- [x] 3 unit tests in `scheduler.rs`: register+list, unregister, take_due.
- [x] `AppState` gains `schedule_store: Arc<ScheduleStore>`.
- [x] `http.rs` refactored: `build_router(AppState)` extracted; `router()` builds `default_app_state()`, spawns runner, calls `build_router`.
- [x] `spawn_schedule_runner(AppState)` — tokio background task: polls every 15 s, calls `take_due()`, fetches graph, resolves credentials, calls `execution_service.start()` with `input_json: "{}"`.
- [x] `publish_workflow_version` handler: after publish, calls `extract_trigger_interval()` to check trigger node config; if `interval_secs >= 60`, registers schedule.
- [x] `archive_workflow` handler: calls `schedule_store.unregister(latest_version_id)`.
- [x] `extract_trigger_interval(&WorkflowGraph) -> Option<u64>` helper: finds Trigger node, reads `interval_secs` from config, enforces minimum 60 s.
- [x] `GET /v1/schedules?tenant_id=...` → `Vec<ScheduleSummary>` (workflow_id, version_id, interval_secs, secs_until_next_run).
- [x] 2 new HTTP tests: `publishing_with_schedule_trigger_registers_schedule`, `archiving_workflow_removes_schedule`.
- [x] `ScheduleSummary` type in TypeScript.
- [x] `listSchedules(tenantId)` in `api/client.ts`.
- [x] Trigger node config panel: "Auto-run Interval" dropdown (None, 1 min, 5 min, 1 hr, 1 day); hint text when interval > 0.
- [x] `WorkflowList` table gains "Schedule" column showing ⏱ every Xh/m/d for active schedules; loads schedules alongside workflows on mount.
- [x] 73 Rust tests (23 executor + 46 platform + 4 workflow-core), 0 TypeScript errors.

## How scheduling works

1. User sets "Auto-run Interval" on the Trigger node config (e.g. "Every hour").
2. User saves and publishes the workflow version.
3. The `publish_workflow_version` handler detects `interval_secs` on the trigger node and calls `ScheduleStore::register`.
4. A background tokio task in `router()` polls `ScheduleStore::take_due()` every 15 seconds.
5. For each due entry: get the workflow graph, resolve credentials, call `ExecutionService::start` with `input_json: "{}"`.
6. Archiving a workflow unregisters its schedule.
7. Publishing a new version with a different interval replaces the previous schedule.

## Limitations

- In-memory only: schedules are lost on server restart.
- Minimum interval: 60 seconds.
- No per-tenant rate limiting or quota enforcement.
- Webhook triggers and schedule triggers are independent — both can be configured simultaneously.
