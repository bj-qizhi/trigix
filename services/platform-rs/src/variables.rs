// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub key: String,
    pub value: serde_json::Value,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── In-memory ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MemoryVariableStore {
    // key: "{tenant_id}/{workflow_id}/{var_key}"
    data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl MemoryVariableStore {
    fn make_key(tenant_id: &str, workflow_id: &str, key: &str) -> String {
        format!("{tenant_id}/{workflow_id}/{key}")
    }

    pub async fn get(&self, tenant_id: &str, workflow_id: &str, key: &str) -> Option<Variable> {
        let k = Self::make_key(tenant_id, workflow_id, key);
        self.data.read().unwrap().get(&k).map(|v| Variable {
            key: key.to_string(),
            value: v.clone(),
        })
    }

    pub async fn set(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Variable {
        let k = Self::make_key(tenant_id, workflow_id, key);
        self.data.write().unwrap().insert(k, value.clone());
        Variable {
            key: key.to_string(),
            value,
        }
    }

    pub async fn delete(&self, tenant_id: &str, workflow_id: &str, key: &str) -> bool {
        let k = Self::make_key(tenant_id, workflow_id, key);
        self.data.write().unwrap().remove(&k).is_some()
    }

    pub async fn list(&self, tenant_id: &str, workflow_id: &str) -> Vec<Variable> {
        let prefix = format!("{tenant_id}/{workflow_id}/");
        let mut out: Vec<Variable> = self
            .data
            .read()
            .unwrap()
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix(&prefix).map(|key| Variable {
                    key: key.to_string(),
                    value: v.clone(),
                })
            })
            .collect();
        out.sort_by(|a, b| a.key.cmp(&b.key));
        out
    }

    pub async fn increment(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        key: &str,
        by: f64,
    ) -> Variable {
        let k = Self::make_key(tenant_id, workflow_id, key);
        let mut map = self.data.write().unwrap();
        let current = map.get(&k).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let next = current + by;
        let value = if next.fract() == 0.0 && next.abs() < 1e15 {
            serde_json::json!(next as i64)
        } else {
            serde_json::json!(next)
        };
        map.insert(k, value.clone());
        Variable {
            key: key.to_string(),
            value,
        }
    }
}

// ── Postgres ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PostgresVariableStore {
    pool: PgPool,
}

