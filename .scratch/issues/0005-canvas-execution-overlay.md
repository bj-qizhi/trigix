# Canvas Execution Overlay

## Status

done

## Category

enhancement

## What to build

Visual execution feedback on the workflow canvas: node status badges, colored borders, highlighted edges, and node output shown in the config panel when a node is selected.

## Acceptance criteria

- [x] Node border color reflects execution status: green (succeeded), red (failed), blue animated (running), gray (skipped).
- [x] Status dot badge (8px circle) in top-right corner of each node showing current execution status.
- [x] Edge color follows the source node status: green for succeeded, red for failed, animated blue for running.
- [x] Clicking a node while an execution is loaded shows "Last result" box at the top of the config panel.
- [x] Result box shows: status badge, output JSON (pretty-printed), or error message.
- [x] `NodeStatusContext` React context passes statuses to `FlowNodeComponent` without touching node positions.
- [x] `nodeStatuses` prop on `Canvas` accepts `Record<string, NodeExecutionRecord>`.
- [x] TypeScript zero errors, Vite production build successful.

## Notes

- Context approach avoids re-triggering the canvas sync effect that resets node positions.
- `displayEdges` derived with `useMemo` from edge state + `nodeStatuses`; does not mutate stored edges.
