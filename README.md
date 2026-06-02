# AI Workflow

Enterprise AI Agent workflow low-code platform.

This repository is organized as a multi-service system:

```text
apps/web                 React web console
services/platform-rs     Rust platform backend
services/executor        Rust workflow execution engine
services/ai-runtime      Python AI Runtime
crates/workflow-core     Shared Workflow Graph model and validation
crates/execution-core    Shared Execution state model
proto                    Shared gRPC/protobuf contracts
infra                    Local infrastructure and migrations
packages/cli             Developer CLI scaffold
templates                Capability and workflow templates
docs                     Architecture, ADRs, and agent documentation
skills                   Local agent skills
```

## Architecture

See [docs/architecture/ai-agent-workflow-platform-design.md](docs/architecture/ai-agent-workflow-platform-design.md).

## Local Services

The initial local stack is:

- PostgreSQL for durable business state
- Redis for queue, cache, locks, and temporary execution state
- MinIO for files, documents, and artifacts

```bash
docker compose up -d
```

## Service Boundaries

```text
Rust platform backend:
  users, tenants, permissions, workflow metadata, credentials, audit, billing

Rust executor:
  workflow DAG execution, scheduling, retries, timeout, cancellation, event stream

Python AI Runtime:
  Agent, RAG, LLM providers, embedding, document parsing, prompt eval
```

Run the executor service (with AI Runtime wired for Agent nodes):

```bash
AI_RUNTIME_BASE_URL=http://127.0.0.1:38070 \
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 \
cargo run -p velara-executor
```

Run the AI Runtime:

```bash
cd services/ai-runtime
pip install -e .
ANTHROPIC_API_KEY=sk-ant-... uvicorn app.main:app --port 38070
```

Run the Web Console (proxies `/v1/*` to Platform API):

```bash
cd apps/web
npm install
npm run dev
# Open http://localhost:3100
```

## Current Status

**Seventh vertical slice complete.** Retry and timeout per node:

- **`max_retries`** (0–5): the executor retries a failed node up to N times with exponential backoff (200ms, 400ms, 800ms… capped at ~6.4s between attempts).
- **`timeout_secs`**: each attempt is individually wrapped with `tokio::time::timeout`; expiry marks the attempt as failed (triggering a retry if retries remain).
- Retry + timeout compose: a timed-out attempt counts as one failure before the next retry.
- Frontend: "Retries" and "Timeout (s)" fields added to HTTP and Agent node config panels.

**Sixth vertical slice complete.** Variable interpolation in node configs:

- **Template syntax**: `{{input.field}}`, `{{input}}`, `{{node_id.field}}`, `{{node_id}}` work in any string config value.
- **HTTP node**: URL, body, and headers resolve templates before the request — e.g. `https://api.example.com/{{input.id}}` or `{"user": "{{trigger.email}}"}`.
- **Condition node**: `field` accepts `{{node_id.field}}` to compare a prior node's output; `equals` also resolves templates.
- **Agent node**: `prompt_template` now resolves all `{{...}}` expressions (not just `{{input}}`), using the full `node_outputs` map from the executor.
- **Frontend**: Template hint shown under HTTP URL, HTTP body, Agent prompt template, and Condition equals fields.
- Unresolvable expressions silently collapse to empty string (mustache convention).

**Fifth vertical slice complete.** Canvas execution overlay:

- **Node status borders**: succeeded → green, failed → red, running → animated blue border, skipped → gray.
- **Status dot**: 8 px colored badge in the top-right corner of each canvas node.
- **Edge highlighting**: edges from a succeeded node turn green; failed → red; running → animated blue.
- **Node result in config panel**: clicking any node while an execution is loaded shows a "Last result" box with pretty-printed JSON output or error message at the top of the panel.
- All overlays use a React context (`NodeStatusContext`) so node positions are never disturbed by execution updates.

**Fourth vertical slice complete.** Async execution with polling and execution history:

- **Async execution**: `POST /v1/executions` returns `{ status: "running" }` immediately; a tokio background task drives execution to completion.
- **Polling**: Web Console polls `GET /v1/executions/{id}` every second while `status === "running"`, then auto-updates the execution panel.
- **Execution history**: Right sidebar shows recent executions for the workflow when no node is selected; click any entry to load its results.
- **`GET /v1/executions?workflow_id=`**: New optional filter to scope history to a specific workflow.
- **`fail()` on `ExecutionStore`**: Marks execution as `failed` if the background task errors, stored in all adapters (Memory, Postgres).

**Third vertical slice complete.** The full stack is live: Web Console → Platform → Executor → AI Runtime:

