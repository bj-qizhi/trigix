# Trigix Browser Runtime

The Browser Runtime is the isolated Playwright execution service for governed browser Workflow nodes. The Rust Executor orchestrates tasks; this service owns Chromium processes, Browser Contexts, Pages, sessions, actions, artifacts, resource limits, and browser-specific recovery.

## Local development

Node.js 22 or newer and a compatible Chromium are required.

```bash
npm ci
npx playwright install chromium
BROWSER_RUNTIME_AUTH_TOKEN=replace-with-at-least-32-characters npm run dev
```

The service listens on `0.0.0.0:38100`. Health and readiness are public inside the deployment network. All `/v1/*` calls require `Authorization: Bearer ...` when an authentication token is configured and require `X-Trigix-Tenant-Id` for tenant ownership.

## Security boundary

All Chromium traffic goes through the runtime's local enforcing proxy. The proxy resolves each destination, connects to the validated address directly, blocks loopback, private, link-local, metadata, and non-HTTP destinations by default, and revalidates redirect destinations. Private deployments can add exact hosts or `*.suffix` rules with `BROWSER_ALLOWED_HOSTS`; the global private-network block remains enabled.

Browser Contexts are never shared between tenants. JavaScript evaluation is disabled by default. Logs redact authorization, cookies, secrets, input values, and uploaded payloads. Artifact binaries use the filesystem for single-instance development or S3-compatible storage for distributed deployment.

## Task example

```bash
curl -sS http://localhost:38100/v1/tasks \
  -H 'Authorization: Bearer replace-with-at-least-32-characters' \
  -H 'X-Trigix-Tenant-Id: tenant-1' \
  -H 'Content-Type: application/json' \
  -d '{"tenant_id":"tenant-1","actions":[{"type":"navigate","params":{"url":"https://example.com"}},{"type":"extract","params":{"selector":"title","mode":"text"}},{"type":"screenshot","params":{"full_page":true}}]}'
```

Poll `GET /v1/tasks/{task_id}` with the same headers. `DELETE` cancels an active task. Sessions use `POST`, `GET`, and `DELETE /v1/sessions`; artifacts use `GET /v1/artifacts/{artifact_id}`.

## Configuration

| Variable | Default | Purpose |
|---|---:|---|
| `BROWSER_RUNTIME_AUTH_TOKEN` | none | Required service credential in production; minimum 32 characters |
| `BROWSER_POOL_SIZE` | `3` | Chromium process count |
| `BROWSER_MAX_CONTEXTS_PER_BROWSER` | `10` | Context capacity per process |
| `BROWSER_MAX_PAGES_PER_CONTEXT` | `3` | Page limit per session |
| `BROWSER_TASK_TIMEOUT_MS` | `60000` | Maximum task duration |
| `BROWSER_ACTION_TIMEOUT_MS` | `10000` | Default action duration |
| `BROWSER_IDLE_SESSION_TIMEOUT_MS` | `300000` | Idle session expiry |
| `BROWSER_TENANT_MAX_RUNNING_TASKS` | `10` | Per-Tenant running/queued task quota |
| `BROWSER_TENANT_MAX_SESSIONS` | `10` | Per-Tenant active session quota |
| `BROWSER_BLOCK_PRIVATE_NETWORK` | `true` | Deny private/local/metadata networks |
| `BROWSER_ALLOWED_HOSTS` | empty | Explicit comma-separated private-host exceptions |
| `BROWSER_ENABLE_EVALUATE` | `false` | Permit the high-risk evaluate action |
| `BROWSER_ARTIFACT_PROVIDER` | `local` | `local` or `s3` storage |
| `BROWSER_ARTIFACT_DIR` | `/data/browser` | Local binary and metadata root |
| `BROWSER_ARTIFACT_BUCKET` | none | Required S3-compatible bucket in S3 mode |
| `REDIS_URL` | none | Publish lifecycle events to `browser.events` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | none | OTLP HTTP collector base URL |

## Validation

```bash
npm run lint
npm test
npm run build
npm run test:e2e
```

The Browser Agent exposes `browser_start`, the policy-approved action tools, and `browser_close` when an Agent node enables the `browser` tool. It requires an authenticated Tenant supplied by Executor plus explicit `browser_allowed_hosts`, `browser_allowed_actions`, `browser_max_steps`, and `browser_max_duration_seconds` limits.
