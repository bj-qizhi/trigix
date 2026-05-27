# Sort Node

## Status

done

## Category

enhancement

## What to build

A `Sort` node that orders a JSON array by a field value, supporting both string and numeric comparison in ascending or descending order.

## Acceptance criteria

- [x] `NodeType::Sort` added to `workflow-core`.
- [x] `execute_sort` in `executor/src/executor.rs`:
  - `items` (required): expression resolving to a JSON array.
  - `field` (required): dot-path on each item to sort by.
  - `order`: optional, `"asc"` (default) or `"desc"`.
  - `type`: optional, `"string"` (default, lexicographic) or `"number"` (f64 comparison).
  - Nulls / missing fields sort to the end.
  - Returns `{ "count": N, "items": [...sorted...] }`.
- [x] Dispatch case `NodeType::Sort => execute_sort(node, context)`.
- [x] `Sort => "sort"` in `node_type_to_str`.
- [x] 3 executor tests: `sort_node_ascending_string`, `sort_node_descending_numeric`, `sort_node_fails_when_items_not_array`.
- [x] Frontend: `sort` in `NodeType`, NODE_LABELS (`Sort`), NODE_ICONS (`⇅`), nodeTypes, MiniMap color `#d97706`, CSS `--node-sort: #d97706`.
- [x] `SortConfig` panel: items expression, field, order select (asc/desc), type select (string/number) — order+type in a two-column row.
- [x] Palette entry in WorkflowEditor.
- [x] 105 Rust tests (45 executor + 56 platform + 4 workflow-core), 0 TypeScript errors.

## UX flow

Palette → drag Sort → config: items + field + order + type → downstream uses `{{sort_node.items}}` for sorted results
