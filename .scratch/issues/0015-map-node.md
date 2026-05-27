# Map Node

## Status

done

## Category

enhancement

## What to build

Add a `map` node type that fans out execution over a JSON array, applying an optional item template to each element, and returns a collected result array. This enables per-item transformation without sub-executions.

## Acceptance criteria

- [x] `NodeType::Map` added to `crates/workflow-core/src/graph.rs`.
- [x] `execute_map(node, context) -> NodeExecutionResult` in `services/executor/src/executor.rs`:
  - Config: `items` (string expression like `{{trigger.leads}}` resolving to a JSON array), `item_template` (optional JSON object/value where `{{item.field}}` substitution is applied).
  - Resolves `items` via existing `resolve_template`, parses result as JSON array.
  - For each element: if `item_template` present, renders it using `resolve_config_strings` with a child context where `node_outputs["item"]` = element JSON; otherwise passes element through.
  - Returns `{ "count": N, "items": [...] }`.
- [x] `NodeType::Map` arm added to `dispatch` match in `executor.rs`.
- [x] `node_type_to_str` in `execution.rs` updated to include `"map"`.
- [x] 3 executor unit tests: `map_node_fans_out_array_passthrough`, `map_node_applies_item_template`, `map_node_fails_when_items_not_array`.
- [x] 1 runtime integration test: `map_node_runs_in_workflow`.
- [x] Frontend: `'map'` added to `NodeType` union in `types/index.ts`.
- [x] Canvas: `NODE_LABELS`, `NODE_ICONS`, `nodeTypes`, MiniMap colors updated with `map` entry; node preview shows items expression.
- [x] CSS: `--node-map: #e05d44` variable + `.flow-node-map .flow-node-header` rule.
- [x] `WorkflowEditor` palette: `{ type: 'map', label: 'Map', ... }` entry.
- [x] `NodeConfigPanel`: `MapConfig` component — items expression field, item_template JSON textarea, template hint.
- [x] 83 Rust tests (27 executor + 52 platform + 4 workflow-core), 0 TypeScript errors.

## How map execution works

1. `items` config is a template expression (e.g. `{{trigger.leads}}`). It is resolved against the standard execution context using the existing `resolve_template` function, yielding a JSON string.
2. The JSON string is parsed; if it is not an array, the node fails immediately.
3. For each element in the array a child `ExecutionContext` is created with `node_outputs["item"]` set to the element's JSON. The `item_template` (if present) is passed through `resolve_config_strings` with that child context, so `{{item.field}}` expressions work identically to all other template expressions in the system.
4. If no `item_template` is configured, elements are passed through unmodified.
5. Output: `{ "count": N, "items": [...transformed elements...] }`.

## Template example

Input from trigger: `{"leads": [{"name": "Alice", "email": "alice@x.com"}, {"name": "Bob", "email": "bob@x.com"}]}`

Map config:
```json
{
  "items": "{{trigger.leads}}",
  "item_template": { "label": "{{item.name}}", "contact": "{{item.email}}" }
}
```

Output: `{ "count": 2, "items": [{ "label": "Alice", "contact": "alice@x.com" }, { "label": "Bob", "contact": "bob@x.com" }] }`
