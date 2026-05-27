# Human Approval Gate

## Status

done

## Category

enhancement

## What to build

Allow workflows to pause mid-execution at an `approval` node and wait for a human to approve or reject before continuing. This is the core enterprise "human-in-the-loop" feature.

## Acceptance criteria

- [x] `approval` added to `NodeType` in `workflow-core`.
- [x] `ApprovalGate` in `services/executor/src/approval.rs`: `register(execution_id)` → oneshot receiver; `resolve(execution_id, approved)` → sends signal; `is_waiting(execution_id)` → bool.
- [x] `DispatchingNodeExecutor` handles `NodeType::Approval` before the retry/timeout loop: awaits the gate, returns `{"approved":true}` on approve or `Failed("Rejected")` on reject.
- [x] `InlineExecutorClient` wraps `ApprovalAwareEchoExecutor` which handles approval nodes via the same gate; all other nodes echo. Existing echo-based platform tests unaffected.
- [x] `AppState` holds `Arc<ApprovalGate>` shared with `PlatformExecutorClient::inline_with_gate`.
- [x] `GET /v1/executions/{id}`: overrides status to `"waiting_approval"` if store says `"running"` but gate is waiting.
- [x] `POST /v1/executions/{id}/approve` → signals gate with `true`, returns `{"ok": true}`.
- [x] `POST /v1/executions/{id}/reject` → signals gate with `false`, returns `{"ok": true}`.
- [x] Unknown execution in approve/reject returns 404.
- [x] Frontend: `approval` node in palette (teal/cyan, ✋ icon).
- [x] Frontend: `ApprovalConfig` panel explains the wait-and-resume mechanic.
- [x] Frontend: `ExecutionPanel` shows Approve/Reject buttons when `status === 'waiting_approval'`.
- [x] `waiting_approval` badge (cyan) and dot (pulsing cyan) added to CSS.
- [x] 3 new platform tests: approve flow, reject flow, 404 on non-pending.
- [x] 59 Rust tests passing, TypeScript zero errors.

## API reference

```
GET  /v1/executions/{id}?tenant_id=...
  Response: status "waiting_approval" when paused at approval node

POST /v1/executions/{id}/approve
  Body: {} (tenant_id optional)
  Response: {"ok": true}

POST /v1/executions/{id}/reject
  Body: {}
  Response: {"ok": true}
```

## Notes

- Approval only works in inline executor mode. The external HTTP executor path does not share the gate (no cross-service signalling yet — a future slice).
- The gate is per-process and in-memory; server restart cancels pending approvals (receiver drop → "Approval gate was closed").
