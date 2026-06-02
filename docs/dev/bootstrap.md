# Dev Bootstrap

The local PostgreSQL container runs every SQL file under `infra/postgres/migrations/` on first database initialization.

The dev seed creates:

| Entity | ID |
|---|---|
| Tenant | `00000000-0000-4000-8000-000000000001` |
| User | `00000000-0000-4000-8000-000000000002` |
| Workspace | `00000000-0000-4000-8000-000000000003` |
| Project | `00000000-0000-4000-8000-000000000004` |
| Workflow | `00000000-0000-4000-8000-000000000005` |
| Workflow Version | `00000000-0000-4000-8000-000000000006` |

Start infrastructure:

```bash
docker compose up -d
```

Run the full local verification:

```bash
make dev-verify
```

The verification script requires at least 1GB free on the filesystem containing the repository. PostgreSQL can fail during startup with `No space left on device` when the host filesystem is full.

Run Rust Platform against PostgreSQL:

```bash
DATABASE_URL=postgres://velara:velara@localhost:35432/velara \
  EXECUTOR_BASE_URL=http://127.0.0.1:38090 \
  PLATFORM_HTTP_ADDR=127.0.0.1:38080 \
  cargo run -p velara-platform
```

Run Rust Executor:

```bash
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 cargo run -p velara-executor
```

Load the seeded Workflow Version:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflow-versions/00000000-0000-4000-8000-000000000006?tenant_id=00000000-0000-4000-8000-000000000001"
```

Create a Workflow in the seeded Project:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001",
    "workspace_id": "00000000-0000-4000-8000-000000000003",
    "project_id": "00000000-0000-4000-8000-000000000004",
    "name": "Lead Workflow"
  }'
```

List Workflows:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows?tenant_id=00000000-0000-4000-8000-000000000001&project_id=00000000-0000-4000-8000-000000000004"
```

Filter Workflows by status:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows?tenant_id=00000000-0000-4000-8000-000000000001&project_id=00000000-0000-4000-8000-000000000004&status=published"
```

Get one Workflow:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005?tenant_id=00000000-0000-4000-8000-000000000001"
```

Rename a Workflow:

```bash
curl -sS -X PATCH http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005 \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001",
    "name": "Renamed Lead Workflow"
  }'
```

Archive a Workflow:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005/archive \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001"
  }'
```

Restore a Workflow:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005/restore \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001"
  }'
```

Archived Workflows keep versions and executions for audit, but cannot be run or modified with new/published versions.

Save a new draft Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005/versions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001",
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
curl -sS "http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005/versions?tenant_id=00000000-0000-4000-8000-000000000001"
```

Filter Workflow Versions by status:

```bash
curl -sS "http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005/versions?tenant_id=00000000-0000-4000-8000-000000000001&status=draft"
```

Publish a Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflow-versions/00000000-0000-4000-8000-000000000006/publish \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001"
  }'
```

Start an Execution from the seeded Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflow-versions/00000000-0000-4000-8000-000000000006/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001",
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```

Start an Execution from the latest published Workflow Version:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/workflows/00000000-0000-4000-8000-000000000005/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001",
    "input_json": "{\"lead_id\":\"lead-1\"}"
  }'
```

Start an Execution by sending an ad hoc graph:

```bash
curl -sS -X POST http://127.0.0.1:38080/v1/executions \
  -H 'Content-Type: application/json' \
  -d '{
    "tenant_id": "00000000-0000-4000-8000-000000000001",
    "workflow_id": "00000000-0000-4000-8000-000000000005",
    "workflow_version_id": "00000000-0000-4000-8000-000000000006",
    "graph": {
      "workflow_version_id": "00000000-0000-4000-8000-000000000006",
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
