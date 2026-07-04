# Changelog

All notable changes to Trigix will be documented in this file.

## [Unreleased]

## [1.5.0] - 2026-07-04

The workflow editor's AI assistant grows from a read-only Q&A box into an
agentic, streaming, multi-provider copilot; the generator learns the full node
palette and stops double-spending LLM calls; workflows can declare their output
as a typed, labeled contract that the run view renders by content type; and AI
assist can draw its key from the stored credential vault.

### Added

- **The editor copilot can now edit the canvas.** Ask it to add / fix / rewire
  nodes and it replies with the complete updated graph; an "Apply to canvas"
  button drops the change into the editor (undo / redo / save as usual). It
  streams its reply token-by-token (new `POST /v1/copilot/stream`, SSE) and works
  with any OpenAI-compatible provider, not just Anthropic.
- **Output Schema — declare what a workflow produces.** A workflow can define
  output fields (key / type / description / source), where `source` maps from node
  outputs or the input via `{{node_id.field}}` / `{{input.field}}`. The final
  `output_json` is assembled into a clean, labeled, typed object instead of
  guessing "the last node"; workflows without a schema are unchanged.
- **Friendly result rendering.** The execution view renders output by content
  type — labeled fields, image / video previews, arrays-of-objects as tables,
  links, text, or a JSON tree — with a raw-JSON toggle.
- **Reuse stored credentials for AI assist.** The generate modal and copilot panel
  can draw their API key from a saved credential (resolved + decrypted
  server-side) instead of pasting one. A structured generate entry is also
  available inside the editor (apply-to-canvas).

### Changed

- **The workflow generator knows the full node palette.** Generation was limited
  to ~30 curated node types; it now also receives all 178 node types, so it can
  reach any integration node (Discord, Stripe, Notion, …).

### Fixed

- **"Create Workflow" no longer re-generates.** The generate modal's create button
  persisted a second LLM call's result rather than the previewed graph — it now
  saves exactly what you previewed.

## [1.4.0] - 2026-07-02

Live token streaming across every deployment mode, retrieval-augmented
generation as a first-class node, a round of AI-node correctness fixes, and the
consolidation of the per-vendor LLM nodes — plus the fail-closed credential
encryption staged since 1.3.0.

### Added

- **Live token streaming — "watch the AI type" — end to end.** LLM and agent
  output now streams into the run view token-by-token instead of appearing only
  when the node finishes. An in-process execution event bus (bypassing the DB)
  plus a task-local token sink carry deltas from every direct LLM node (OpenAI,
  Gemini, Claude, and the OpenAI-compatible providers) and from the agent (across
  the ai-runtime boundary). Works in inline, separate-HTTP-executor, and
  multi-instance / queue deployments (the last via a Redis pub/sub bridge). The
  final node output is byte-identical to the non-streamed path.
- **Retrieval-augmented generation as a node.** The `rag` node gains a `generate`
  mode: it retrieves from the knowledge base, grounds a prompt on the retrieved
  chunks, and has an LLM answer — returning `{answer, sources, usage}` (with live
  streaming) instead of only raw chunks. New `POST /v1/rag/generate` (and
  `/generate/stream`) endpoints.

### Changed

- **Consolidated eight per-vendor LLM nodes into one `openai_compat` node.**
  grok / ollama / deepseek / qwen / zhipu / moonshot / doubao / hunyuan were
  identical shells over the same OpenAI-compatible call; they are now a single
  node whose `config.provider` selects a preset endpoint + default model (or
  `config.base_url` targets any other endpoint). Existing workflows migrate
  transparently on load — the vendor name becomes `config.provider` — so no
  action is required. (`minimax` / `ernie` stay separate: vendor-specific auth.)
- **BREAKING (ops):** Persistent deployments must now provide
  `CREDENTIAL_MASTER_KEY` (or explicitly set `ALLOW_PLAINTEXT_CREDENTIALS=true`).
  The Helm chart fails the install with a clear message when neither is set
  (`secrets.credentialMasterKey` / `allowPlaintextCredentials`), and the
  production Compose file requires `CREDENTIAL_MASTER_KEY` at parse time.
  Existing installs that relied on the silent plaintext fallback must set one of
  these to start. In-memory (no-database) mode is unaffected.

### Fixed

- **RAG search never mixes embedding backends.** Each stored chunk records the
  embedding backend that produced it; vector search only compares vectors from
  the active backend, so a knowledge base ingested under one backend and queried
  under another no longer returns nonsense-ranked results.
- **Real JSONPath-lite field addressing.** The shared path resolver
  (extract / filter / sort / aggregate / dedupe / join) now understands bracket
  indices (`items[0]`), quoted keys (`['a.b']`), an optional leading `$`, and the
  `[*]` wildcard (extract returns every match as an array); plain dot paths are
  unchanged.
