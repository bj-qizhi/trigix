# Execution Reliability and Tenant Isolation

## Queue delivery

Redis Streams execution delivery is at-least-once. Workers first reclaim stale
pending entries with `XAUTOCLAIM`, then read new entries with `XREADGROUP`.
`EXEC_QUEUE_RECLAIM_IDLE_MS` controls the stale threshold and defaults to 15
minutes. `EXEC_QUEUE_MAX_RECOVERIES` defaults to 3; jobs exceeding the limit are
written to `af:exec:queue:dead` before the source entry is acknowledged.

Execution terminal state is the idempotency boundary. A reclaimed job whose
Execution is already Succeeded, Failed, or Cancelled is acknowledged without
running Nodes again. State transitions are defined centrally by
`execution_core::ExecutionStatus`.

While a worker owns a long-running job it refreshes the Redis pending-entry idle
time with an `XCLAIM` lease heartbeat. This prevents healthy executions from
being reclaimed merely because a Node runs longer than the reclaim threshold.
PostgreSQL completion uses a compare-and-set update; a concurrent cancellation
or failure wins and late Node results are not persisted over that terminal state.
Successful terminal transitions are also appended to
`af_execution_state_transitions`, providing a durable state-change history for
recovery analysis and audit replay.

## Node handler boundary

`NodeHandlerRegistry` is the Executor's dispatch interface. The built-in
implementation owns the production Node handlers, while runtime policy remains
separate metadata. Retry, timeout, caching, Approval, Wait, and dry-run wrap the
registry rather than being duplicated in individual integrations.

Operators should set the reclaim threshold above the expected maximum healthy
Execution duration. Long-running Approval and Wait Nodes require particular
care because process-local suspension cannot survive a process restart yet.

## Tenant isolation

Every Store query must continue binding `tenant_id`; callers must treat a record
owned by another Tenant as Not Found. Migration `0068_tenant_row_level_security`
adds PostgreSQL Row Level Security policies to every public table containing a
`tenant_id` column.

PostgreSQL integration tests verify that every tenant-bearing table has the
`tenant_isolation` policy after migrations run. Store and HTTP tests separately
verify that a Tenant cannot retrieve another Tenant's records.

For the database policies to enforce isolation, production must connect through
a non-owner application role and set `app.tenant_id` for each transaction. Table
owners retain PostgreSQL's normal RLS bypass so schema migration remains
possible. Application filters and RLS are complementary controls, not substitutes.
