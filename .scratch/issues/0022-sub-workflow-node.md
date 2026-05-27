# Sub-Workflow Node

## Status

done

## Category

enhancement

## What to build

A `SubWorkflow` node type that calls another published workflow inline during execution, waits for it to complete, and returns its output. Enables composing complex workflows from reusable building blocks.

## Acceptance criteria

- [x] `NodeType::SubWorkflow` added to `workflow-core`.
- [x] `execute_sub_workflow` in `executor/src/executor.rs`:
  - Reads `_graph` (pre-injected `WorkflowGraph` JSON) from node config.
  - Reads optional `input_template` (JSON with `{{...}}`) for sub-execution input; falls back to current `input_json`.
  - Creates a fresh `DispatchingNodeExecutor` for the sub-execution.
  - Calls `run_workflow` recursively with sub-ID `"{exec_id}:sub:{node_id}"`.
  - Returns `{ "status": "succeeded", "output": <last succeeded node output> }` on success; propagates failure.
- [x] `NodeExecutor` trait changed from `async fn execute` to boxed-future return (`Pin<Box<dyn Future + Send + 'a>>`) to allow async recursion without infinite future types.
- [x] `inject_sub_workflow_graphs` in `platform-rs/src/http.rs`:
  - For each `SubWorkflow` node, resolves `workflow_id` → published version → resolves credentials → injects graph as `_graph` in node config.
  - Called in `start_execution`, `start_execution_from_workflow`, `start_execution_from_workflow_version`.
- [x] `SubWorkflow => "sub_workflow"` in `node_type_to_str`.
- [x] 2 executor tests: `sub_workflow_node_fails_without_graph_config`, `sub_workflow_node_runs_embedded_graph`.
- [x] Frontend: `sub_workflow` in `NodeType`, `NODE_LABELS`, `NODE_ICONS`, `nodeTypes`, MiniMap colors, CSS variable `--node-sub-workflow: #be185d`.
- [x] `SubWorkflowConfig` panel: `workflow_id` text input, `input_template` JSON textarea.
- [x] 94 Rust tests (34 executor + 56 platform + 4 workflow-core), 0 TypeScript errors.

## UX flow

Palette → drag Sub-Workflow → config panel: set Workflow ID + optional Input Template → publish → run → executor calls sub-workflow inline → result available as `{{sub_node.output.field}}` in downstream nodes

## Known limitation

Nested sub-workflows (a sub-workflow containing another sub-workflow) are supported at the executor level (the recursive executor correctly handles it), but the `inject_sub_workflow_graphs` resolver only injects one level deep (no recursive graph injection in the HTTP handlers).
