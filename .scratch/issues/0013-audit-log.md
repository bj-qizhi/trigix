# Audit Log

## Status

done

## Category

enhancement

## What to build

Record every significant platform action to a per-tenant in-memory audit log and expose a read-only API endpoint and a UI page so operators can see what happened and when.

## Acceptance criteria

- [x] `services/platform-rs/src/audit.rs` (NEW): `AuditEvent`, `action` constants module, `MemoryAuditStore`, `PlatformAuditStore` alias.
- [x] 3 unit tests in `audit.rs`: records_and_lists_events, stores_detail_as_json_string, respects_limit.
- [x] `pub mod audit;` added to `lib.rs`.
- [x] `AppState` gains `audit_store: Arc<PlatformAuditStore>`.
- [x] `ScheduleStore::unregister` changed to return `bool` (true = entry removed).
- [x] Audit events emitted in handlers: `start_execution` (all 3 entry points + webhook), `create_workflow`, `update_workflow`, `archive_workflow`, `restore_workflow`, `publish_workflow_version`, `approve_execution`, `reject_execution`, `create_credential`, `delete_credential`, schedule register (inside `publish_workflow_version`), schedule remove (inside `archive_workflow`).
- [x] `GET /v1/audit-log?tenant_id=...&limit=N` → `Vec<AuditEvent>` (tenant-filtered, newest first, max 1000).
- [x] 1 HTTP integration test: `audit_log_records_execution_started`.
- [x] `AuditEvent` TypeScript type in `types/index.ts`.
- [x] `listAuditLog(tenantId, limit?)` in `api/client.ts`.
- [x] `apps/web/src/components/AuditLogPage.tsx` (NEW): table of events with timestamp, action, resource_type, resource_id (last 8 chars).
- [x] `App.tsx` updated: `{ name: 'audit' }` page type, `AuditLogPage` routing.
- [x] `WorkflowList.tsx` topbar: "Audit Log" button that navigates to audit page.
- [x] 77 Rust tests (23 executor + 50 platform + 4 workflow-core), 0 TypeScript errors.

## How audit log works

1. Every mutating handler records an event synchronously into `MemoryAuditStore` (an `Arc<RwLock<VecDeque<AuditEvent>>>`).
2. Events are stored newest-first, capped at 1000 per store (not per tenant).
3. `GET /v1/audit-log?tenant_id=T&limit=N` returns the last N events for tenant T.
4. The UI "Audit Log" button from WorkflowList opens a full-page table; clicking "Refresh" re-fetches.
5. `approve/reject` handlers use `body.tenant_id` (optional field); if absent, tenant_id is recorded as "".

## Limitations

- In-memory only: events are lost on server restart.
- No pagination cursor — only a `limit` cap.
- Tenant isolation is by filter, not by separate store.
