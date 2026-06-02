# PostgreSQL Persistence

## Status

done

## Category

infrastructure / P0

## What to build

Wire up the existing PostgreSQL stores so that the platform persists data across restarts when `DATABASE_URL` is set. Keep in-memory stores as the default for development/testing without a database.

## Acceptance criteria

- [x] `infra/postgres/migrations/0003_af_stores.sql` — new TEXT-keyed tables (`af_workflows`, `af_workflow_versions`, `af_executions`, `af_node_executions`) compatible with the current string-ID model. Indexes on tenant + timestamp lookups.
- [x] `infra/postgres/migrations/0004_af_dev_seed.sql` — dev seed mirroring the in-memory seed (`workflow-1`, `tenant-1`, etc.) so frontend constants work unchanged.
- [x] `services/platform-rs/Cargo.toml` — added `"migrate"` feature to sqlx.
- [x] `services/platform-rs/src/main.rs` — runs `sqlx::migrate!("../../infra/postgres/migrations").run(&pool)` at startup before serving requests.
- [x] `PostgresWorkflowVersionStore` — all queries ported from UUID-keyed `workflows`/`workflow_versions` to TEXT-keyed `af_workflows`/`af_workflow_versions`; all `::uuid` casts removed.
- [x] `PostgresExecutionStore` — all queries ported to `af_executions`/`af_node_executions`; inline `graph_json` storage (no JOIN to workflow_versions); real `started_at`/`finished_at` as BIGINT unix timestamps; `error` column is plain TEXT (not JSONB).
- [x] `PostgresExecutionRow` — added `started_at: i64`, `finished_at: Option<i64>` fields; `try_into_record` uses real timestamps.
- [x] `PostgresExecutionSummaryRow` — added `started_at: i64`; `try_into_summary` no longer hardcodes `0`.
- [x] `PostgresNodeExecutionRow` — uses `error: Option<String>` instead of `error_json`; `try_into_record` simplified.
- [x] 109 Rust tests (48 + 57 + 4), 0 compile errors. All existing tests use in-memory stores and pass unchanged.

## Architecture decision

Created new `af_*` tables with TEXT PKs rather than altering the UUID-keyed tables from migration 0001. Rationale:
- Avoids UUID casting failures for existing string IDs (`tenant-1`, `workflow-1`)  
- Keeps the UUID infrastructure tables intact for future auth/multi-tenant work
- Allows zero-change migration path: existing deployments just get new tables

## How to activate PostgreSQL mode

```bash
docker-compose up -d postgres
export DATABASE_URL=postgres://velara:velara@localhost:35432/velara
cargo run -p velara-platform
```

On first run, migrations execute automatically and the dev seed populates `af_workflows` / `af_workflow_versions`.
