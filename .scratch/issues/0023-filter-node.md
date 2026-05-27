# Filter Node

## Status

done

## Category

enhancement

## What to build

A `Filter` node that reduces a JSON array to only items matching a per-item field condition. Complements the Map node for data-pipeline workflows.

## Acceptance criteria

- [x] `NodeType::Filter` added to `workflow-core`.
- [x] `execute_filter` in `executor/src/executor.rs`:
  - `items` (required): expression resolving to a JSON array.
  - `field` (required): dot-path field on each item to test.
  - `operator` (optional, default `"exists"`): `exists` | `not_exists` | `equals` | `not_equals` | `contains` | `gt` | `lt`.
  - `value`: required for `equals`, `not_equals`, `contains`, `gt`, `lt`.
  - Returns `{ "count": N, "items": [...] }` with matching items only.
- [x] Dispatch case `NodeType::Filter => execute_filter(node, context)`.
- [x] `Filter => "filter"` in `node_type_to_str`.
- [x] 4 executor tests: `filter_node_keeps_matching_items`, `filter_node_exists_operator`, `filter_node_gt_operator`, `filter_node_fails_when_items_not_array`.
- [x] Frontend: `filter` in `NodeType`, NODE_LABELS (`Filter`), NODE_ICONS (`⊃`), nodeTypes, MiniMap color `#0891b2`, CSS `--node-filter: #0891b2`.
- [x] `FilterConfig` panel: items expression, field, operator select (7 options), value input (shown only for comparison operators).
- [x] Palette entry in WorkflowEditor.
- [x] 98 Rust tests (38 executor + 56 platform + 4 workflow-core), 0 TypeScript errors.

## UX flow

Palette → drag Filter → config: set Items expression + Field + Operator (+ Value if needed) → downstream nodes reference `{{filter_node.items}}` or `{{filter_node.count}}`
