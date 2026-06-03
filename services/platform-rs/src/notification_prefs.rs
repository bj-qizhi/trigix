// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPrefs {
    pub user_id: String,
    /// Send email when an execution the user owns fails.
    #[serde(default)]
    pub email_on_failure: bool,
    /// Send email when an execution the user owns succeeds.
    #[serde(default)]
    pub email_on_success: bool,
}

impl NotificationPrefs {
    pub fn default_for(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_owned(),
            email_on_failure: false,
            email_on_success: false,
        }
    }
}

pub trait NotificationPrefsStore: Send + Sync {
    fn get(&self, user_id: &str) -> NotificationPrefs;
    fn upsert(&self, prefs: NotificationPrefs);
}

// ── Memory store ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct MemoryNotificationPrefsStore {
    data: RwLock<HashMap<String, NotificationPrefs>>,
}

impl NotificationPrefsStore for MemoryNotificationPrefsStore {
    fn get(&self, user_id: &str) -> NotificationPrefs {
        self.data
            .read()
            .unwrap()
            .get(user_id)
            .cloned()
            .unwrap_or_else(|| NotificationPrefs::default_for(user_id))
    }

    fn upsert(&self, prefs: NotificationPrefs) {
        self.data
            .write()
            .unwrap()
            .insert(prefs.user_id.clone(), prefs);
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

pub struct PostgresNotificationPrefsStore {
    pool: sqlx::PgPool,
}

impl PostgresNotificationPrefsStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct PrefsRow {
    user_id: String,
    #[sqlx(default)]
    email_on_failure: bool,
    #[sqlx(default)]
    email_on_success: bool,
}

impl NotificationPrefsStore for PostgresNotificationPrefsStore {
    fn get(&self, user_id: &str) -> NotificationPrefs {
        let pool = self.pool.clone();
        let uid = user_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, PrefsRow>(
                    "SELECT user_id, email_on_failure, email_on_success \
                     FROM af_notification_prefs WHERE user_id = $1",
                )
                .bind(&uid)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .map(|r| NotificationPrefs {
                    user_id: r.user_id,
                    email_on_failure: r.email_on_failure,
                    email_on_success: r.email_on_success,
                })
                .unwrap_or_else(|| NotificationPrefs::default_for(&uid))
            })
        })
    }

    fn upsert(&self, prefs: NotificationPrefs) {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let _ = sqlx::query(
                    "INSERT INTO af_notification_prefs (user_id, email_on_failure, email_on_success) \
                     VALUES ($1, $2, $3) \
                     ON CONFLICT (user_id) DO UPDATE SET email_on_failure = EXCLUDED.email_on_failure, \
                     email_on_success = EXCLUDED.email_on_success"
                )
                .bind(&prefs.user_id)
                .bind(prefs.email_on_failure)
                .bind(prefs.email_on_success)
                .execute(&pool).await;
            })
        })
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

pub enum PlatformNotificationPrefsStore {
    Memory(Arc<MemoryNotificationPrefsStore>),
    Postgres(PostgresNotificationPrefsStore),
}

impl PlatformNotificationPrefsStore {
    pub fn memory() -> Self {
        Self::Memory(Arc::new(MemoryNotificationPrefsStore::default()))
    }
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self::Postgres(PostgresNotificationPrefsStore::new(pool))
    }
}

impl NotificationPrefsStore for PlatformNotificationPrefsStore {
    fn get(&self, user_id: &str) -> NotificationPrefs {
        match self {
            Self::Memory(s) => s.get(user_id),
            Self::Postgres(s) => s.get(user_id),
        }
    }
    fn upsert(&self, prefs: NotificationPrefs) {
        match self {
            Self::Memory(s) => s.upsert(prefs),
            Self::Postgres(s) => s.upsert(prefs),
        }
    }
}
