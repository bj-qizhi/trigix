// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub id: String,
    pub tenant_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub detail: Option<String>,
    pub timestamp: u64,
}

// Well-known action constants
pub mod action {
    pub const EXECUTION_STARTED: &str = "execution.started";
    pub const EXECUTION_APPROVED: &str = "execution.approved";
    pub const EXECUTION_REJECTED: &str = "execution.rejected";
    pub const EXECUTION_CANCELLED: &str = "execution.cancelled";
    pub const EXECUTION_RETRIED: &str = "execution.retried";
    pub const WORKFLOW_CREATED: &str = "workflow.created";
    pub const WORKFLOW_UPDATED: &str = "workflow.updated";
    pub const WORKFLOW_PUBLISHED: &str = "workflow.published";
    pub const WORKFLOW_ARCHIVED: &str = "workflow.archived";
    pub const WORKFLOW_RESTORED: &str = "workflow.restored";
    pub const WORKFLOW_DUPLICATED: &str = "workflow.duplicated";
    pub const WORKFLOW_TAGGED: &str = "workflow.tagged";
    pub const WORKFLOW_PINNED: &str = "workflow.pinned";
    pub const WORKFLOW_UNPINNED: &str = "workflow.unpinned";
    pub const WORKFLOW_LOCKED: &str = "workflow.locked";
    pub const WORKFLOW_UNLOCKED: &str = "workflow.unlocked";
    pub const CREDENTIAL_CREATED: &str = "credential.created";
    pub const CREDENTIAL_DELETED: &str = "credential.deleted";
    pub const SCHEDULE_REGISTERED: &str = "schedule.registered";
    pub const SCHEDULE_REMOVED: &str = "schedule.removed";
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn next_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("evt-{:x}", nanos)
}

const MAX_EVENTS: usize = 1000;

#[derive(Clone, Default)]
pub struct MemoryAuditStore {
    events: Arc<RwLock<VecDeque<AuditEvent>>>,
}

impl MemoryAuditStore {
    pub fn record(
        &self,
        tenant_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        detail: Option<serde_json::Value>,
    ) {
        let event = AuditEvent {
            id: next_id(),
            tenant_id: tenant_id.to_string(),
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            detail: detail.map(|v| v.to_string()),
            timestamp: unix_now(),
        };
        if let Ok(mut events) = self.events.write() {
            events.push_front(event);
            if events.len() > MAX_EVENTS {
                events.pop_back();
            }
        }
    }

    pub fn list(&self, tenant_id: &str, limit: usize) -> Vec<AuditEvent> {
        self.events
            .read()
            .map(|events| {
                events
                    .iter()
                    .filter(|e| e.tenant_id == tenant_id)
                    .take(limit)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct PostgresAuditStore {
    pool: PgPool,
}

impl PostgresAuditStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn record(
        &self,
        tenant_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        detail: Option<serde_json::Value>,
    ) {
        let pool = self.pool.clone();
        let id = uuid::Uuid::new_v4().to_string();
        let tenant_id = tenant_id.to_string();
        let action = action.to_string();
        let resource_type = resource_type.to_string();
        let resource_id = resource_id.to_string();
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO af_audit_log (id, tenant_id, action, resource_type, resource_id, detail_json) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(&id)
            .bind(&tenant_id)
            .bind(&action)
            .bind(&resource_type)
            .bind(&resource_id)
            .bind(detail)
            .execute(&pool)
            .await;
        });
    }

    pub async fn list(&self, tenant_id: &str, limit: usize) -> Vec<AuditEvent> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            tenant_id: String,
            action: String,
            resource_type: String,
            resource_id: String,
            detail_json: Option<serde_json::Value>,
            ts: i64,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT id, tenant_id, action, resource_type, resource_id, detail_json,
                      EXTRACT(EPOCH FROM created_at)::bigint AS ts
               FROM af_audit_log
               WHERE tenant_id = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
        )
        .bind(tenant_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        rows.into_iter()
            .map(|r| AuditEvent {
                id: r.id,
                tenant_id: r.tenant_id,
                action: r.action,
                resource_type: r.resource_type,
                resource_id: r.resource_id,
                detail: r.detail_json.map(|v| v.to_string()),
                timestamp: r.ts as u64,
            })
            .collect()
    }
}

#[derive(Clone)]
pub enum PlatformAuditStore {
    Memory(MemoryAuditStore),
    Postgres(PostgresAuditStore),
}

impl Default for PlatformAuditStore {
    fn default() -> Self {
        Self::Memory(MemoryAuditStore::default())
    }
}

impl PlatformAuditStore {
    pub fn memory() -> Self {
        Self::default()
    }

    pub fn postgres(store: PostgresAuditStore) -> Self {
        Self::Postgres(store)
    }

    pub fn record(
        &self,
        tenant_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        detail: Option<serde_json::Value>,
    ) {
        match self {
            Self::Memory(s) => s.record(tenant_id, action, resource_type, resource_id, detail),
            Self::Postgres(s) => s.record(tenant_id, action, resource_type, resource_id, detail),
        }
    }

    pub async fn list(&self, tenant_id: &str, limit: usize) -> Vec<AuditEvent> {
        match self {
            Self::Memory(s) => s.list(tenant_id, limit),
            Self::Postgres(s) => s.list(tenant_id, limit).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_lists_events() {
        let store = MemoryAuditStore::default();
        store.record("t1", action::WORKFLOW_CREATED, "workflow", "wf-1", None);
        store.record("t1", action::WORKFLOW_PUBLISHED, "workflow", "wf-1", None);
        store.record("t2", action::EXECUTION_STARTED, "execution", "exec-1", None);

        let events = store.list("t1", 10);
        assert_eq!(events.len(), 2);
        // newest first
        assert_eq!(events[0].action, action::WORKFLOW_PUBLISHED);
        assert_eq!(events[1].action, action::WORKFLOW_CREATED);

        // different tenant isolated
        let events = store.list("t2", 10);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, action::EXECUTION_STARTED);
    }

    #[test]
    fn stores_detail_as_json_string() {
        let store = MemoryAuditStore::default();
        store.record(
            "t1",
            action::WORKFLOW_CREATED,
            "workflow",
            "wf-1",
            Some(serde_json::json!({"name": "My Flow"})),
        );
        let events = store.list("t1", 10);
        assert!(events[0].detail.as_deref().unwrap().contains("My Flow"));
    }

    #[test]
    fn respects_limit() {
        let store = MemoryAuditStore::default();
        for i in 0..20 {
            store.record(
                "t1",
                action::EXECUTION_STARTED,
                "execution",
                &format!("exec-{i}"),
                None,
            );
        }
        let events = store.list("t1", 5);
        assert_eq!(events.len(), 5);
    }
}
