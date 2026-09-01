# Browser Runtime Architecture and Operations

## Purpose and boundaries

Browser Runtime is Trigix's isolated web-automation execution plane. Workflow scheduling, retries, approvals, Tenant authentication, and graph state remain in Platform and Executor. Browser Runtime owns Chromium processes, Browser Contexts, Pages, sessions, actions, task state, cancellation, and browser-generated artifacts.

The integration flow is:

1. Platform binds the authenticated Tenant to the workflow execution request.
2. Executor resolves Browser node configuration and creates a Tenant-bound session or task.
3. Browser Runtime validates service authentication, Tenant consistency, quotas, action limits, and destination policy.
4. A bounded Browser Context executes the action list. State transitions and metrics are emitted during execution.
5. Executor polls the task until a terminal state and publishes a normalized `browser` output for downstream nodes.
6. Executor sends task cancellation when its node future is dropped or times out. Browser Close releases the session context explicitly; idle expiration provides a second cleanup boundary.

## Public contract

The service exposes:

- `GET /health`, `GET /healthz`: process health.
- `GET /ready`, `GET /readyz`: browser-pool readiness and capacity state.
- `GET /metrics`: Prometheus exposition.
- `POST /v1/tasks`, `GET /v1/tasks/:taskId`, `DELETE /v1/tasks/:taskId`: task lifecycle.
- `POST /v1/sessions`, `GET /v1/sessions/:sessionId`, `DELETE /v1/sessions/:sessionId`: session lifecycle.
- `GET /v1/artifacts/:artifactId` and `/metadata`: Tenant-bound artifact access.

End users do not receive the Browser Runtime service token. The authenticated Platform proxy at `GET /v1/browser/artifacts/:artifactId` derives the Tenant from JWT/API-key claims and forwards the request over the internal service boundary with `Cache-Control: private, no-store`.

Mutating and data endpoints require `Authorization: Bearer <service token>` and `x-trigix-tenant-id`. Production startup fails when the service token is absent or shorter than 32 characters.

Task states are `queued`, `running`, `completed`, `failed`, `timeout`, and `cancelled`. Terminal states are immutable. The supported action surface is navigate, click, input, wait, extract, screenshot, cookies, upload, download, PDF, network recording, HAR, trace, page management, and policy-gated evaluate.

Executor Browser nodes expose a common downstream shape:

```json
{
  "browser": {
    "session_id": "...",
    "task_id": "...",
    "result": {},
    "artifact_url": "/v1/browser/artifacts/ba_...",
    "duration_ms": 42,
    "url": "https://example.test/",
    "title": "Example"
  }
}
```

A complete importable graph is available at `docs/examples/browser-workflow.json`.

## Browser Agent tools

An Agent node can enable the `browser` tool family. AI Runtime then exposes `browser_start`, the configured subset of `browser_navigate`, `browser_click`, `browser_input`, `browser_wait`, `browser_extract`, and `browser_screenshot`, followed by `browser_close`. Executor supplies the authenticated Tenant and execution identifiers; workflow configuration cannot override that authority. The Agent configuration must bound `browser_allowed_hosts`, `browser_allowed_actions`, `browser_max_steps`, and `browser_max_duration_seconds`. Tool calls are serialized within the session, and normal Agent completion closes any still-open session. The system and runtime policies prohibit CAPTCHA solving, anti-bot evasion, fingerprint spoofing, and access-control bypass.

## Isolation and security

Each session owns one Playwright Browser Context and one or more bounded Pages. Contexts are never shared between Tenants. Ephemeral tasks receive a new context and close it after the terminal transition. Session lookup, task lookup, cancellation, and artifact retrieval all verify Tenant ownership and return no cross-Tenant metadata.

Chromium is launched through `SecureBrowserProxy`. The proxy validates both ordinary HTTP requests and HTTPS CONNECT tunnels. DNS results are checked before connection and the validated IP is used for the upstream socket, preventing DNS-rebinding between policy check and connection. Private, loopback, link-local, unspecified, multicast, and metadata addresses are denied. Operator allowlists are exact hostnames or explicit subdomain patterns; they are intended for controlled first-party test systems, not broad network exceptions.

The runtime image uses the Playwright browser image and a non-root user. Kubernetes supplies an in-memory `/dev/shm`. JavaScript evaluation is disabled by default. Logs redact authorization, cookies, form content, tokens, uploaded base64, and secrets.

## Capacity and failure behavior

Capacity equals `BROWSER_POOL_SIZE * BROWSER_MAX_CONTEXTS_PER_BROWSER`. A FIFO semaphore bounds active contexts, queue capacity bounds pending tasks, per-Tenant limits bound running tasks and active sessions, and each context bounds page count. Action, navigation, task, idle-session, artifact-size, download-size, and shutdown timeouts are independently configurable.

A browser disconnect marks the pool unready while a replacement launches. The affected task fails with a classified browser error, its context is discarded, and no state is reassigned across Tenants. On SIGTERM/SIGINT the service stops HTTP intake, cancels or drains work within the configured grace period, closes sessions, closes browsers, flushes event/telemetry clients, and exits.

## Artifacts and state

Screenshots, downloads, PDFs, HAR files, and traces are binary objects and never enter PostgreSQL. Local mode stores files under `/data/browser` with a persistent volume and is restricted to a single runtime replica. S3 mode stores objects under Tenant-scoped keys and is required for shared artifact access. API metadata contains type, content type, size, task/execution association, and creation time.

When `REDIS_URL` is configured, lifecycle events are appended to the `browser.events` Redis Stream for monitoring and downstream integrations. Browser process and session objects remain runtime-owned and are never serialized.

## Deployment

Docker Compose starts one runtime with Redis, a 1 GiB shared-memory segment, a persistent artifact volume, private-network blocking, and evaluate disabled. `BROWSER_RUNTIME_AUTH_TOKEN` is mandatory and shared with Platform/Executor through environment injection.

The Helm chart keeps Browser Runtime opt-in. Enable it with a protected token of at least 32 characters. Local artifacts use a ReadWriteOnce PVC and one replica. Enabling HPA requires S3 artifacts. The internal Service uses ClientIP affinity so one Executor or AI Runtime process keeps a session on its owning runtime replica; a replica failure invalidates those in-memory sessions and callers receive a classified failure rather than having state reassigned. Readiness uses `/readyz`, liveness uses `/healthz`, rolling updates allow no unavailable replica, and a PodDisruptionBudget preserves service availability.

## Verification

The runtime quality gate runs TypeScript compilation, ESLint with zero warnings, unit tests for URL policy/configuration/concurrency, production dependency audit, and Playwright Chromium integration tests. The integration suite verifies a complete session workflow, extraction, screenshot persistence, cross-Tenant artifact denial, metadata-address blocking, and in-flight cancellation. CI also builds the container, probes its health endpoint, renders the Helm permutation, and validates Kubernetes schemas.
