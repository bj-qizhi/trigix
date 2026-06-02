# Platform Backend

Rust service for platform business concerns.

Responsibilities:

- Auth
- Tenants
- Workspaces
- Projects
- Workflow metadata
- Agent metadata
- Capability library metadata
- Credentials
- Audit
- Billing
- REST API for the web console
- Executor client boundary
- AI Runtime client boundary

## Current Slice

The current implementation includes Rust domain services and Axum HTTP APIs:

- `ExecutionService` validates and starts Executions.
- `WorkflowService` creates/lists Workflows and creates/lists/loads stored Workflow Versions.
- `MemoryExecutionStore` is a temporary Store adapter.
- `PostgresExecutionStore` persists Executions against the local PostgreSQL schema.
- `PostgresWorkflowVersionStore` saves and loads persisted Workflow Graphs.
- `InlineExecutorClient` runs the Rust executor kernel in-process for the first vertical slice.
- `HttpExecutorClient` calls the standalone Executor service when `EXECUTOR_BASE_URL` is configured.
- Execution responses include node-level results from `node_executions`.
- `workflow-core` owns shared Workflow Graph validation.
- `execution-core` owns shared Execution and Node status types.

HTTP is wired with Axum for the first Execution slice.

Run:

```bash
PLATFORM_HTTP_ADDR=127.0.0.1:38080 cargo run -p trigix-platform
```

By default the service uses `MemoryExecutionStore`. To use PostgreSQL, provide `DATABASE_URL`:

```bash
DATABASE_URL=postgres://trigix:trigix@localhost:35432/trigix \
  EXECUTOR_BASE_URL=http://127.0.0.1:38090 \
  PLATFORM_HTTP_ADDR=127.0.0.1:38080 \
  cargo run -p trigix-platform
```

The local Docker PostgreSQL setup includes a dev seed with fixed Tenant, Workflow, and Workflow Version IDs. See `docs/dev/bootstrap.md`.

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

Save a Workflow Version:

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

List Workflow Versions:

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

Start an Execution from a stored Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflow-versions/version-1/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```

Start an Execution from the latest published Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/workflow-1/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "tenant-1",
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```

Start an Execution with an ad hoc graph:

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
        {"id": "agent", "type": "agent"}
      ],
      "edges": [
        {"source": "trigger", "target": "agent"}
      ]
    },
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```
