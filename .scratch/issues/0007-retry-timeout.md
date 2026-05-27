# Retry and Timeout per Node

## Status

done

## Category

enhancement

## What to build

Allow HTTP and Agent nodes to be configured with `max_retries` (0–5) and `timeout_secs` for production-grade resilience against transient failures.

## Acceptance criteria

- [x] `max_retries` in node config: executor retries the node up to N times on failure before giving up.
- [x] Exponential backoff between retries: 200ms × 2^attempt, capped at ~6.4s per interval.
- [x] `timeout_secs` in node config: each attempt is individually wrapped with `tokio::time::timeout`; on expiry, node is marked Failed with "timed out after Ns".
- [x] Retry + timeout compose correctly: a timed-out attempt counts as one failure before the next retry.
- [x] Trigger and Condition nodes work correctly when `max_retries`/`timeout_secs` are set (no-op for fast nodes).
- [x] `dispatch_with_timeout()` and `dispatch()` free functions split routing from retry logic.
- [x] `node_config_u64()` helper reads u64 from node config safely.
- [x] Frontend: Retries (0–5) and Timeout (s) fields added to HTTP and Agent config panels.
- [x] `num` added to shared `ConfigProps` interface; AgentConfig no longer needs intersection type.
- [x] 4 new Rust tests: config extraction, zero retries, failing node + retries, fast node + timeout config.
- [x] 55 Rust tests passing, TypeScript zero errors.

## Node config reference (updated)

| Node type | Optional config |
|---|---|
| `http` | `url`*, `method`, `headers`, `body`, **`max_retries`**, **`timeout_secs`** |
| `agent` | `model`, `system_prompt`, `prompt_template`, `max_tokens`, **`max_retries`**, **`timeout_secs`** |