- **Data / AI node correctness:** the `regex` node does real regex matching
  (numbered + named capture groups), `csv` parses RFC-4180 (quoted fields with
  commas) instead of naive splitting, `video_gen` fails loudly on an empty result
  URL, the agent's tool-call trace is exposed in its output, Chinese text no
  longer produces zero embedding vectors, and RAG hybrid search applies
  `min_score` to the vector half while batching and offloading remote embedding
  calls off the event loop.
- Transient LLM failures (429 / 5xx / connect / timeout) are retried with
  exponential backoff in the shared OpenAI-compatible call, not only in the agent
  loop.

### Security

- **Credential encryption is now fail-closed.** Stored secrets (credential
  values and SSO/OIDC client secrets) are encrypted at rest with AES-256-GCM via
  `CREDENTIAL_MASTER_KEY`. Previously a missing key — or even an encryption error
  while a key *was* configured — silently fell back to writing **plaintext** to
  the database. Now an encryption failure fails closed (never downgrades to
  plaintext-at-rest), and a persistent deployment without a key refuses to start
  unless `ALLOW_PLAINTEXT_CREDENTIALS=true` is set.

## [1.3.0] - 2026-06-16

A large expansion of the workflow node catalog — **44 new node types
(136 → 180)** — covering the gaps against comparable workflow/automation
tools and deepening the AI-native and Chinese-enterprise coverage. Each node
is full-stack (engine + config UI) and backward compatible. Most are plain
HTTP; the few that aren't were deliberately implemented in pure Rust (or via a
runtime CLI) so the default `cargo build` needs no extra system library.

### Added

**LLM providers**
- `azure_openai` (deployment-based, `api-key` header), `vertex` (Google Vertex
  AI / Gemini `generateContent` with a caller-supplied OAuth2 token), `bedrock`
  (AWS Bedrock `InvokeModel`, model-native body), `grok` (xAI), and `ollama`
  (self-hosted, OpenAI-compatible).

**AI-native building blocks** (OpenAI-/Cohere-compatible, configurable base URL)
- `embedding`, `reranker`, `text_splitter` (pure-compute, UTF-8-safe chunking),
  `structured_output` (LLM JSON mode), `classifier`, `image_gen`,
  `speech_to_text` (Whisper, multipart upload), and `tts`.

**Vector stores**
- `weaviate` (REST + GraphQL), `chroma` (REST data API), and `milvus` / Zilliz
  (REST API v2).

**Databases & data warehouses**
- `mongodb` (Atlas Data API), `clickhouse` (HTTP), `mysql` (sqlx), `snowflake`
  (SQL API v2, bearer token), `bigquery` (jobs.query REST), and `sqlserver`
  (Microsoft SQL Server over the pure-Rust tiberius TDS driver — no native
  client needed).

**Object storage**
- `gcs` (Google Cloud Storage JSON API) and `azure_blob` (REST + SAS token),
  both using caller-supplied credentials.

