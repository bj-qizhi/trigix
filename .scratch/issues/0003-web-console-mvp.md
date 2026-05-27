# Web Console MVP

## Status

done

## Category

enhancement

## What to build

Build a minimal but functional React web console that lets users create and edit Workflow graphs, configure nodes, save and publish versions, and run executions — all connected to the live Platform REST API.

## Acceptance criteria

- [x] Vite + React 18 + TypeScript project with production build.
- [x] Workflow list page: list, create new workflow via modal.
- [x] Workflow editor page with React Flow canvas.
- [x] Left palette: click to add Trigger, HTTP, Agent, Condition nodes.
- [x] DAG auto-layout: nodes placed by topological level on load.
- [x] Node connections: drag handles to create edges, Delete key to remove.
- [x] Right config panel: context-sensitive form for each node type.
- [x] Save Version: saves current graph as a new draft version.
- [x] Publish: publishes the latest draft version.
- [x] Run: executes the latest published version, shows results in bottom panel.
- [x] Execution panel: per-node status (dot + badge) and output preview.
- [x] Toast notifications for success/error feedback.
- [x] Dark theme matching platform aesthetic.
- [x] Vite proxy: `/v1/*` → `http://127.0.0.1:38080` (no CORS issues in dev).
- [x] `VITE_TENANT_ID` / `VITE_WORKSPACE_ID` / `VITE_PROJECT_ID` env vars for non-dev seeds.

## Notes

- No auth yet — tenant/workspace/project IDs are hardcoded from dev seed.
- Node positions are computed client-side on load and not persisted to the API.
- The dev server runs on port 3100 to match existing port assignments.
