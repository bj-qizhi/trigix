# Aggregate Node

## Status

done

## Category

enhancement

## What to build

An `Aggregate` node that reduces a JSON array to a single scalar value. Completes the array-processing toolkit alongside Map and Filter.

## Acceptance criteria

- [x] `NodeType::Aggregate` added to `workflow-core`.
- [x] `execute_aggregate` in `executor/src/executor.rs`:
  - `items` (required): expression resolving to a JSON array.
  - `operation` (required): `count` | `sum` | `avg` | `min` | `max` | `join` | `first` | `last`.
  - `field`: required for all ops except `count`; optional for `first`/`last` (returns whole item if omitted).
  - `separator`: optional string for `join` (default `, `).
  - Returns `{ "result": <value> }` — integer/float for numeric ops, string for join, JSON value for first/last.
- [x] `json_number` helper: emits integer if whole number, float otherwise.
- [x] Dispatch case `NodeType::Aggregate => execute_aggregate(node, context)`.
- [x] `Aggregate => "aggregate"` in `node_type_to_str`.
- [x] 4 executor tests: `aggregate_node_count`, `aggregate_node_sum_and_avg`, `aggregate_node_join`, `aggregate_node_fails_with_unknown_operation`.
- [x] Frontend: `aggregate` in `NodeType`, NODE_LABELS (`Aggregate`), NODE_ICONS (`Σ`), nodeTypes, MiniMap color `#7c3aed`, CSS `--node-aggregate: #7c3aed`.
- [x] `AggregateConfig` panel: items expression, operation select (8 options), conditional field input, conditional separator input (for join).
- [x] Palette entry in WorkflowEditor.
- [x] 102 Rust tests (42 executor + 56 platform + 4 workflow-core), 0 TypeScript errors.

## UX flow

Palette → drag Aggregate → config: items + operation + field → downstream uses `{{agg_node.result}}`
