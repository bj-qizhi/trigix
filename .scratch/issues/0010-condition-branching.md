# Condition Node True/False Branching

## Status

done

## Category

enhancement

## What to build

Route workflow execution along the correct branch after a Condition node evaluates to `true` or `false`. Nodes on the inactive branch should be skipped rather than executed.

## Acceptance criteria

- [x] `Edge.condition_label: Option<String>` added to `workflow-core` (serialized as `"condition_label"`, omitted when `None`).
- [x] `run_workflow` in `services/executor/src/runtime.rs` builds an incoming-edge index and tracks:
  - `skipped: HashSet<String>` — nodes not reached by the active branch.
  - `condition_results: HashMap<String, bool>` — result of each condition node.
- [x] Skip rule: a node is skipped when it has ≥1 incoming edge AND every incoming edge is inactive (source skipped, OR condition label doesn't match source result).
- [x] Condition node output `{"result": true/false}` is parsed and stored for routing.
- [x] Skipped nodes emit `NodeReport { status: Skipped, ... }` and are not passed to the executor.
- [x] 2 new runtime tests: `condition_true_skips_false_branch`, `condition_false_skips_true_branch`.
- [x] All existing `Edge` struct literal sites updated to include `condition_label: None`.
- [x] `ApiEdge.condition_label?: 'true' | 'false'` added to TypeScript types.
- [x] Canvas: `approval` node fully wired (`NODE_LABELS`, `NODE_ICONS`, `nodeTypes`, MiniMap color, CSS header color).
- [x] Canvas: `toFlowEdges` maps `condition_label` → React Flow `label` + `data.conditionLabel`.
- [x] Canvas: `fromFlowGraph` restores `condition_label` from `data.conditionLabel`.
- [x] Canvas: `onConnect` auto-assigns "true" then "false" when connecting from a condition node.
- [x] Canvas: `onEdgeClick` toggles condition label on edges leaving a condition node.
- [x] Canvas: `onEdgesUpdated` callback added so WorkflowEditor tracks edge state for saving.
- [x] Canvas edge sync bug fixed: node and edge effects separated so node drags no longer reset edge state.
- [x] `exec-waiting_approval` border pulse animation added to CSS.
- [x] 61 Rust tests passing, TypeScript zero errors.

## Routing logic

For each node (in topological order):
1. Collect all incoming edges `(source, condition_label)`.
2. An edge is **inactive** if:
   - its source was skipped, OR
   - it has a `condition_label` that doesn't match `condition_results[source]`.
3. If the node has ≥1 incoming edge AND all are inactive → skip.
4. After a condition node succeeds: parse `{"result": <bool>}` from output and record in `condition_results`.

## UX

- Edges drawn from a condition node auto-label: first edge gets "true", second gets "false".
- Clicking an existing condition-node edge cycles the label: none → "true" → "false" → none.
- Edge labels rendered with React Flow's built-in `label` field.