- **React Web Console** (`apps/web`): workflow list + editor canvas (React Flow), node config panel, execution panel. Runs on port 3100.
- **Rust Platform** (`services/platform-rs`): Workflow CRUD, WorkflowVersion publish/list, Execution start/list/get, PostgreSQL store adapter.
- **Rust Executor** (`services/executor`): DAG scheduling, `DispatchingNodeExecutor` routes Trigger, HTTP, Agent, and Condition nodes.
- **Python AI Runtime** (`services/ai-runtime`): `/v1/nodes/agent` calls the Claude API and returns structured output.
- Trigger node returns the workflow `input_json` as its output, available to downstream nodes.
- HTTP node makes real outbound requests from `config.url`, `config.method`, `config.headers`, `config.body`.
- Agent node calls AI Runtime with node config and execution context.
- Condition node evaluates a field presence or equality check against `input_json`.
- `Node.config` carries per-node JSON configuration, backward compatible.

## First Vertical Slice

The first vertical slice starts an Execution through the Rust Platform service core and validates the Executor graph scheduling core.

Run local checks:

```bash
cargo test
python3 -m py_compile services/ai-runtime/app/main.py
docker compose config
```

Run the local end-to-end verification:

```bash
make dev-verify
```

Run the Platform skeleton:

```bash
PLATFORM_HTTP_ADDR=127.0.0.1:38080 cargo run -p velara-platform
```

Create a Workflow:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "workspace_id": "workspace-1",
    "project_id": "project-1",
    "name": "Lead Workflow"
  }'
```

List Workflows:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows?tenant_id=tenant-1&project_id=project-1"
```

Filter Workflows by status:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows?tenant_id=tenant-1&project_id=project-1&status=published"
```

Get one Workflow:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows/workflow-1?tenant_id=tenant-1"
```

Rename a Workflow:

```bash
curl -sS -X PATCH http://127.0.0.1:38080/v1/workflows/workflow-1 \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "name": "Renamed Lead Workflow"
  }'
```

Archive a Workflow:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/workflow-1/archive \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1"
  }'
```

Restore a Workflow:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/workflow-1/restore \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1"
  }'
```

Archived Workflows keep versions and executions for audit, but cannot be run or modified with new/published versions.

Start an Execution (with real Agent node config):

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "workflow_id": "workflow-1",
    "workflow_version_id": "version-1",
    "graph": {
      "workflow_version_id": "version-1",
      "nodes": [
        {"id": "trigger", "type": "trigger"},
        {
          "id": "agent",
          "type": "agent",
          "config": {
            "model": "claude-sonnet-4-6",
            "system_prompt": "You are a helpful sales assistant.",
            "prompt_template": "Analyze this lead and summarize: {{input}}",
            "max_tokens": 512
          }
        }
      ],
      "edges": [
        {"source": "trigger", "target": "agent"}
      ]
    },
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```

Node config reference:

| Node type | Required config fields | Optional config fields |
|---|---|---|
| `trigger` | — | — |
| `http` | `url` | `method` (default GET), `headers`, `body`, `max_retries` (0–5), `timeout_secs` |
| `agent` | — | `model`, `system_prompt`, `prompt_template`, `max_tokens`, `max_retries` (0–5), `timeout_secs` |
| `condition` | `field` | `equals` (omit to test field presence) |

Then query it:

```bash
curl -sS "http://127.0.0.1:38080/v1/executions/<execution_id>?tenant_id=tenant-1"
```

You can also run from a stored Workflow Version instead of sending the full graph:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflow-versions/version-1/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```

Or run the latest published version of a Workflow:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/workflow-1/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```

The editor can save a new Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/workflow-1/versions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "graph": {
      "workflow_version_id": "client-supplied-id",
      "nodes": [
        {"id": "trigger", "type": "trigger"},
        {"id": "agent", "type": "agent"}
      ],
      "edges": [
        {"source": "trigger", "target": "agent"}
      ]
    }
  }'
```

List versions for a Workflow:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows/workflow-1/versions?tenant_id=tenant-1"
```

Filter Workflow Versions by status:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows/workflow-1/versions?tenant_id=tenant-1&status=draft"
```

Publish a Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflow-versions/version-1/publish \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1"
  }'
```

Current state:

- Platform manages Workflow / WorkflowVersion / Execution lifecycle via REST API.
- Executor schedules the DAG and dispatches each node by type: Trigger, HTTP, Agent, Condition.
- Trigger node passes workflow input to downstream nodes.
- HTTP node calls external URLs based on node config.
- Agent node calls the Python AI Runtime which invokes the Claude API.
- Condition node evaluates field conditions against `input_json`.

Environment variables:

| Variable | Service | Purpose |
|---|---|---|
| `DATABASE_URL` | Platform | Switch to PostgreSQL store (default: in-memory) |
| `EXECUTOR_BASE_URL` | Platform | Route executions to standalone Executor (default: inline) |
| `AI_RUNTIME_BASE_URL` | Executor | Enable Agent node execution via AI Runtime |
| `ANTHROPIC_API_KEY` | AI Runtime | Required for Claude API calls |

The PostgreSQL adapter currently assumes Tenant, Workflow, and Workflow Version records already exist.

See [docs/dev/bootstrap.md](docs/dev/bootstrap.md) for seeded local IDs and a PostgreSQL-backed Execution example.
