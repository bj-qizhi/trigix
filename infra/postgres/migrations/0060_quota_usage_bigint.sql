-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- The platform models quotas and usage counters as i64, but several columns were
-- declared INTEGER (int4). The enterprise/unlimited tier uses i64::MAX (overflows
-- int4), and sqlx decodes these columns into i64 (which only accepts int8), so
-- reads of an int4 column fail and fall back to defaults. The net effect on
-- Postgres: paid quota upgrades silently did not persist, enterprise quotas
-- errored, and execution usage always read back as zero.
--
-- Widen the affected columns to BIGINT to match the Rust types. ALTER ... TYPE
-- BIGINT is a no-op when a column is already bigint, so this is safe to re-run.

ALTER TABLE af_tenant_quotas
    ALTER COLUMN max_executions_per_month  TYPE BIGINT,
    ALTER COLUMN max_concurrent_executions TYPE BIGINT,
    ALTER COLUMN max_workflows             TYPE BIGINT;

ALTER TABLE af_usage_ledger
    ALTER COLUMN executions_used TYPE BIGINT;
