# Velara

**AI-Native Workflow Automation Platform**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-18-61dafb.svg)](https://react.dev/)
[![GitHub Stars](https://img.shields.io/github/stars/bj-qizhi/velara?style=social)](https://github.com/bj-qizhi/velara)
[![GitHub Issues](https://img.shields.io/github/issues/bj-qizhi/velara)](https://github.com/bj-qizhi/velara/issues)
[![GitHub Forks](https://img.shields.io/github/forks/bj-qizhi/velara?style=social)](https://github.com/bj-qizhi/velara/fork)

[English] · [中文](README.zh.md)

> © 2026 [北京祺智科技有限公司](https://www.qzso.com/) · managecode@gmail.com

---

## What is Velara?

Velara is an enterprise-grade, AI-native workflow automation platform.
Build, run, and monitor complex workflows visually — connecting AI models, APIs, databases, and SaaS tools with a drag-and-drop canvas editor.

**Key differentiators:**
- **136 node types** — AI models (Claude, GPT-4, Gemini, Groq, Mistral…), SaaS integrations (Slack, Jira, Notion, Salesforce…), data transforms, control flow
- **Rust execution engine** — DAG scheduling, parallel fan-out, retries, timeouts, cancellation
- **AI-native** — 8 built-in LLM nodes, pgvector-ready, MCP protocol support
- **Enterprise-ready** — JWT + RBAC, multi-tenant, audit log, webhook signatures, Kubernetes Helm chart

---

## Features

| Category | Highlights |
|----------|-----------|
| **Canvas Editor** | Drag-and-drop, React Flow, minimap, snap-to-grid, undo/redo, keyboard shortcuts |
| **Execution Engine** | Async DAG, parallel branches (Fan-out/Fan-in), sub-workflows, ForEach, loop |
| **AI Nodes** | Claude, OpenAI, Gemini, Groq, Mistral, Cohere, Replicate, Perplexity + 7 Chinese LLMs |
| **Integrations** | 100+ nodes: GitHub, Jira, Notion, Slack, Stripe, Salesforce, Airtable, Linear… |
| **Transforms** | Filter, Map, Aggregate, Sort, Merge, Extract, Dedupe, Regex, CSV, XML, YAML… |
| **Triggers** | Webhook (HMAC-SHA256), Cron, Interval, Manual, Form submit |
| **Auth & Security** | JWT, RBAC (Viewer/Editor/Admin), API Keys, bcrypt passwords, email verification |
| **Observability** | Audit log, execution timeline, Prometheus metrics, OpenTelemetry tracing |
| **Infrastructure** | PostgreSQL, Redis Streams, Docker, Kubernetes Helm Chart |

---

## Quick Start

```bash
# 1. Start local infrastructure
docker compose up -d

# 2. Run the platform backend
DATABASE_URL=postgres://velara:velara@localhost:35432/velara \
PLATFORM_HTTP_ADDR=127.0.0.1:38080 \
cargo run -p velara-platform

# 3. Run the execution engine
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 \
cargo run -p velara-executor

# 4. Run the web console
cd apps/web && npm install && npm run dev
# Open http://localhost:3100
```

Default dev API key: `dev`

---

## Repository Structure

```text
apps/web                 React web console (Vite + React Flow)
services/platform-rs     Rust platform API (Axum, JWT, multi-tenant)
services/executor        Rust execution engine (DAG, parallel, retries)
services/ai-runtime      Python AI runtime (FastAPI, LangChain)
crates/workflow-core     Shared WorkflowGraph model + DAG validation
crates/execution-core    Shared ExecutionStatus types
infra/postgres           54 database migrations
charts/aiworkflow        Kubernetes Helm chart
docs/                    Architecture, ADRs, dev guides
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture Design](docs/architecture/ai-agent-workflow-platform-design.md) | System architecture, service boundaries, domain model |
| [Workflow Graph JSON](docs/architecture/workflow-graph-json.md) | Node/Edge schema reference |
| [Dev Bootstrap Guide](docs/dev/bootstrap.md) | Local setup with PostgreSQL |
| [Port Reference](docs/dev/ports.md) | All local service ports |
| [ADR-0001: Layered Architecture](docs/adr/0001-layered-platform-architecture.md) | Architecture decision record |

---

## Environment Variables

| Variable | Service | Purpose |
|----------|---------|---------|
| `DATABASE_URL` | Platform | PostgreSQL connection (default: in-memory) |
| `EXECUTOR_BASE_URL` | Platform | Standalone executor URL (default: inline) |
| `AI_RUNTIME_BASE_URL` | Executor | Python AI runtime URL |
| `ANTHROPIC_API_KEY` | AI Runtime | Claude API key |
| `AUTH_REQUIRED` | Platform | Enforce JWT on all routes (`true`/`false`) |
| `DEV_API_KEY` | Platform | Dev API key (default: `dev`) |

---

## License

MIT License — see [LICENSE](LICENSE)

Copyright © 2026 北京祺智科技有限公司 · https://www.qzso.com/
