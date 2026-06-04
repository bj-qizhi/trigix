// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Registry of community / third-party custom nodes (the node SDK ecosystem).
//!
//! A custom node is an HTTP service that follows the SDK contract: it receives
//! `{ node_id, config, input_json, node_outputs }` and returns `{ output_json }`.
//! Registered definitions provide the label, description, config schema (for the
//! UI) and endpoint. Workflows reference a node by slug; the editor bakes the
//! endpoint into the node config so the executor can call it directly.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomNodeDef {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    pub endpoint: String,
    /// JSON Schema describing the node's config fields (rendered by the editor).
    #[serde(default)]
    pub config_schema: serde_json::Value,
    pub created_at: i64,
}

#[derive(Default)]
pub struct MemoryCustomNodeStore {
    rows: RwLock<Vec<CustomNodeDef>>,
}

impl MemoryCustomNodeStore {
    pub async fn upsert(&self, def: CustomNodeDef) -> CustomNodeDef {
        let mut rows = self.rows.write().unwrap();
        rows.retain(|d| !(d.tenant_id == def.tenant_id && d.slug == def.slug));
        rows.push(def.clone());
        def
    }
    pub async fn list_by_tenant(&self, tenant_id: &str) -> Vec<CustomNodeDef> {
        self.rows
            .read()
            .unwrap()
            .iter()
            .filter(|d| d.tenant_id == tenant_id)
            .cloned()
            .collect()
    }
    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        let mut rows = self.rows.write().unwrap();
        let before = rows.len();
        rows.retain(|d| !(d.tenant_id == tenant_id && d.id == id));
        rows.len() != before
    }
}

pub struct PostgresCustomNodeStore {
    pool: sqlx::PgPool,
}

impl PostgresCustomNodeStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, def: CustomNodeDef) -> CustomNodeDef {
        let _ = sqlx::query(
            "INSERT INTO af_custom_nodes \
             (id, tenant_id, slug, label, description, endpoint, config_schema, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8) \
             ON CONFLICT (tenant_id, slug) DO UPDATE SET \
             label = EXCLUDED.label, description = EXCLUDED.description, \
             endpoint = EXCLUDED.endpoint, config_schema = EXCLUDED.config_schema",
        )
        .bind(&def.id)
        .bind(&def.tenant_id)
        .bind(&def.slug)
        .bind(&def.label)
        .bind(&def.description)
        .bind(&def.endpoint)
        .bind(&def.config_schema)
        .bind(def.created_at)
        .execute(&self.pool)
        .await;
        def
    }

    pub async fn list_by_tenant(&self, tenant_id: &str) -> Vec<CustomNodeDef> {
        sqlx::query_as::<_, CustomNodeRow>(
            "SELECT * FROM af_custom_nodes WHERE tenant_id = $1 ORDER BY label",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect()
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        sqlx::query("DELETE FROM af_custom_nodes WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .unwrap_or(false)
    }
}

#[derive(sqlx::FromRow)]
struct CustomNodeRow {
    id: String,
    tenant_id: String,
    slug: String,
    label: String,
    description: String,
    endpoint: String,
    config_schema: serde_json::Value,
    created_at: i64,
}

impl From<CustomNodeRow> for CustomNodeDef {
    fn from(r: CustomNodeRow) -> Self {
        Self {
            id: r.id,
            tenant_id: r.tenant_id,
            slug: r.slug,
            label: r.label,
            description: r.description,
            endpoint: r.endpoint,
            config_schema: r.config_schema,
            created_at: r.created_at,
        }
    }
}

pub enum PlatformCustomNodeStore {
    Memory(MemoryCustomNodeStore),
    Postgres(PostgresCustomNodeStore),
}

impl Default for PlatformCustomNodeStore {
    fn default() -> Self {
        Self::Memory(MemoryCustomNodeStore::default())
    }
}

impl PlatformCustomNodeStore {
    pub fn memory() -> Self {
        Self::default()
    }
    pub fn postgres(s: PostgresCustomNodeStore) -> Self {
        Self::Postgres(s)
    }
    pub async fn upsert(&self, def: CustomNodeDef) -> CustomNodeDef {
        match self {
            Self::Memory(s) => s.upsert(def).await,
            Self::Postgres(s) => s.upsert(def).await,
        }
    }
    pub async fn list_by_tenant(&self, tenant_id: &str) -> Vec<CustomNodeDef> {
        match self {
            Self::Memory(s) => s.list_by_tenant(tenant_id).await,
            Self::Postgres(s) => s.list_by_tenant(tenant_id).await,
        }
    }
    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        match self {
            Self::Memory(s) => s.delete(tenant_id, id).await,
            Self::Postgres(s) => s.delete(tenant_id, id).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(slug: &str) -> CustomNodeDef {
        CustomNodeDef {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: "t".into(),
            slug: slug.into(),
            label: "My Node".into(),
            description: "does a thing".into(),
            endpoint: "http://localhost:9000/nodes/my".into(),
            config_schema: serde_json::json!({"type": "object"}),
            created_at: unix_now(),
        }
    }

    #[tokio::test]
    async fn upsert_replaces_by_slug() {
        let store = MemoryCustomNodeStore::default();
        store.upsert(def("greet")).await;
        let mut d2 = def("greet");
        d2.label = "Renamed".into();
        store.upsert(d2).await;
        let all = store.list_by_tenant("t").await;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].label, "Renamed");
    }

    #[tokio::test]
    async fn delete_removes() {
        let store = MemoryCustomNodeStore::default();
        let d = store.upsert(def("greet")).await;
        assert!(store.delete("t", &d.id).await);
        assert!(store.list_by_tenant("t").await.is_empty());
    }
}
