# Transform Node

## Status

done

## Category

enhancement

## What to build

Add a `transform` node type that applies a JSON template with `{{...}}` interpolation to produce structured output without any external calls. Enables data reshaping between nodes.

## Acceptance criteria

- [x] `NodeType::Transform` added to `crates/workflow-core/src/graph.rs`.
- [x] `execute_transform(node, context) -> NodeExecutionResult` in `services/executor/src/executor.rs`:
  - Config: `template` (any JSON value — object, array, or string — containing `{{...}}` expressions).
  - Passes `template` through `resolve_config_strings` to interpolate all string values.
  - Returns the rendered template value as output JSON (`.to_string()` on the resolved `serde_json::Value`).
  - Fails if config is absent or `template` key is missing.
- [x] `NodeType::Transform` arm added to `dispatch` match in `executor.rs`.
- [x] `node_type_to_str` in `execution.rs` updated to include `"transform"`.
- [x] 3 executor unit tests: `transform_node_renders_template`, `transform_node_passes_through_scalar_template`, `transform_node_fails_without_template`.
- [x] Frontend: `'transform'` added to `NodeType` union in `types/index.ts`.
- [x] Canvas: `NODE_LABELS['transform'] = 'Transform'`, `NODE_ICONS['transform'] = '⇄'`, preview shows "template configured" / "No template set", `nodeTypes` includes `transform`, MiniMap color `'#0d9488'`.
- [x] CSS: `--node-transform: #0d9488` variable + `.flow-node-transform .flow-node-header` rule.
- [x] `WorkflowEditor` palette: `{ type: 'transform', label: 'Transform', color: 'var(--node-transform)', icon: '⇄' }`.
- [x] `NodeConfigPanel`: `TransformConfig` component — JSON template textarea (falls back to string if not valid JSON), template hint, output format hint.
- [x] 87 Rust tests (30 executor + 53 platform + 4 workflow-core), 0 TypeScript errors.

## How transform execution works

1. Config must have a `template` key whose value is any JSON value (object, array, or string).
2. `resolve_config_strings` is called on the template, recursively interpolating all `{{...}}` expressions in string values.
3. The resolved value is serialized back to JSON string and returned as the node output.
4. Downstream nodes can reference `{{transform_node_id.field}}` to read from the shaped output.

## Example

Input: `{ "first": "Alice", "last": "Smith" }`

Transform config:
```json
{
  "template": {
    "full_name": "{{input.first}} {{input.last}}",
    "email_key": "{{input.first}}"
  }
}
```

Output: `{"full_name":"Alice Smith","email_key":"Alice"}`
