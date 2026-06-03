// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Data retention — periodically deletes execution / audit / usage rows older
//! than a configurable window so the unbounded tables don't grow forever.
//!
//! Opt-in: disabled unless `DATA_RETENTION_DAYS` is set to a positive value.
//! Sweep interval is configurable via `DATA_RETENTION_SWEEP_SECS` (default 6h).

use sqlx::PgPool;
use std::time::Duration;

const DEFAULT_SWEEP_SECS: u64 = 21_600; // 6 hours
const SECS_PER_DAY: u64 = 86_400;

/// Read the configured retention window from `DATA_RETENTION_DAYS`.
/// Returns 0 (disabled) when unset or unparseable.
pub fn retention_days_from_env() -> u64 {
    std::env::var("DATA_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Spawn the background retention sweeper. No-op when `days == 0`.
pub fn spawn_data_retention(pool: PgPool, days: u64) {
    if days == 0 {
        tracing::info!("Data retention disabled (set DATA_RETENTION_DAYS to enable)");
        return;
    }
    let sweep_secs = std::env::var("DATA_RETENTION_SWEEP_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SWEEP_SECS);
    tracing::info!(days, sweep_secs, "Data retention enabled");

    tokio::spawn(async move {
        // Small initial delay so the first sweep doesn't compete with startup.
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            match run_retention_pass(&pool, days).await {
                Ok(total) if total > 0 => {
                    tracing::info!(rows = total, days, "Data retention pass complete")
                }
                Ok(_) => {}
                Err(e) => tracing::error!(error = %e, "Data retention pass failed"),
            }
            tokio::time::sleep(Duration::from_secs(sweep_secs)).await;
        }
    });
}

/// Delete rows older than `days` across the unbounded tables. Returns total rows removed.
pub async fn run_retention_pass(pool: &PgPool, days: u64) -> Result<u64, sqlx::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff_unix = now.saturating_sub(days.saturating_mul(SECS_PER_DAY)) as i64;
    let interval = format!("{days} days");
    let mut total = 0u64;

    // Child rows first: af_node_executions has a FK to af_executions with no cascade.
    total += delete(
        pool,
        "af_node_executions",
        sqlx::query(
            "DELETE FROM af_node_executions WHERE execution_id IN \
             (SELECT id FROM af_executions WHERE started_at < $1)",
        )
        .bind(cutoff_unix),
    )
    .await?;

    // Unix-seconds (BIGINT) timestamp tables.
    total += delete(
        pool,
        "af_executions",
        sqlx::query("DELETE FROM af_executions WHERE started_at < $1").bind(cutoff_unix),
    )
    .await?;
    total += delete(
        pool,
        "af_token_usage",
        sqlx::query("DELETE FROM af_token_usage WHERE created_at < $1").bind(cutoff_unix),
    )
    .await?;

    // TIMESTAMPTZ tables use an interval cutoff relative to now().
    total += delete(
        pool,
        "af_audit_log",
        sqlx::query("DELETE FROM af_audit_log WHERE created_at < now() - $1::interval")
            .bind(&interval),
    )
    .await?;
    total += delete(
        pool,
        "af_webhook_deliveries",
        sqlx::query("DELETE FROM af_webhook_deliveries WHERE delivered_at < now() - $1::interval")
            .bind(&interval),
    )
    .await?;

    Ok(total)
}

async fn delete<'q>(
    pool: &PgPool,
    table: &str,
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
) -> Result<u64, sqlx::Error> {
    let rows = query.execute(pool).await?.rows_affected();
    if rows > 0 {
        tracing::info!(table, rows, "retention: deleted rows");
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    /// Live integration test against a real Postgres. Ignored by default (CI has
    /// no DB); run with: `RETENTION_TEST_DATABASE_URL=... cargo test -p
    /// trigix-platform retention -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn retention_pass_deletes_only_old_rows() {
        let url = match std::env::var("RETENTION_TEST_DATABASE_URL") {
            Ok(u) => u,
            Err(_) => return,
        };
        let pool = PgPool::connect(&url).await.expect("connect test db");
        let p = format!("rtn-{}", uuid::Uuid::new_v4());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let old = now - 100 * 86_400; // 100 days ago → beyond a 90-day window

        // af_executions (+ child af_node_executions) — one old, one recent.
        for (suffix, ts) in [("old", old), ("new", now)] {
            let exec_id = format!("{p}-exec-{suffix}");
            sqlx::query(
                "INSERT INTO af_executions (id, tenant_id, workflow_id, workflow_version_id, \
                 status, input_json, graph_json, started_at, dry_run, starred, node_count, \
                 completed_node_count) VALUES ($1,'t','w','v','succeeded','{}'::jsonb,\
                 '{}'::jsonb,$2,false,false,0,0)",
            )
            .bind(&exec_id)
            .bind(ts)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO af_node_executions (id, tenant_id, execution_id, node_id, \
                 node_type, status, duration_ms, started_at_ms) \
                 VALUES ($1,'t',$2,'n','http','succeeded',1,$3)",
            )
            .bind(format!("{p}-node-{suffix}"))
            .bind(&exec_id)
            .bind(ts * 1000)
            .execute(&pool)
            .await
            .unwrap();

            sqlx::query(
                "INSERT INTO af_token_usage (id, tenant_id, execution_id, node_id, model, \
                 prompt_tokens, completion_tokens, total_tokens, created_at) \
                 VALUES ($1,'t',$2,'n','gpt',1,1,2,$3)",
            )
            .bind(format!("{p}-tok-{suffix}"))
            .bind(&exec_id)
            .bind(ts)
            .execute(&pool)
            .await
            .unwrap();
        }

        // TIMESTAMPTZ tables — one old (100 days), one recent.
        for (suffix, interval) in [("old", "100 days"), ("new", "0 days")] {
            sqlx::query(
                "INSERT INTO af_audit_log (id, tenant_id, action, resource_type, resource_id, \
                 created_at) VALUES ($1,'t','a','r','r', now() - $2::interval)",
            )
            .bind(format!("{p}-aud-{suffix}"))
            .bind(interval)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO af_webhook_deliveries (id, webhook_token, tenant_id, delivered_at, \
                 success) VALUES ($1,'tok','t', now() - $2::interval, true)",
            )
            .bind(format!("{p}-whd-{suffix}"))
            .bind(interval)
            .execute(&pool)
            .await
            .unwrap();
        }

        // Run retention with a 90-day window.
        run_retention_pass(&pool, 90).await.expect("retention pass");

        // Helper: does a row with this id still exist in `table`?
        async fn exists(pool: &PgPool, table: &str, id: &str) -> bool {
            let sql = format!("SELECT count(*) AS c FROM {table} WHERE id = $1");
            let row = sqlx::query(&sql).bind(id).fetch_one(pool).await.unwrap();
            row.get::<i64, _>("c") > 0
        }

        // Old rows gone, recent rows kept — across all five tables.
        for (table, kind) in [
            ("af_executions", "exec"),
            ("af_node_executions", "node"),
            ("af_token_usage", "tok"),
        ] {
            assert!(
                !exists(&pool, table, &format!("{p}-{kind}-old")).await,
                "{table}: old row should be deleted"
            );
            assert!(
                exists(&pool, table, &format!("{p}-{kind}-new")).await,
                "{table}: recent row should survive"
            );
        }
        assert!(!exists(&pool, "af_audit_log", &format!("{p}-aud-old")).await);
        assert!(exists(&pool, "af_audit_log", &format!("{p}-aud-new")).await);
        assert!(!exists(&pool, "af_webhook_deliveries", &format!("{p}-whd-old")).await);
        assert!(exists(&pool, "af_webhook_deliveries", &format!("{p}-whd-new")).await);

        // Cleanup the surviving test rows.
        for (table, kind) in [
            ("af_node_executions", "node"),
            ("af_token_usage", "tok"),
            ("af_executions", "exec"),
            ("af_audit_log", "aud"),
            ("af_webhook_deliveries", "whd"),
        ] {
            let sql = format!("DELETE FROM {table} WHERE id LIKE $1");
            sqlx::query(&sql)
                .bind(format!("{p}-{kind}-%"))
                .execute(&pool)
                .await
                .unwrap();
        }
    }
}
