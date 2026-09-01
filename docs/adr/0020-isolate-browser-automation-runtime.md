# ADR 0020: Isolate Browser Automation in a Dedicated Runtime

- Status: Accepted
- Date: 2026-09-01
- Decision owners: Workflow, Runtime, Security, and Platform Engineering

## Context

Workflow and Agent executions need stateful web automation, including navigation, interaction, extraction, screenshots, downloads, PDFs, network evidence, and multi-page sessions. Embedding Chromium in the Rust Executor would couple browser crashes and memory pressure to workflow scheduling, enlarge the Executor image, and make browser-specific scaling and sandbox policy difficult. Browser navigation also creates a high-risk server-side request surface because untrusted workflows can attempt to reach private networks or cloud metadata.

## Decision

Browser automation runs in the independent Node.js 22 and Playwright service under `services/browser-runtime`. The Rust Executor remains the authoritative workflow scheduler and calls the runtime through a versioned HTTP task/session contract. Browser nodes never launch Chromium inside the Executor.

Every request carries service authentication and an authenticated Tenant identifier. The runtime verifies the Tenant header against the request body, owns all Browser, Browser Context, Page, Task, Session, and Artifact lifecycles, and never reuses a Browser Context across Tenants. A bounded pool and per-Tenant quotas protect memory and concurrency. Task cancellation and node timeout propagate through the HTTP cancellation endpoint; runtime shutdown stops intake, drains bounded work, and closes all contexts and browser processes.

All browser traffic passes through a runtime-owned enforcing proxy. Hostnames are resolved before connection, every resolved address is checked, the proxy connects to the validated address, redirects are checked again, and loopback, private, link-local, multicast, unspecified, and cloud metadata destinations are denied unless an operator explicitly allowlists the hostname. Only HTTP and HTTPS navigation is accepted. Arbitrary JavaScript evaluation is disabled by default and requires an explicit deployment policy.

Artifacts are stored outside PostgreSQL. Local persistent storage is supported for a single replica; S3-compatible object storage is required when artifact access must be shared across replicas. Metadata and retrieval are Tenant-bound. Logs redact authorization, cookies, form values, uploaded content, and secrets. Prometheus metrics and OpenTelemetry traces expose capacity, latency, errors, and lifecycle state without exposing page content.

The runtime does not bypass CAPTCHA, anti-bot controls, authentication challenges, or website access policy. First-party CAPTCHA pages may be used only for controlled integration verification.

## Consequences

- Chromium failures and resource use are isolated from workflow orchestration.
- Browser capacity, storage, and security policy can be operated independently.
- Workflow graphs gain explicit Browser Start, Navigate, Click, Input, Wait, Extract, Screenshot, and Close nodes with a stable shared output shape.
- Session state is process-local in the initial deployment topology, so the safe default is one Browser Runtime replica. Horizontal scaling is enabled only with shared artifacts and must preserve session affinity until distributed session ownership is deployed.
- A runtime outage fails Browser nodes with a bounded, classified error and does not silently fall back to unsafe in-process execution.
