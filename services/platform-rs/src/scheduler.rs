// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub tenant_id: String,
    pub interval_secs: u64,
    pub cron_expression: Option<String>,
    pub next_run_at: Instant,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleSummary {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub interval_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_expression: Option<String>,
    pub secs_until_next_run: u64,
    pub paused: bool,
}

impl ScheduleSummary {
    fn from_entry(entry: &ScheduleEntry) -> Self {
        let secs_until_next_run = entry
            .next_run_at
            .checked_duration_since(Instant::now())
            .unwrap_or(Duration::ZERO)
            .as_secs();
        Self {
            workflow_id: entry.workflow_id.clone(),
            workflow_version_id: entry.workflow_version_id.clone(),
            interval_secs: entry.interval_secs,
            cron_expression: entry.cron_expression.clone(),
            secs_until_next_run,
            paused: entry.paused,
        }
    }
}

#[derive(Clone, Default)]
pub struct ScheduleStore {
    entries: Arc<Mutex<HashMap<String, ScheduleEntry>>>,
}

impl ScheduleStore {
    pub fn register(&self, entry: ScheduleEntry) {
        let mut map = self.entries.lock().unwrap();
        map.insert(entry.workflow_version_id.clone(), entry);
    }

    pub fn unregister(&self, workflow_version_id: &str) -> bool {
        self.entries.lock().unwrap().remove(workflow_version_id).is_some()
    }

    pub fn list(&self, tenant_id: &str) -> Vec<ScheduleSummary> {
        let map = self.entries.lock().unwrap();
        let mut out: Vec<ScheduleSummary> = map
            .values()
            .filter(|e| e.tenant_id == tenant_id)
            .map(ScheduleSummary::from_entry)
            .collect();
        out.sort_by(|a, b| a.workflow_id.cmp(&b.workflow_id));
        out
    }

    /// Returns entries whose `next_run_at` has elapsed and advances them to their next scheduled time.
    /// Skips paused entries.
    pub fn take_due(&self) -> Vec<ScheduleEntry> {
        let now = Instant::now();
        let mut map = self.entries.lock().unwrap();
        let mut due = Vec::new();
        for entry in map.values_mut() {
            if !entry.paused && entry.next_run_at <= now {
                due.push(entry.clone());
                entry.next_run_at = next_run_after_now(entry, now);
            }
        }
        due
    }

    pub fn set_paused(&self, workflow_version_id: &str, paused: bool) -> bool {
        let mut map = self.entries.lock().unwrap();
        if let Some(entry) = map.get_mut(workflow_version_id) {
            entry.paused = paused;
            true
        } else {
            false
        }
    }
}

fn next_run_after_now(entry: &ScheduleEntry, now: Instant) -> Instant {
    if let Some(expr) = &entry.cron_expression {
        if let Some(instant) = cron_next_instant(expr) {
            return instant;
        }
    }
    now + Duration::from_secs(entry.interval_secs)
}

// ── Instant ↔ Unix helpers ────────────────────────────────────────────────────

fn instant_to_unix(instant: Instant) -> i64 {
    let now_instant = Instant::now();
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let diff = if instant >= now_instant {
        instant.duration_since(now_instant).as_secs() as i64
    } else {
        -(now_instant.duration_since(instant).as_secs() as i64)
    };
    now_unix + diff
}

fn unix_to_instant(unix: i64) -> Instant {
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let diff = unix - now_unix;
    if diff >= 0 {
        Instant::now() + Duration::from_secs(diff as u64)
    } else {
        Instant::now()
            .checked_sub(Duration::from_secs((-diff) as u64))
            .unwrap_or(Instant::now())
    }
}

// ── Platform store (Memory or Postgres write-through) ─────────────────────────

