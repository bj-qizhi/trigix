// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn new_id() -> String {
    format!("notif_{}", uuid::Uuid::new_v4().simple())
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Notification {
    pub id: String,
    pub tenant_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    pub title: String,
    pub body: String,
    pub level: String,
    pub read: bool,
    pub created_at: u64,
}

pub trait NotificationStore: Send + Sync {
    fn create(&self, tenant_id: &str, user_id: Option<&str>, title: &str, body: &str, level: &str) -> Notification;
    fn list(&self, tenant_id: &str, user_id: Option<&str>, limit: usize) -> Vec<Notification>;
    fn mark_read(&self, id: &str, tenant_id: &str) -> bool;
    fn mark_all_read(&self, tenant_id: &str, user_id: Option<&str>);
    fn delete(&self, id: &str, tenant_id: &str) -> bool;
    fn unread_count(&self, tenant_id: &str, user_id: Option<&str>) -> u64;
}

// ── Memory store ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct MemoryNotificationStore {
    data: RwLock<HashMap<String, Notification>>,
}

impl NotificationStore for MemoryNotificationStore {
    fn create(&self, tenant_id: &str, user_id: Option<&str>, title: &str, body: &str, level: &str) -> Notification {
        let n = Notification {
            id: new_id(),
            tenant_id: tenant_id.to_owned(),
            user_id: user_id.map(str::to_owned),
            title: title.to_owned(),
            body: body.to_owned(),
            level: level.to_owned(),
            read: false,
            created_at: unix_now(),
        };
        self.data.write().unwrap().insert(n.id.clone(), n.clone());
        n
    }

    fn list(&self, tenant_id: &str, user_id: Option<&str>, limit: usize) -> Vec<Notification> {
        let mut items: Vec<_> = self.data.read().unwrap()
            .values()
            .filter(|n| n.tenant_id == tenant_id && user_id.map_or(true, |uid| n.user_id.as_deref() == Some(uid) || n.user_id.is_none()))
            .cloned()
            .collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items.truncate(limit);
        items
    }

    fn mark_read(&self, id: &str, tenant_id: &str) -> bool {
        let mut data = self.data.write().unwrap();
        if let Some(n) = data.get_mut(id) {
            if tenant_id.is_empty() || n.tenant_id == tenant_id {
                n.read = true;
                return true;
            }
        }
        false
    }

    fn mark_all_read(&self, tenant_id: &str, user_id: Option<&str>) {
        let mut data = self.data.write().unwrap();
        for n in data.values_mut() {
            if (tenant_id.is_empty() || n.tenant_id == tenant_id) && user_id.map_or(true, |uid| n.user_id.as_deref() == Some(uid) || n.user_id.is_none()) {
                n.read = true;
            }
        }
    }

    fn delete(&self, id: &str, tenant_id: &str) -> bool {
        let mut data = self.data.write().unwrap();
        if data.get(id).map(|n| tenant_id.is_empty() || n.tenant_id == tenant_id).unwrap_or(false) {
            data.remove(id);
            return true;
        }
        false
    }

    fn unread_count(&self, tenant_id: &str, user_id: Option<&str>) -> u64 {
        self.data.read().unwrap()
            .values()
            .filter(|n| !n.read && n.tenant_id == tenant_id && user_id.map_or(true, |uid| n.user_id.as_deref() == Some(uid) || n.user_id.is_none()))
            .count() as u64
    }
}

// ── Postgres row ──────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct NotifRow {
    id: String,
    tenant_id: String,
    user_id: Option<String>,
    title: String,
    body: String,
    level: String,
    read: bool,
    created_at: i64,
}