**AWS** (a from-scratch Signature V4 signer — no AWS SDK — validated against
AWS's published `get-vanilla` test vector)
- `sqs`, `sns`, and `bedrock`.

**Message brokers**
- `kafka` (Confluent REST Proxy) and `rabbitmq` (Management HTTP API).

**Chinese enterprise collaboration**
- `feishu` / Lark (custom-bot webhook or app message API), `dingtalk` (custom
  robot with optional HMAC sign), and `wecom` (WeChat Work group robot).

**Network file / shell / mail** (pure-Rust clients — no libssh2/system library)
- `ftp` / FTPS (suppaftp), `sftp` and `ssh` (russh / russh-sftp, password or
  private-key auth), and `imap` (TLS mailbox reads).

**Utilities & core primitives**
- `hash` (SHA-256/384/512 + HMAC), `jwt` (HS256/384/512 sign & verify),
  `zip` (create/extract), `image` (resize/convert/metadata), `pdf_extract`
  (text extraction), `ocr` (via the `tesseract` CLI), `html_extract` (CSS
  selectors), `rss` (RSS/Atom/JSON feeds), and `wait` (pause for a duration /
  until a timestamp, or suspend until externally resumed via the existing
  approve/resume endpoint).

### Notes

- The `ocr` node needs the `tesseract` CLI on the executor host; all other
  nodes have no extra runtime dependency.
- The `wait` (resume mode) and `snowflake`/`bigquery`/`vertex`/`gcs`/`azure_blob`
  nodes take a caller-supplied token rather than performing their own
  credential exchange.

## [1.2.0] - 2026-06-09

Deeper AI-native capabilities — the agent runs on Chinese / self-hosted models,
richer RAG retrieval, and acting tools — plus a deployment and distribution
chain that actually builds and deploys. Backward compatible with 1.1.0.

### Added

**AI-native**
- The Agent node runs on OpenAI-compatible models (Qwen, DeepSeek, Zhipu,
  Moonshot, a self-hosted vLLM/Ollama gateway) in addition to Anthropic, so the
  tool-use agent works in a self-hosted / China deployment where the Anthropic
  API is unreachable.
- Agent tools: a sandboxed `http_request` (default-deny egress, SSRF guard,
  DNS-rebinding-safe IP pinning, response size cap) and custom-node tools that
  let the agent call the tenant's own registered nodes.
- Hybrid RAG retrieval — vector + full-text fused with Reciprocal Rank Fusion —
  and an optional `min_score` floor; helps queries that hinge on exact tokens
  (codes, identifiers, English terms inside CJK text).
- Optional cross-encoder reranking over a Cohere/Jina/BGE-compatible endpoint
  (e.g. a self-hosted bge-reranker), with a dependency-free local fallback.
- An HNSW vector index for retrieval at scale, and CJK tokenization for the
  hybrid keyword side when a Chinese FTS config (pg_jieba / zhparser) is present.
- The Agent node reports token usage (`_agent_usage`).

**Deployment & CI**
- CI: a Docker image build smoke job (builds the platform and AI runtime images
  and checks they run) and a Helm job (lint + render across value permutations
  + kubeconform schema validation).
- A Dockerfile for the AI runtime (the repo had a platform image but none for
  the AI runtime).
- Helm chart: an `ai-runtime` Deployment + Service, so the chart deploys the
  full stack — platform + AI runtime + PostgreSQL/pgvector + Redis; the AI
  runtime and Redis were also added to `docker-compose.prod.yml`.
- The Helm chart is published to GHCR (`oci://ghcr.io/bj-qizhi/charts/trigix`)
  and attached to GitHub Releases; a workflow auto-publishes on a `chart-v*`
  tag, syncs both channels, and bumps the README install version.

### Changed

- Both Agent LLM backends run the (synchronous) model call off the event loop.
- The platform image tracks the latest stable Rust (`rust:1-slim`), matching CI,
  so it no longer rots when a dependency raises its edition / MSRV.
- Helm chart `0.3.2`, `appVersion` `1.2.0`.

### Fixed

- The platform Docker image could not build: a stale crate name left over from
  the agentflow→trigix rename, a Rust base image too old for the dependency
  tree (a transitive crate now requires edition 2024), and the migrations
  directory missing from the build stage (`sqlx::migrate!` reads it at compile
  time). The runtime image also lacked `curl`, which its healthcheck used.
- Helm chart could not deploy: the platform `DATABASE_URL` was a password-less
  placeholder so it could not authenticate, and the platform Service/PDB
  selectors also matched the Redis (and now AI runtime) pods. Both the platform
  and AI runtime now build the DSN from the postgres secret, and each component
  has a scoped selector.
- `docker-compose.prod.yml`: dropped a migrations `initdb` mount that conflicted
  with the app's own `sqlx::migrate!` step on a fresh database.
- The condition node now evaluates operators (`gt`/`lt`/`contains`/…) and a
  `source` path instead of silently falling back to an existence check, and the
  bundled gallery templates were corrected to read values from the right paths.

## [1.1.0] - 2026-06-05

New AI-native and enterprise capabilities, a custom node SDK, and a major
quality/CI uplift. Backward compatible with 1.0.0.

### Added

**AI-native**
- RAG knowledge store on pgvector in the AI runtime: ingest, embed (OpenAI or
  an offline local embedding), and cosine-similarity retrieval.
- `rag` and `rag_ingest` workflow nodes, plus a Knowledge Bases management page.
- Agent tool-use loop: the Agent node can call tools (sandboxed calculator and
  knowledge-base search) and iterate to an answer.

**Custom node SDK (node ecosystem)**
- Python (`trigix-node-sdk` on PyPI) and TypeScript/JavaScript
  (`trigix-node-sdk` on npm) SDKs for writing nodes served over HTTP.
- `custom` node type, a tenant-scoped node registry, and one-click registration
  from a node service's `/manifest`.
- Example nodes (HTML to text, PII redaction, sentiment) and an end-to-end demo.

**Enterprise**
- Enterprise SSO via OIDC (Okta / Azure AD / Google Workspace / Alibaba Cloud
  IDaaS / Huawei OneAccess / Tencent / Authing) plus Feishu, DingTalk, and
  WeChat Work; admin management UI with enable/disable.
- Encryption at rest for credential and SSO secrets (AES-256-GCM via
  `CREDENTIAL_MASTER_KEY`), with transparent passthrough of legacy plaintext.
- Dead-letter queue for the Redis Streams execution queue; failed jobs are
  preserved and can be re-driven instead of silently dropped.
- Opt-in data retention sweeper (`DATA_RETENTION_DAYS`) for executions, audit
  log, token usage, and webhook deliveries.

### Changed
- Split the 14k-line `executor.rs` (previously `include!`-spliced) into cohesive
  submodules.
- CI now enforces formatting, the full Rust test suite, the web production
  build, the AI runtime tests against a pgvector service, and both node SDK test
  suites.

### Fixed
- Repaired a test suite that did not compile and 122 frontend TypeScript build
  errors.
- Credential creation on PostgreSQL (an i64 was bound to a `TIMESTAMPTZ` column).
- A few latent frontend bugs surfaced by the typechecker.

## [1.0.0] - 2026-06-02

### 🎉 Initial Release

First public release of **Trigix** — AI-Native Workflow Automation Platform.

### Features

**Canvas Editor**
- Drag-and-drop workflow canvas powered by React Flow
- Minimap, snap-to-grid, undo/redo (50-step history)
- Keyboard shortcuts (Ctrl+S, Ctrl+Enter, Ctrl+K, Ctrl+Z, ?)
- Node palette with search, categories, and recent nodes
- Node duplication, custom labels, config raw JSON preview

**Execution Engine (Rust)**
- Async DAG scheduling with topological level-based parallel execution
- Fan-out / Fan-in parallel branches
- Sub-workflow and ForEach recursive execution
- Per-node retries (0–5, exponential backoff) and timeout
- Execution cancel, retry, bulk cancel
- Live node-by-node SSE streaming updates
- Dry-run mode (no external requests)

**136 Node Types**
- **AI**: Claude, OpenAI, Gemini, Groq, Mistral, Cohere, Replicate, Perplexity + 7 Chinese LLMs (Deepseek, Qwen, Zhipu, Moonshot, Doubao, Minimax, Ernie, Hunyuan)
- **Integration**: GitHub, Jira, Notion, Slack, Stripe, Salesforce, Airtable, Linear, Discord, Teams, Twilio, HubSpot, Zendesk, Shopify, Datadog, and 50+ more
- **Transform**: Filter, Map, Aggregate, Sort, Merge, Extract, Dedupe, Regex, CSV, XML, YAML, Split, Join, Rename, Format, Math, ArrayUtils, Handlebars
- **Control**: Condition, Approval, Catch, FanOut, FanIn, Loop, Switch, ForEach, SubWorkflow, Delay, Assert, Note
- **Utility**: HTTP, Webhook, Code (Rhai), Validate, Random, Crypto, Date, Database, Redis, Elasticsearch, GraphQL

**Triggers**
- Webhook with HMAC-SHA256 signature verification and replay-attack protection
- Cron expression scheduling with next-fire preview
- Interval-based scheduling
- Manual execution with input schema validation
- Form submit (public `/forms/:token` endpoint)

**Auth & Security**
- JWT authentication with 7-day tokens
- RBAC roles: Viewer / Editor / Admin
- API Key management with SHA256 hashing
- bcrypt password hashing, email verification, password reset
- Organization management with member RBAC
- Multi-tenant isolation with tenant ID enforcement

**Enterprise Features**
- PostgreSQL persistence (54 migrations)
- Redis Streams distributed execution queue
- Audit log with action filtering and CSV export
- Execution quota per tenant (free/pro/business/enterprise tiers)
- Webhook delivery tracking with exponential backoff retry
- Distributed scheduler lock (SELECT FOR UPDATE SKIP LOCKED)
- Prometheus metrics + OpenTelemetry tracing
- Kubernetes Helm Chart (HPA, PDB, pgvector, Redis)
- Docker multi-stage build + nginx SPA proxy
- Graceful shutdown (SIGTERM → drain → zero-loss)
- MCP (Model Context Protocol) native integration

**Web Console**
- Workflow list with search, filter, tags, sort, pinning, bulk actions
- Version history with diff view (structural + config-level changes)
- Execution detail with timeline, node results, audit trail
- Analytics dashboard with token usage, cost estimation, heatmap
- Real-time SSE updates across all pages
- Input schema with typed form generation
- Template gallery (18 pre-built workflows)
- AI-assisted workflow generation (Claude API)

### Tech Stack

- **Backend**: Rust (Axum 0.7, SQLx 0.8, Tokio)
- **Frontend**: React 18, TypeScript, Vite, React Flow
- **Database**: PostgreSQL 16 + pgvector
- **Cache/Queue**: Redis 7 (Streams)
- **AI Runtime**: Python (FastAPI)
- **Infrastructure**: Docker, Kubernetes

---

© 2026 北京祺智科技有限公司 · https://www.qzso.com/