/// Write-through scheduler store.
/// Memory is the source of truth for `take_due` / `list` (fast, no lock contention).
/// `register` / `unregister` additionally persist to Postgres so schedules survive restart.
#[derive(Clone)]
pub enum PlatformScheduleStore {
    Memory(ScheduleStore),
    Postgres { memory: ScheduleStore, pool: PgPool },
}

impl Default for PlatformScheduleStore {
    fn default() -> Self {
        Self::Memory(ScheduleStore::default())
    }
}

impl PlatformScheduleStore {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres { memory: ScheduleStore::default(), pool }
    }

    /// Load persisted entries from Postgres into the in-memory cache on startup.
    pub async fn bootstrap_from_postgres(&self) {
        let Self::Postgres { memory, pool } = self else { return };

        #[derive(sqlx::FromRow)]
        struct Row {
            workflow_id: String,
            workflow_version_id: String,
            tenant_id: String,
            interval_secs: i64,
            cron_expression: Option<String>,
            next_run_unix: i64,
            #[sqlx(default)]
            paused: bool,
        }

        let rows = sqlx::query_as::<_, Row>(
            "SELECT workflow_id, workflow_version_id, tenant_id, interval_secs,
                    cron_expression, EXTRACT(EPOCH FROM next_run_at)::bigint AS next_run_unix,
                    paused
             FROM af_schedules",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for row in rows {
            memory.register(ScheduleEntry {
                workflow_id: row.workflow_id,
                workflow_version_id: row.workflow_version_id,
                tenant_id: row.tenant_id,
                interval_secs: row.interval_secs as u64,
                cron_expression: row.cron_expression,
                next_run_at: unix_to_instant(row.next_run_unix),
                paused: row.paused,
            });
        }
    }

    pub fn register(&self, entry: ScheduleEntry) {
        match self {
            Self::Memory(s) => s.register(entry),
            Self::Postgres { memory, pool } => {
                memory.register(entry.clone());
                let pool = pool.clone();
                let next_unix = instant_to_unix(entry.next_run_at) as f64;
                tokio::spawn(async move {
                    sqlx::query(
                        "INSERT INTO af_schedules
                             (workflow_version_id, workflow_id, tenant_id, interval_secs, cron_expression, next_run_at, paused)
                         VALUES ($1, $2, $3, $4, $5, to_timestamp($6), $7)
                         ON CONFLICT (workflow_version_id) DO UPDATE
                             SET workflow_id = EXCLUDED.workflow_id,
                                 tenant_id = EXCLUDED.tenant_id,
                                 interval_secs = EXCLUDED.interval_secs,
                                 cron_expression = EXCLUDED.cron_expression,
                                 next_run_at = EXCLUDED.next_run_at,
                                 paused = EXCLUDED.paused",
                    )
                    .bind(&entry.workflow_version_id)
                    .bind(&entry.workflow_id)
                    .bind(&entry.tenant_id)
                    .bind(entry.interval_secs as i64)
                    .bind(&entry.cron_expression)
                    .bind(next_unix)
                    .bind(entry.paused)
                    .execute(&pool)
                    .await
                    .ok();
                });
            }
        }
    }

    pub fn unregister(&self, workflow_version_id: &str) -> bool {
        match self {
            Self::Memory(s) => s.unregister(workflow_version_id),
            Self::Postgres { memory, pool } => {
                let removed = memory.unregister(workflow_version_id);
                if removed {
                    let pool = pool.clone();
                    let vid = workflow_version_id.to_string();
                    tokio::spawn(async move {
                        sqlx::query(
                            "DELETE FROM af_schedules WHERE workflow_version_id = $1",
                        )
                        .bind(vid)
                        .execute(&pool)
                        .await
                        .ok();
                    });
                }
                removed
            }
        }
    }

    pub fn list(&self, tenant_id: &str) -> Vec<ScheduleSummary> {
        match self {
            Self::Memory(s) => s.list(tenant_id),
            Self::Postgres { memory, .. } => memory.list(tenant_id),
        }
    }

    pub fn set_paused(&self, workflow_version_id: &str, paused: bool) -> bool {
        match self {
            Self::Memory(s) => s.set_paused(workflow_version_id, paused),
            Self::Postgres { memory, pool } => {
                let changed = memory.set_paused(workflow_version_id, paused);
                if changed {
                    let pool = pool.clone();
                    let vid = workflow_version_id.to_string();
                    tokio::spawn(async move {
                        sqlx::query(
                            "UPDATE af_schedules SET paused = $1 WHERE workflow_version_id = $2",
                        )
                        .bind(paused)
                        .bind(vid)
                        .execute(&pool)
                        .await
                        .ok();
                    });
                }
                changed
            }
        }
    }

    /// Returns due entries. The Postgres variant uses `SELECT … FOR UPDATE SKIP LOCKED`
    /// so only one instance processes each entry — safe for multi-instance deployments.
    pub async fn take_due(&self) -> Vec<ScheduleEntry> {
        match self {
            Self::Memory(s) => s.take_due(),
            Self::Postgres { memory, pool } => {
                take_due_postgres(memory, pool).await
            }
        }
    }
}

