# Changelog

All notable changes to Trigix will be documented in this file.

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
