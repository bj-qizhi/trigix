# Workflow Graph JSON

Workflow Graph JSON is the platform contract shared by the web console, Rust Platform, Rust Executor, and future gRPC payloads.

## Shape

```json
{
  "workflow_version_id": "version-1",
  "nodes": [
    {
      "id": "trigger",
      "type": "trigger"
    },
    {
      "id": "agent",
      "type": "agent"
    }
  ],
  "edges": [
    {
      "source": "trigger",
      "target": "agent"
    }
  ]
}
```

## Node Types

Supported MVP node types:

- `trigger`
- `http`
- `agent`
- `condition`

## Validation Rules

- `workflow_version_id` is required.
- `workflow_version_id` must match the Execution request `workflow_version_id`.
- `nodes` must contain at least one node.
- Node IDs must be non-empty and unique.
- Node types must be supported.
- Every edge source and target must reference an existing node.
- The graph must be acyclic.

## Start Execution Request

```json
{
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
  "input": {
    "lead_id": "lead-1"
  }
}
```
