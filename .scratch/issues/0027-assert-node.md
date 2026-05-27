# Assert Node

## Status

done

## Category

enhancement

## What to build

An `Assert` node that evaluates a `{{...}}` expression and halts the workflow with a custom message if the resolved value is falsy. Enables validation/guard patterns (e.g., "stop if filter returned zero results").

## Acceptance criteria

- [x] `NodeType::Assert` added to `workflow-core`.
- [x] `is_truthy(s: &str) -> bool` helper: falsy = `"" | "false" | "null" | "0" | "[]" | "{}"`; everything else truthy.
- [x] `execute_assert` in `executor/src/executor.rs`:
  - `condition` (required): `{{...}}` expression resolved via `resolve_template`.
  - `message` (optional): failure message, default `"Assertion failed"`.
  - Returns `{ "ok": true }` when truthy; fails with `message` when falsy.
- [x] Dispatch case `NodeType::Assert => execute_assert(node, context)`.
- [x] `Assert => "assert"` in `node_type_to_str`.
- [x] 3 executor tests: `assert_node_passes_truthy_value`, `assert_node_fails_falsy_value`, `assert_node_uses_default_message`.
- [x] Frontend: `assert` in `NodeType`, NODE_LABELS (`Assert`), NODE_ICONS (`⊘`), nodeTypes, MiniMap color `#dc2626`, CSS `--node-assert: #dc2626`.
- [x] `AssertConfig` panel: condition textarea, message input, TemplateHint, output description.
- [x] Canvas preview: `assert {condition}` or "No condition set".
- [x] Palette entry in WorkflowEditor.
- [x] 109 Rust tests (48 executor + 57 platform + 4 workflow-core), 0 TypeScript errors.

## UX flow

Palette → drag Assert → config: condition `{{filter.count}}` + message `"No results found"` → fails execution if count is 0

## Note

`"input"` is a reserved keyword in `resolve_expr` (reads `context.input_json`). Node ID `input` in `node_outputs` is never accessible via `{{input.field}}`. Tests must use real node IDs (e.g. `filter`, `some_node`).
