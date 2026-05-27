# Real Node Execution

## Status

in-progress

## Category

enhancement

## What to build

Replace the placeholder `EchoNodeExecutor` with a dispatching executor that runs each node type with real logic. Wire the Python AI Runtime to execute Agent nodes using the Claude API. This proves the full execution path from Platform → Executor → AI Runtime → LLM.

## Acceptance criteria

- [ ] `Node` carries an optional `config` JSON field that node executors can read.
- [ ] `NodeExecutor` trait and `run_workflow` are async, enabling real I/O in node handlers.
- [ ] `DispatchingNodeExecutor` routes by node type (Trigger, Http, Agent, Condition).
- [ ] Trigger node returns the workflow input as its output.
- [ ] HTTP node makes a real outbound HTTP call using config `url`, `method`, `headers`, `body`.
- [ ] Agent node calls the Python AI Runtime `/v1/nodes/agent` endpoint.
- [ ] Python AI Runtime calls the Claude API and returns structured output.
- [ ] Executor service reads `AI_RUNTIME_BASE_URL` from environment to wire Agent nodes.
- [ ] All existing tests pass; new tests cover each node type.

## Notes

- `InlineExecutorClient` in the Platform service keeps `EchoNodeExecutor` for embedded dev/test use. Real node execution happens in the standalone Executor service.
- The Python AI Runtime uses `ANTHROPIC_API_KEY` from environment.
- HTTP node templating (variable substitution in URL/body) is deferred to the next slice.
- Condition node branching (conditional edges) requires graph model changes and is deferred.

## Progress

- Node config field added to workflow-core.
- NodeExecutor and run_workflow made async.
- DispatchingNodeExecutor implemented with Trigger, Http, Agent, Condition handlers.
- Python AI Runtime `/v1/nodes/agent` endpoint implemented.
- Executor service wires AI_RUNTIME_BASE_URL.
