# Credential Store

## Status

done

## Category

enhancement

## What to build

Store named secrets (API keys, bearer tokens) and reference them in node configs with `{{credential.name}}`. Secrets are resolved at execution time before the graph reaches the executor — they never appear in API responses.

## Acceptance criteria

- [x] `services/platform-rs/src/credentials.rs` (NEW): `CredentialRecord`, `CredentialSummary`, `CredentialError`, `CredentialStore` trait, `MemoryCredentialStore`, `PlatformCredentialStore` type alias.
- [x] `resolve_credentials_in_json()` — recursively walks JSON, replaces `{{credential.name}}` patterns; unresolved names left intact and reported.
- [x] 5 unit tests in `credentials.rs`: CRUD + resolution (happy path + unknown pattern).
- [x] `AppState` gains `credential_store: Arc<PlatformCredentialStore>`.
- [x] All router builders updated (`router`, `router_with_store`, `router_with_store_and_executor`, `router_with_services`).
- [x] `start_execution`, `start_execution_from_workflow`, `start_execution_from_workflow_version`, `trigger_webhook` all call `resolve_graph_credentials()` before `execution_service.start()`.
- [x] `resolve_graph_credentials()` helper walks node configs and resolves credentials per tenant.
- [x] Routes: `GET /v1/credentials`, `POST /v1/credentials` (201), `DELETE /v1/credentials/:id` (204).
- [x] `From<CredentialError> for ApiError` (404 not found, 409 name taken, 500 store unavailable).
- [x] 2 new HTTP tests: `creates_lists_and_deletes_credentials_over_http`, `credential_reference_resolved_before_execution`.
- [x] `CredentialSummary { id, name }` type in TypeScript; value never serialized.
- [x] `listCredentials`, `createCredential`, `deleteCredential` in `api/client.ts`.
- [x] `CredentialsPage.tsx` (NEW): table of credentials, "Add Credential" modal (name + password field), delete per row.
- [x] `App.tsx` extended with `{ name: 'credentials' }` page.
- [x] `WorkflowList` gains `onCredentials` prop; topbar shows "🔑 Credentials" button.
- [x] `NodeConfigPanel` — HTTP node: "Auth Token (Bearer)" field with `{{credential.name}}` placeholder; `TemplateHint` updated to include credential syntax.
- [x] 68 Rust tests (23 executor + 41 platform + 4 workflow-core), 0 TypeScript errors.

## API reference

```
GET  /v1/credentials?tenant_id=...
  Response: [{ id, name }]   (value never returned)

POST /v1/credentials
  Body: { tenant_id, name, value }
  Response 201: { id, name }
  Response 409: name already taken

DELETE /v1/credentials/:id?tenant_id=...
  Response 204
```

## Credential interpolation

Pattern: `{{credential.NAME}}` in any string value anywhere in a node's `config` JSON.  
Resolution happens at the HTTP handler level before `ExecutionService.start()`. The graph stored in the execution record has resolved values.  
Unknown credential names are left as-is (execution proceeds without error; the AI runtime or HTTP caller will see the raw pattern).