/// Atomically claims due schedule entries from Postgres using `SELECT … FOR UPDATE SKIP LOCKED`.
/// Guarantees each entry is processed by exactly one instance in a multi-instance deployment.
/// Also syncs claimed entries back into the in-memory cache.
async fn take_due_postgres(memory: &ScheduleStore, pool: &PgPool) -> Vec<ScheduleEntry> {
    #[derive(sqlx::FromRow)]
    struct DueRow {
        workflow_id: String,
        workflow_version_id: String,
        tenant_id: String,
        interval_secs: i64,
        cron_expression: Option<String>,
        #[sqlx(default)]
        paused: bool,
    }

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(_) => return memory.take_due(), // fall back to in-memory on error
    };

    let rows = sqlx::query_as::<_, DueRow>(
        r#"
        SELECT workflow_id, workflow_version_id, tenant_id, interval_secs, cron_expression, paused
        FROM af_schedules
        WHERE next_run_at <= NOW() AND paused = FALSE
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        let _ = tx.commit().await;
        return Vec::new();
    }

    let now = Instant::now();
    let mut entries = Vec::with_capacity(rows.len());

    for row in rows {
        let mut entry = ScheduleEntry {
            workflow_id: row.workflow_id,
            workflow_version_id: row.workflow_version_id,
            tenant_id: row.tenant_id,
            interval_secs: row.interval_secs as u64,
            cron_expression: row.cron_expression,
            next_run_at: now, // will be updated below
            paused: row.paused,
        };
        entry.next_run_at = next_run_after_now(&entry, now);

        let next_unix = instant_to_unix(entry.next_run_at) as f64;
        let _ = sqlx::query(
            "UPDATE af_schedules SET next_run_at = to_timestamp($1) WHERE workflow_version_id = $2",
        )
        .bind(next_unix)
        .bind(&entry.workflow_version_id)
        .execute(&mut *tx)
        .await;

        // Keep in-memory cache in sync
        memory.register(entry.clone());
        entries.push(entry);
    }

    if tx.commit().await.is_err() {
        return Vec::new(); // transaction failed; other instance will retry
    }

    entries
}

/// Parses a cron expression and returns an Instant for the next scheduled occurrence.
/// Supports both 5-field (standard) and 7-field (with seconds + year) expressions.
pub fn cron_next_instant(expr: &str) -> Option<Instant> {
    use chrono::Utc;
    let schedule = cron::Schedule::from_str(expr).ok()?;
    let next = schedule.upcoming(Utc).next()?;
    let duration_until = (next - Utc::now()).to_std().ok()?;
    Some(Instant::now() + duration_until)
}

