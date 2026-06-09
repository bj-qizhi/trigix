# Trigix

**AI-Native Workflow Automation Platform**

[![CI](https://github.com/bj-qizhi/trigix/actions/workflows/ci.yml/badge.svg)](https://github.com/bj-qizhi/trigix/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-18-61dafb.svg)](https://react.dev/)

[English] · [中文](README.zh.md)

> © 2026 [北京祺智科技有限公司](https://www.qzso.com/) · managecode@gmail.com

---

## What is Trigix?

Trigix is an enterprise-grade, AI-native workflow automation platform.
Build, run, and monitor complex workflows visually — connecting AI models, APIs, databases, and SaaS tools with a drag-and-drop canvas editor.

**Key differentiators:**
- **140 node types** — AI models (Claude, GPT-4, Gemini, Groq, Mistral…), SaaS integrations (Slack, Jira, Notion, Salesforce…), data transforms, control flow — plus your own via the [node SDK](#extend-with-custom-nodes-node-sdk)
- **Rust execution engine** — DAG scheduling, parallel fan-out, retries, timeouts, cancellation
- **AI-native** — built-in LLM nodes, RAG over pgvector, an agent tool-use loop, MCP protocol support
- **Enterprise-ready** — SSO (OIDC), JWT + RBAC, multi-tenant, encrypted secrets, audit log, webhook signatures, Kubernetes Helm chart

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
DATABASE_URL=postgres://trigix:trigix@localhost:35432/trigix \
PLATFORM_HTTP_ADDR=127.0.0.1:38080 \
cargo run -p trigix-platform

# 3. Run the execution engine
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 \
cargo run -p trigix-executor

# 4. Run the web console
cd apps/web && npm install && npm run dev
# Open http://localhost:3100
```

Default dev API key: `dev`

---

## Deploy

**Kubernetes (Helm chart, published to GHCR):**

```bash
helm install trigix oci://ghcr.io/bj-qizhi/charts/trigix --version 0.3.2 \
  --namespace trigix --create-namespace
```

Deploys the platform, AI runtime, PostgreSQL/pgvector, and Redis. Configure via
`charts/trigix/values.yaml`. The chart `.tgz` is also attached to each
[`chart-v*` release](https://github.com/bj-qizhi/trigix/releases).

**Docker Compose (single host):**

```bash
docker compose -f docker-compose.prod.yml up -d --build
```

---

## Repository Structure

```text
apps/web                 React web console (Vite + React Flow)
services/platform-rs     Rust platform API (Axum, JWT, multi-tenant)
services/executor        Rust execution engine (DAG, parallel, retries)
services/ai-runtime      Python AI runtime (FastAPI) — RAG (pgvector) + agent tool-use
sdk/python               Custom node SDK (Python) — published as trigix-node-sdk
sdk/typescript           Custom node SDK (TypeScript/JS) — published as trigix-node-sdk
crates/workflow-core     Shared WorkflowGraph model + DAG validation
crates/execution-core    Shared ExecutionStatus types
infra/postgres           PostgreSQL migrations
charts/trigix            Kubernetes Helm chart
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
| [Demo: Feedback Triage](docs/demo-feedback-triage.md) | Annotated end-to-end run of a custom-node pipeline |

---

## Extend with custom nodes (Node SDK)

Write your own workflow node as a small HTTP service — in Python or
TypeScript/JavaScript — and use it like any built-in node. No changes to the
Trigix executor required.

```bash
pip install trigix-node-sdk          # Python
npm install trigix-node-sdk          # TypeScript / JavaScript
```

```python
from trigix_node_sdk import node, create_app

@node(slug="greet", label="Greeter",
      config_schema={"type": "object", "properties": {"name": {"type": "string"}}})
def greet(config, input, node_outputs):
    return {"greeting": f"Hello, {config.get('name') or input.get('name', 'world')}!"}

app = create_app()        # uvicorn module:app --port 9000
```

Then in the web console → **Custom Nodes**, paste your service URL and click
**Import All** — every node from `GET /manifest` is registered at once and shows
up in the workflow editor.

- [Python SDK](sdk/python) · [PyPI](https://pypi.org/project/trigix-node-sdk/)
- [TypeScript SDK](sdk/typescript) · [npm](https://www.npmjs.com/package/trigix-node-sdk)
- [Releasing the SDKs](sdk/RELEASING.md)

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
