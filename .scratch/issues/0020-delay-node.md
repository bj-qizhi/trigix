# Delay Node

## Status

done

## Category

enhancement

## What to build

Add a `delay` node type that pauses workflow execution for a configurable number of seconds before proceeding. Enables rate-limiting between nodes, simulated pacing, and timed workflows without external dependencies.

## Acceptance criteria

- [x] `NodeType::Delay` added to `crates/workflow-core/src/graph.rs`.
- [x] `execute_delay(node) -> NodeExecutionResult` (async) in `services/executor/src/executor.rs`:
  - Config: `seconds` (u64, capped at 3600 = 1 hour).
  - Calls `tokio::time::sleep(Duration::from_secs(seconds))` if seconds > 0.
  - Returns `{ "waited_secs": N }`.
  - Fails if config absent or `seconds` key missing.
- [x] `NodeType::Delay` arm added to `dispatch` match in `executor.rs`.
- [x] `node_type_to_str` in `execution.rs` updated with `"delay"`.
- [x] 2 executor unit tests: `delay_node_zero_seconds_completes_immediately`, `delay_node_fails_without_seconds_config`.
- [x] Frontend: `'delay'` added to `NodeType` union in `types/index.ts`.
- [x] CSS: `--node-delay: #b45309` variable + `.flow-node-delay .flow-node-header` rule.
- [x] Canvas: `NODE_LABELS['delay'] = 'Delay'`, `NODE_ICONS['delay'] = '⏱'`, preview shows `wait Ns`, `nodeTypes` includes `delay`, MiniMap color `'#b45309'`.
- [x] `WorkflowEditor` palette: `{ type: 'delay', label: 'Delay', color: 'var(--node-delay)', icon: '⏱' }`.
- [x] `NodeConfigPanel`: `DelayConfig` component — number input (0–3600), reads via `num('seconds', 0)`, writes via `set('seconds', value)`, output format hint.
- [x] 91 Rust tests (32 executor + 55 platform + 4 workflow-core), 0 TypeScript errors.
