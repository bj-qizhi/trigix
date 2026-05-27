# Export / Import Workflow JSON

## Status

done

## Category

enhancement

## What to build

Allow users to export a published workflow graph as a JSON file and import that file to create a new workflow in any project, enabling workflow sharing and backup.

## Acceptance criteria

- [x] `GET /v1/workflows/:id/export?tenant_id=...` → `WorkflowExport { name, graph, exported_at }` using the workflow's latest published version.
- [x] Returns `400 NoPublishedVersion` if the workflow has no published version.
- [x] `POST /v1/workflows/import` body: `{ tenant_id, workspace_id, project_id, name?, graph }` → creates a new draft workflow + draft version, returns `WorkflowRecord`.
- [x] `name` is optional — defaults to `"Imported Workflow"` if absent or blank.
- [x] Audit event `workflow.created` emitted on import.
- [x] 2 HTTP tests: `exports_workflow_graph_over_http`, `imports_workflow_from_json_over_http`.
- [x] `WorkflowExport` TypeScript type in `types/index.ts`.
- [x] `exportWorkflow(tenantId, workflowId)` and `importWorkflow(tenantId, workspaceId, projectId, name, graph)` in `api/client.ts`.
- [x] `WorkflowEditor` toolbar: "↓ Export" button — downloads the JSON file; disabled when no published version.
- [x] `WorkflowList` topbar: "↑ Import" button — opens a hidden `<input type="file">`, parses JSON, calls import API, then opens the new workflow in the editor.
- [x] 79 Rust tests (23 executor + 52 platform + 4 workflow-core), 0 TypeScript errors.

## How export/import works

**Export:**
1. `GET /v1/workflows/:id/export?tenant_id=T` returns `{ name, graph, exported_at }`.
2. In the editor, clicking "↓ Export" fetches the endpoint and triggers a browser `<a>` download of the prettified JSON with filename `<workflow-name>.json`.

**Import:**
1. User clicks "↑ Import" in WorkflowList — opens a file picker.
2. The file is read as text, parsed as `WorkflowExport` JSON.
3. `POST /v1/workflows/import` creates the workflow + a single draft version containing the imported graph.
4. The editor opens immediately so the user can review, adjust, and publish.

## Route conflict note

`/v1/workflows/import` is a fixed path that sits alongside `/v1/workflows/:workflow_id/export`. Axum matches fixed segments before parameterised ones, so the import route is registered with an explicit `GET → method_not_allowed` guard to ensure it is never confused with an export call on a workflow named "import".