/// Parses a cron expression and returns a Duration until the next occurrence (for display).
pub fn cron_secs_until_next(expr: &str) -> Option<u64> {
    use chrono::Utc;
    let schedule = cron::Schedule::from_str(expr).ok()?;
    let next = schedule.upcoming(Utc).next()?;
    let duration_until = (next - Utc::now()).to_std().ok()?;
    Some(duration_until.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_list() {
        let store = ScheduleStore::default();
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 3600,
            cron_expression: None,
            next_run_at: Instant::now() + Duration::from_secs(3600),
            paused: false,
        });

        let list = store.list("t1");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].workflow_id, "wf-1");
        assert_eq!(list[0].interval_secs, 3600);

        // Other tenant sees nothing
        assert!(store.list("t2").is_empty());
    }

    #[test]
    fn unregister_removes_entry() {
        let store = ScheduleStore::default();
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 60,
            cron_expression: None,
            next_run_at: Instant::now() + Duration::from_secs(60),
            paused: false,
        });
        store.unregister("v-1");
        assert!(store.list("t1").is_empty());
    }

    #[test]
    fn take_due_returns_and_bumps_overdue_entry() {
        let store = ScheduleStore::default();
        // next_run_at in the past → immediately due
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 3600,
            cron_expression: None,
            next_run_at: Instant::now() - Duration::from_secs(1),
            paused: false,
        });

        let due = store.take_due();
        assert_eq!(due.len(), 1);

        // After take_due, the entry is no longer due (next_run bumped by 3600s).
        let due_again = store.take_due();
        assert!(due_again.is_empty());
    }

    #[test]
    fn cron_next_instant_parses_valid_expression() {
        // Every minute — should produce an instant within the next 60s
        let instant = cron_next_instant("0 * * * * * *");
        assert!(instant.is_some());
        let secs = instant.unwrap().duration_since(Instant::now()).as_secs();
        assert!(secs <= 60, "next run should be within 60 seconds");
    }

    #[test]
    fn cron_next_instant_returns_none_for_invalid_expression() {
        let instant = cron_next_instant("not-a-cron");
        assert!(instant.is_none());
    }

    #[test]
    fn paused_entry_skipped_by_take_due() {
        let store = ScheduleStore::default();
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 60,
            cron_expression: None,
            next_run_at: Instant::now() - Duration::from_secs(1),
            paused: false,
        });

        // Pause the schedule
        assert!(store.set_paused("v-1", true));
        let due = store.take_due();
        assert!(due.is_empty(), "paused entries should not be returned by take_due");

        // Resume and verify it fires again
        store.set_paused("v-1", false);
        let due = store.take_due();
        assert_eq!(due.len(), 1);
    }

    #[test]
    fn set_paused_returns_false_for_unknown_entry() {
        let store = ScheduleStore::default();
        assert!(!store.set_paused("nonexistent", true));
    }

    #[test]
    fn paused_flag_reflected_in_summary() {
        let store = ScheduleStore::default();
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 60,
            cron_expression: None,
            next_run_at: Instant::now() + Duration::from_secs(60),
            paused: false,
        });
        assert!(!store.list("t1")[0].paused);
        store.set_paused("v-1", true);
        assert!(store.list("t1")[0].paused);
    }

    #[tokio::test]
    async fn platform_memory_take_due_is_async_compatible() {
        let platform = PlatformScheduleStore::Memory(ScheduleStore::default());
        if let PlatformScheduleStore::Memory(ref inner) = platform {
            inner.register(ScheduleEntry {
                workflow_id: "wf-1".into(),
                workflow_version_id: "v-async".into(),
                tenant_id: "t1".into(),
                interval_secs: 3600,
                cron_expression: None,
                next_run_at: Instant::now() - Duration::from_secs(1),
                paused: false,
            });
        }
        let due = platform.take_due().await;
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].workflow_version_id, "v-async");
        // Second call: entry is bumped, not due again
        assert!(platform.take_due().await.is_empty());
    }
}
