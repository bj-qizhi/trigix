# Webhook Trigger

## Status

done

## Category

enhancement

## What to build

Allow external systems to trigger workflow executions via a stable HTTP webhook URL, one per published workflow version.

## Acceptance criteria

- [x] `POST /v1/workflow-versions/{version_id}/webhook`: creates (or returns existing) webhook record with a UUID token. Idempotent — same version always gets the same token.
- [x] `POST /v1/webhooks/{token}`: accepts any JSON body, looks up the webhook record, starts an execution with the body as `input_json`. Returns `202 Accepted` with the execution record.
- [x] Unknown token returns 404.
- [x] `WebhookRecord` stored in `MemoryWebhookStore` (two maps: `by_token`, `by_version`).
- [x] `WebhookStore` trait uses explicit `fn ... -> impl Future + Send` for all methods.
- [x] `AppState` gains `webhook_store: Arc<PlatformWebhookStore>`.
- [x] `WebhookInfo { token, url }` added to `types/index.ts`.
- [x] `getWebhook(tenantId, versionId)` added to `api/client.ts`.
- [x] Trigger node config panel shows the webhook URL with a Copy button when a published version exists; shows a "Publish a version to get a webhook URL" hint otherwise.
- [x] `WorkflowEditor` fetches webhook URL on load (if version is published) and after publish; passes `webhookUrl` to `NodeConfigPanel`.
- [x] 1 new Rust test: create webhook, idempotency check, trigger, unknown-token 404.
- [x] 56 Rust tests passing, TypeScript zero errors.

## API reference

```
POST /v1/workflow-versions/{version_id}/webhook
  Body: { "tenant_id": "..." }
  Response: { "token": "...", "url": "/v1/webhooks/{token}" }

POST /v1/webhooks/{token}
  Body: any JSON
  Response: ExecutionRecord (202 Accepted)
```
