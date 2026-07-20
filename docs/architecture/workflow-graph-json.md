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

`type` is the serialized form of `workflow_core::NodeType`. The current runtime
supports the full production palette (roughly 180 types), including triggers,
control flow, transforms, Agent/RAG, model providers, databases, messaging,
storage, SaaS connectors, document processing, and human Approval.

The Rust `NodeType` enum is the compatibility contract. Executor runtime policy
(local, external, Approval, or Wait) is centralized in the Node registry. New
types must update the enum, registry policy, handler, frontend configuration,
and positive/negative execution tests in one change.

## Validation Rules

- `workflow_version_id` is required.
- `workflow_version_id` must match the Execution request `workflow_version_id`.
- `nodes` must contain at least one node.
- Node IDs must be non-empty and unique.
- Node types must deserialize to a supported `NodeType` variant.
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