impl PostgresVariableStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, tenant_id: &str, workflow_id: &str, key: &str) -> Option<Variable> {
        sqlx::query_as::<_, (serde_json::Value,)>(
            r#"SELECT value_json FROM af_workflow_variables
               WHERE tenant_id = $1 AND workflow_id = $2 AND key = $3"#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|(value,)| Variable {
            key: key.to_string(),
            value,
        })
    }

    pub async fn set(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Variable {
        let now = unix_now();
        let _ = sqlx::query(
            r#"INSERT INTO af_workflow_variables (tenant_id, workflow_id, key, value_json, updated_at)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (tenant_id, workflow_id, key) DO UPDATE
               SET value_json = EXCLUDED.value_json, updated_at = EXCLUDED.updated_at"#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .bind(key)
        .bind(&value)
        .bind(now)
        .execute(&self.pool)
        .await;
        Variable {
            key: key.to_string(),
            value,
        }
    }

    pub async fn delete(&self, tenant_id: &str, workflow_id: &str, key: &str) -> bool {
        sqlx::query(
            r#"DELETE FROM af_workflow_variables
               WHERE tenant_id = $1 AND workflow_id = $2 AND key = $3"#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .bind(key)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
        .unwrap_or(false)
    }

    pub async fn list(&self, tenant_id: &str, workflow_id: &str) -> Vec<Variable> {
        sqlx::query_as::<_, (String, serde_json::Value)>(
            r#"SELECT key, value_json FROM af_workflow_variables
               WHERE tenant_id = $1 AND workflow_id = $2 ORDER BY key ASC"#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| Variable { key, value })
        .collect()
    }

    pub async fn increment(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        key: &str,
        by: f64,
    ) -> Variable {
        let now = unix_now();
        // Atomic increment using Postgres JSONB arithmetic
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            r#"INSERT INTO af_workflow_variables (tenant_id, workflow_id, key, value_json, updated_at)
               VALUES ($1, $2, $3, to_jsonb($4::float8), $5)
               ON CONFLICT (tenant_id, workflow_id, key) DO UPDATE
               SET value_json = to_jsonb((COALESCE((af_workflow_variables.value_json)::float8, 0) + $4::float8)),
                   updated_at = $5
               RETURNING value_json"#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .bind(key)
        .bind(by)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();

        let value = match row {
            Some((v,)) => v,
            None => serde_json::json!(by),
        };
        // Represent whole numbers as integers
        let value = if let Some(f) = value.as_f64() {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                serde_json::json!(f as i64)
            } else {
                serde_json::json!(f)
            }
        } else {
            value
        };
        Variable {
            key: key.to_string(),
            value,
        }
    }
}

// ── Platform enum ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformVariableStore {
    Memory(MemoryVariableStore),
    Postgres(PostgresVariableStore),
}

impl Default for PlatformVariableStore {
    fn default() -> Self {
        Self::Memory(MemoryVariableStore::default())
    }
}

impl PlatformVariableStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryVariableStore::default())
    }

    pub fn postgres(store: PostgresVariableStore) -> Self {
        Self::Postgres(store)
    }

    pub async fn get(&self, tenant_id: &str, workflow_id: &str, key: &str) -> Option<Variable> {
        match self {
            Self::Memory(s) => s.get(tenant_id, workflow_id, key).await,
            Self::Postgres(s) => s.get(tenant_id, workflow_id, key).await,
        }
    }

    pub async fn set(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Variable {
        match self {
            Self::Memory(s) => s.set(tenant_id, workflow_id, key, value).await,
            Self::Postgres(s) => s.set(tenant_id, workflow_id, key, value).await,
        }
    }

    pub async fn delete(&self, tenant_id: &str, workflow_id: &str, key: &str) -> bool {
        match self {
            Self::Memory(s) => s.delete(tenant_id, workflow_id, key).await,
            Self::Postgres(s) => s.delete(tenant_id, workflow_id, key).await,
        }
    }

    pub async fn list(&self, tenant_id: &str, workflow_id: &str) -> Vec<Variable> {
        match self {
            Self::Memory(s) => s.list(tenant_id, workflow_id).await,
            Self::Postgres(s) => s.list(tenant_id, workflow_id).await,
        }
    }

    pub async fn increment(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        key: &str,
        by: f64,
    ) -> Variable {
        match self {
            Self::Memory(s) => s.increment(tenant_id, workflow_id, key, by).await,
            Self::Postgres(s) => s.increment(tenant_id, workflow_id, key, by).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_get_delete_variable() {
        let store = MemoryVariableStore::default();
        store.set("t1", "w1", "counter", serde_json::json!(0)).await;
        assert_eq!(
            store.get("t1", "w1", "counter").await.unwrap().value,
            serde_json::json!(0)
        );
        assert!(store.delete("t1", "w1", "counter").await);
        assert!(store.get("t1", "w1", "counter").await.is_none());
        assert!(!store.delete("t1", "w1", "counter").await);
    }

    #[tokio::test]
    async fn increment_variable() {
        let store = MemoryVariableStore::default();
        store.increment("t1", "w1", "hits", 1.0).await;
        store.increment("t1", "w1", "hits", 1.0).await;
        store.increment("t1", "w1", "hits", 3.0).await;
        assert_eq!(
            store.get("t1", "w1", "hits").await.unwrap().value,
            serde_json::json!(5)
        );
    }

    #[tokio::test]
    async fn list_is_scoped_to_workflow() {
        let store = MemoryVariableStore::default();
        store.set("t1", "w1", "a", serde_json::json!("x")).await;
        store.set("t1", "w1", "b", serde_json::json!("y")).await;
        store.set("t1", "w2", "c", serde_json::json!("z")).await;
        let list = store.list("t1", "w1").await;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].key, "a");
        assert_eq!(list[1].key, "b");
        assert_eq!(store.list("t1", "w2").await.len(), 1);
    }
}
