// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Execution lifecycle event types.
pub const EVENT_EXECUTION_STARTED:   &str = "execution.started";
pub const EVENT_EXECUTION_COMPLETED: &str = "execution.completed";
pub const EVENT_EXECUTION_FAILED:    &str = "execution.failed";
pub const EVENT_EXECUTION_CANCELLED: &str = "execution.cancelled";

fn all_events() -> Vec<String> {
    vec![
        EVENT_EXECUTION_STARTED.to_string(),
        EVENT_EXECUTION_COMPLETED.to_string(),
        EVENT_EXECUTION_FAILED.to_string(),
        EVENT_EXECUTION_CANCELLED.to_string(),
    ]
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

fn next_id() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSubscription {
    pub id: String,
    pub tenant_id: String,
    pub url: String,
    /// Empty list means "all events".
    #[serde(default)]
    pub events: Vec<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSubscriptionRequest {
    pub tenant_id: String,
    pub url: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionError {
    NotFound,
    InvalidUrl,
    StoreUnavailable,
}

// ── Memory store ──────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct MemorySubscriptionStore {
    subs: Arc<RwLock<HashMap<String, EventSubscription>>>,
}

impl MemorySubscriptionStore {
    pub fn create(&self, req: CreateSubscriptionRequest) -> Result<EventSubscription, SubscriptionError> {
        if !req.url.starts_with("http") { return Err(SubscriptionError::InvalidUrl); }
        let sub = EventSubscription {
            id: next_id(),
            tenant_id: req.tenant_id,
            url: req.url,
            events: if req.events.is_empty() { all_events() } else { req.events },
            created_at: unix_now(),
            description: req.description,
        };
        self.subs.write().unwrap().insert(sub.id.clone(), sub.clone());
        Ok(sub)
    }

    pub fn list(&self, tenant_id: &str) -> Vec<EventSubscription> {
        self.subs.read().unwrap()
            .values()
            .filter(|s| s.tenant_id == tenant_id)
            .cloned()
            .collect()
    }

    pub fn delete(&self, tenant_id: &str, id: &str) -> Result<(), SubscriptionError> {
        let mut map = self.subs.write().unwrap();
        let sub = map.get(id).ok_or(SubscriptionError::NotFound)?;
        if sub.tenant_id != tenant_id { return Err(SubscriptionError::NotFound); }
        map.remove(id);
        Ok(())
    }

    pub fn matching(&self, tenant_id: &str, event: &str) -> Vec<EventSubscription> {
        self.subs.read().unwrap()
            .values()
            .filter(|s| s.tenant_id == tenant_id && s.events.iter().any(|e| e == event))
            .cloned()
            .collect()
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PostgresSubscriptionStore {
    pool: PgPool,
}

impl PostgresSubscriptionStore {
    pub fn new(pool: PgPool) -> Self { Self { pool } }

    pub async fn create(&self, req: CreateSubscriptionRequest) -> Result<EventSubscription, SubscriptionError> {
        if !req.url.starts_with("http") { return Err(SubscriptionError::InvalidUrl); }
        let id = next_id();
        let now = unix_now();
        let events = if req.events.is_empty() { all_events() } else { req.events };
        sqlx::query(
            "INSERT INTO af_event_subscriptions (id, tenant_id, url, events, created_at, description)
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&id).bind(&req.tenant_id).bind(&req.url)
        .bind(&events).bind(now).bind(&req.description)
        .execute(&self.pool).await.map_err(|_| SubscriptionError::StoreUnavailable)?;
        Ok(EventSubscription { id, tenant_id: req.tenant_id, url: req.url, events, created_at: now, description: req.description })
    }

    pub async fn list(&self, tenant_id: &str) -> Result<Vec<EventSubscription>, SubscriptionError> {
        #[derive(sqlx::FromRow)]
        struct Row { id: String, tenant_id: String, url: String, events: Vec<String>, created_at: i64, #[sqlx(default)] description: Option<String> }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, tenant_id, url, events, created_at, description FROM af_event_subscriptions WHERE tenant_id = $1 ORDER BY created_at ASC"
        )
        .bind(tenant_id).fetch_all(&self.pool).await.map_err(|_| SubscriptionError::StoreUnavailable)?;
        Ok(rows.into_iter().map(|r| EventSubscription { id: r.id, tenant_id: r.tenant_id, url: r.url, events: r.events, created_at: r.created_at, description: r.description }).collect())
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), SubscriptionError> {
        let result = sqlx::query(
            "DELETE FROM af_event_subscriptions WHERE id = $1 AND tenant_id = $2"
        )
        .bind(id).bind(tenant_id)
        .execute(&self.pool).await.map_err(|_| SubscriptionError::StoreUnavailable)?;
        if result.rows_affected() == 0 { Err(SubscriptionError::NotFound) } else { Ok(()) }
    }

    pub async fn matching(&self, tenant_id: &str, event: &str) -> Result<Vec<EventSubscription>, SubscriptionError> {
        #[derive(sqlx::FromRow)]
        struct Row { id: String, tenant_id: String, url: String, events: Vec<String>, created_at: i64, #[sqlx(default)] description: Option<String> }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, tenant_id, url, events, created_at, description FROM af_event_subscriptions WHERE tenant_id = $1 AND $2 = ANY(events)"
        )
        .bind(tenant_id).bind(event).fetch_all(&self.pool).await.map_err(|_| SubscriptionError::StoreUnavailable)?;
        Ok(rows.into_iter().map(|r| EventSubscription { id: r.id, tenant_id: r.tenant_id, url: r.url, events: r.events, created_at: r.created_at, description: r.description }).collect())
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformSubscriptionStore {
    Memory(MemorySubscriptionStore),
    Postgres(PostgresSubscriptionStore),
}

impl Default for PlatformSubscriptionStore {
    fn default() -> Self { Self::Memory(MemorySubscriptionStore::default()) }
}

impl PlatformSubscriptionStore {
    pub fn postgres(pool: PgPool) -> Self { Self::Postgres(PostgresSubscriptionStore::new(pool)) }

    pub async fn create(&self, req: CreateSubscriptionRequest) -> Result<EventSubscription, SubscriptionError> {
        match self {
            Self::Memory(s) => s.create(req),
            Self::Postgres(s) => s.create(req).await,
        }
    }

    pub async fn list(&self, tenant_id: &str) -> Result<Vec<EventSubscription>, SubscriptionError> {
        match self {
            Self::Memory(s) => Ok(s.list(tenant_id)),
            Self::Postgres(s) => s.list(tenant_id).await,
        }
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), SubscriptionError> {
        match self {
            Self::Memory(s) => s.delete(tenant_id, id),
            Self::Postgres(s) => s.delete(tenant_id, id).await,
        }
    }

    pub async fn matching(&self, tenant_id: &str, event: &str) -> Vec<EventSubscription> {
        match self {
            Self::Memory(s) => s.matching(tenant_id, event),
            Self::Postgres(s) => s.matching(tenant_id, event).await.unwrap_or_default(),
        }
    }
}

/// Fire event to all matching subscriptions for the tenant. Non-blocking.
pub fn fire_event(store: Arc<PlatformSubscriptionStore>, tenant_id: String, event: &'static str, payload: serde_json::Value) {
    tokio::spawn(async move {
        let subs = store.matching(&tenant_id, event).await;
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap_or_default();
        for sub in subs {
            let body = serde_json::json!({
                "event": event,
                "tenant_id": &tenant_id,
                "data": &payload,
            });
            let _ = client.post(&sub.url)
                .header("Content-Type", "application/json")
                .header("X-Velara-Event", event)
                .body(body.to_string())
                .send()
                .await;
        }
    });
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MemorySubscriptionStore { MemorySubscriptionStore::default() }

    #[test]
    fn create_and_list_subscription() {
        let s = store();
        let sub = s.create(CreateSubscriptionRequest {
            tenant_id: "t1".into(),
            url: "https://example.com/hook".into(),
            events: vec![EVENT_EXECUTION_STARTED.to_string()],
            description: None,
        }).unwrap();
        assert_eq!(sub.events, vec![EVENT_EXECUTION_STARTED]);
        let list = s.list("t1");
        assert_eq!(list.len(), 1);
        assert!(s.list("t2").is_empty());
    }

    #[test]
    fn empty_events_defaults_to_all() {
        let s = store();
        let sub = s.create(CreateSubscriptionRequest {
            tenant_id: "t1".into(),
            url: "https://example.com/hook".into(),
            events: vec![],
            description: None,
        }).unwrap();
        assert_eq!(sub.events.len(), 4);
        assert!(sub.events.contains(&EVENT_EXECUTION_STARTED.to_string()));
    }

    #[test]
    fn delete_subscription() {
        let s = store();
        let sub = s.create(CreateSubscriptionRequest {
            tenant_id: "t1".into(),
            url: "https://example.com/hook".into(),
            events: vec![],
            description: None,
        }).unwrap();
        s.delete("t1", &sub.id).unwrap();
        assert!(s.list("t1").is_empty());
    }

    #[test]
    fn matching_filters_by_event() {
        let s = store();
        s.create(CreateSubscriptionRequest {
            tenant_id: "t1".into(),
            url: "https://example.com/hook1".into(),
            events: vec![EVENT_EXECUTION_STARTED.to_string()],
            description: None,
        }).unwrap();
        s.create(CreateSubscriptionRequest {
            tenant_id: "t1".into(),
            url: "https://example.com/hook2".into(),
            events: vec![EVENT_EXECUTION_FAILED.to_string()],
            description: None,
        }).unwrap();
        let started = s.matching("t1", EVENT_EXECUTION_STARTED);
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].url, "https://example.com/hook1");
        let failed = s.matching("t1", EVENT_EXECUTION_FAILED);
        assert_eq!(failed.len(), 1);
        let completed = s.matching("t1", EVENT_EXECUTION_COMPLETED);
        assert!(completed.is_empty());
    }

    #[test]
    fn invalid_url_rejected() {
        let s = store();
        let err = s.create(CreateSubscriptionRequest {
            tenant_id: "t1".into(),
            url: "not-a-url".into(),
            events: vec![],
            description: None,
        }).unwrap_err();
        assert_eq!(err, SubscriptionError::InvalidUrl);
    }
}