impl From<NotifRow> for Notification {
    fn from(r: NotifRow) -> Self {
        Notification {
            id: r.id,
            tenant_id: r.tenant_id,
            user_id: r.user_id,
            title: r.title,
            body: r.body,
            level: r.level,
            read: r.read,
            created_at: r.created_at as u64,
        }
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

pub struct PostgresNotificationStore {
    pool: PgPool,
}

impl PostgresNotificationStore {
    pub fn new(pool: PgPool) -> Self { Self { pool } }
}

impl NotificationStore for PostgresNotificationStore {
    fn create(&self, tenant_id: &str, user_id: Option<&str>, title: &str, body: &str, level: &str) -> Notification {
        let n = Notification {
            id: new_id(),
            tenant_id: tenant_id.to_owned(),
            user_id: user_id.map(str::to_owned),
            title: title.to_owned(),
            body: body.to_owned(),
            level: level.to_owned(),
            read: false,
            created_at: unix_now(),
        };
        let pool = self.pool.clone();
        let n2 = n.clone();
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO af_notifications (id, tenant_id, user_id, title, body, level, read, created_at) VALUES ($1,$2,$3,$4,$5,$6,false,$7)"
            )
            .bind(&n2.id).bind(&n2.tenant_id).bind(&n2.user_id).bind(&n2.title).bind(&n2.body).bind(&n2.level).bind(n2.created_at as i64)
            .execute(&pool).await;
        });
        n
    }

    fn list(&self, tenant_id: &str, user_id: Option<&str>, limit: usize) -> Vec<Notification> {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_owned();
        let user_id = user_id.map(str::to_owned);
        let limit = limit as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                if let Some(uid) = &user_id {
                    sqlx::query_as::<_, NotifRow>(
                        "SELECT id, tenant_id, user_id, title, body, level, read, created_at FROM af_notifications WHERE tenant_id=$1 AND (user_id=$2 OR user_id IS NULL) ORDER BY created_at DESC LIMIT $3"
                    ).bind(&tenant_id).bind(uid).bind(limit).fetch_all(&pool).await.unwrap_or_default()
                } else {
                    sqlx::query_as::<_, NotifRow>(
                        "SELECT id, tenant_id, user_id, title, body, level, read, created_at FROM af_notifications WHERE tenant_id=$1 ORDER BY created_at DESC LIMIT $2"
                    ).bind(&tenant_id).bind(limit).fetch_all(&pool).await.unwrap_or_default()
                }.into_iter().map(Into::into).collect()
            })
        })
    }

    fn mark_read(&self, id: &str, tenant_id: &str) -> bool {
        let pool = self.pool.clone();
        let id = id.to_owned();
        let tenant_id = tenant_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query("UPDATE af_notifications SET read=true WHERE id=$1 AND tenant_id=$2")
                .bind(&id).bind(&tenant_id)
                .execute(&pool).await.map(|r| r.rows_affected() > 0).unwrap_or(false)
            })
        })
    }

    fn mark_all_read(&self, tenant_id: &str, user_id: Option<&str>) {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_owned();
        let user_id = user_id.map(str::to_owned);
        tokio::spawn(async move {
            if let Some(uid) = user_id {
                let _ = sqlx::query(
                    "UPDATE af_notifications SET read=true WHERE tenant_id=$1 AND (user_id=$2 OR user_id IS NULL)"
                ).bind(&tenant_id).bind(&uid).execute(&pool).await;
            } else {
                let _ = sqlx::query(
                    "UPDATE af_notifications SET read=true WHERE tenant_id=$1"
                ).bind(&tenant_id).execute(&pool).await;
            }
        });
    }

    fn delete(&self, id: &str, tenant_id: &str) -> bool {
        let pool = self.pool.clone();
        let id = id.to_owned();
        let tenant_id = tenant_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query("DELETE FROM af_notifications WHERE id=$1 AND tenant_id=$2")
                .bind(&id).bind(&tenant_id)
                .execute(&pool).await.map(|r| r.rows_affected() > 0).unwrap_or(false)
            })
        })
    }

    fn unread_count(&self, tenant_id: &str, user_id: Option<&str>) -> u64 {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_owned();
        let user_id = user_id.map(str::to_owned);
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let count: i64 = if let Some(uid) = user_id {
                    sqlx::query_scalar(
                        "SELECT COUNT(*) FROM af_notifications WHERE tenant_id=$1 AND read=false AND (user_id=$2 OR user_id IS NULL)"
                    ).bind(&tenant_id).bind(&uid).fetch_one(&pool).await.unwrap_or(0)
                } else {
                    sqlx::query_scalar(
                        "SELECT COUNT(*) FROM af_notifications WHERE tenant_id=$1 AND read=false"
                    ).bind(&tenant_id).fetch_one(&pool).await.unwrap_or(0)
                };
                count as u64
            })
        })
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

pub enum PlatformNotificationStore {
    Memory(MemoryNotificationStore),
    Postgres(PostgresNotificationStore),
}

impl Default for PlatformNotificationStore {
    fn default() -> Self { Self::Memory(MemoryNotificationStore::default()) }
}

impl PlatformNotificationStore {
    pub fn postgres(pool: PgPool) -> Self { Self::Postgres(PostgresNotificationStore::new(pool)) }
}

impl NotificationStore for PlatformNotificationStore {
    fn create(&self, tenant_id: &str, user_id: Option<&str>, title: &str, body: &str, level: &str) -> Notification {
        match self { Self::Memory(s) => s.create(tenant_id, user_id, title, body, level), Self::Postgres(s) => s.create(tenant_id, user_id, title, body, level) }
    }
    fn list(&self, tenant_id: &str, user_id: Option<&str>, limit: usize) -> Vec<Notification> {
        match self { Self::Memory(s) => s.list(tenant_id, user_id, limit), Self::Postgres(s) => s.list(tenant_id, user_id, limit) }
    }
    fn mark_read(&self, id: &str, tenant_id: &str) -> bool {
        match self { Self::Memory(s) => s.mark_read(id, tenant_id), Self::Postgres(s) => s.mark_read(id, tenant_id) }
    }
    fn mark_all_read(&self, tenant_id: &str, user_id: Option<&str>) {
        match self { Self::Memory(s) => s.mark_all_read(tenant_id, user_id), Self::Postgres(s) => s.mark_all_read(tenant_id, user_id) }
    }
    fn delete(&self, id: &str, tenant_id: &str) -> bool {
        match self { Self::Memory(s) => s.delete(id, tenant_id), Self::Postgres(s) => s.delete(id, tenant_id) }
    }
    fn unread_count(&self, tenant_id: &str, user_id: Option<&str>) -> u64 {
        match self { Self::Memory(s) => s.unread_count(tenant_id, user_id), Self::Postgres(s) => s.unread_count(tenant_id, user_id) }
    }
}
