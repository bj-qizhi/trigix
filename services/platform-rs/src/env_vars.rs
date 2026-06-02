// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

pub const DEFAULT_SET: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVarRecord {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvSetSummary {
    pub name: String,
    pub var_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvVarError {
    NotFound,
    StoreUnavailable,
}

pub trait EnvVarStore: Clone + Send + Sync + 'static {
    // ── Default set operations (backward compat) ──────────────────────────

    fn set(
        &self,
        tenant_id: &str,
        key: &str,
        value: &str,
    ) -> impl std::future::Future<Output = Result<EnvVarRecord, EnvVarError>> + Send;

    fn list(
        &self,
        tenant_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<EnvVarRecord>, EnvVarError>> + Send;

    fn get(
        &self,
        tenant_id: &str,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<String>, EnvVarError>> + Send;

    fn delete(
        &self,
        tenant_id: &str,
        key: &str,
    ) -> impl std::future::Future<Output = Result<(), EnvVarError>> + Send;

    // ── Named-set operations ──────────────────────────────────────────────

    fn set_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
        value: &str,
    ) -> impl std::future::Future<Output = Result<EnvVarRecord, EnvVarError>> + Send;

    fn list_in(
        &self,
        tenant_id: &str,
        set_name: &str,
    ) -> impl std::future::Future<Output = Result<Vec<EnvVarRecord>, EnvVarError>> + Send;

    fn get_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<String>, EnvVarError>> + Send;

    fn delete_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
    ) -> impl std::future::Future<Output = Result<(), EnvVarError>> + Send;

    fn list_sets(
        &self,
        tenant_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<EnvSetSummary>, EnvVarError>> + Send;

    fn delete_set(
        &self,
        tenant_id: &str,
        set_name: &str,
    ) -> impl std::future::Future<Output = Result<(), EnvVarError>> + Send;
}

// key format: "{tenant_id}/{set_name}/{var_key}"
fn store_key(tenant_id: &str, set_name: &str, key: &str) -> String {
    format!("{}/{}/{}", tenant_id, set_name, key)
}

fn set_prefix(tenant_id: &str, set_name: &str) -> String {
    format!("{}/{}/", tenant_id, set_name)
}

fn tenant_prefix(tenant_id: &str) -> String {
    format!("{}/", tenant_id)
}

#[derive(Clone, Default)]
pub struct MemoryEnvVarStore {
    records: Arc<RwLock<HashMap<String, EnvVarRecord>>>,
}

impl EnvVarStore for MemoryEnvVarStore {
    async fn set(&self, tenant_id: &str, key: &str, value: &str) -> Result<EnvVarRecord, EnvVarError> {
        self.set_in(tenant_id, DEFAULT_SET, key, value).await
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<EnvVarRecord>, EnvVarError> {
        self.list_in(tenant_id, DEFAULT_SET).await
    }

    async fn get(&self, tenant_id: &str, key: &str) -> Result<Option<String>, EnvVarError> {
        self.get_in(tenant_id, DEFAULT_SET, key).await
    }

    async fn delete(&self, tenant_id: &str, key: &str) -> Result<(), EnvVarError> {
        self.delete_in(tenant_id, DEFAULT_SET, key).await
    }

    async fn set_in(&self, tenant_id: &str, set_name: &str, key: &str, value: &str) -> Result<EnvVarRecord, EnvVarError> {
        let mut records = self.records.write().map_err(|_| EnvVarError::StoreUnavailable)?;
        let record = EnvVarRecord { key: key.to_string(), value: value.to_string() };
        records.insert(store_key(tenant_id, set_name, key), record.clone());
        Ok(record)
    }

    async fn list_in(&self, tenant_id: &str, set_name: &str) -> Result<Vec<EnvVarRecord>, EnvVarError> {
        let prefix = set_prefix(tenant_id, set_name);
        let records = self.records.read().map_err(|_| EnvVarError::StoreUnavailable)?;
        let mut out: Vec<EnvVarRecord> = records
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(_, r)| r.clone())
            .collect();
        out.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(out)
    }

    async fn get_in(&self, tenant_id: &str, set_name: &str, key: &str) -> Result<Option<String>, EnvVarError> {
        let records = self.records.read().map_err(|_| EnvVarError::StoreUnavailable)?;
        Ok(records.get(&store_key(tenant_id, set_name, key)).map(|r| r.value.clone()))
    }

    async fn delete_in(&self, tenant_id: &str, set_name: &str, key: &str) -> Result<(), EnvVarError> {
        let mut records = self.records.write().map_err(|_| EnvVarError::StoreUnavailable)?;
        records.remove(&store_key(tenant_id, set_name, key)).ok_or(EnvVarError::NotFound)?;
        Ok(())
    }

    async fn list_sets(&self, tenant_id: &str) -> Result<Vec<EnvSetSummary>, EnvVarError> {
        let prefix = tenant_prefix(tenant_id);
        let records = self.records.read().map_err(|_| EnvVarError::StoreUnavailable)?;
        let mut set_counts: HashMap<String, usize> = HashMap::new();
        for key in records.keys() {
            if let Some(rest) = key.strip_prefix(&prefix) {
                // rest = "{set_name}/{var_key}"
                if let Some(slash) = rest.find('/') {
                    let set_name = &rest[..slash];
                    *set_counts.entry(set_name.to_string()).or_insert(0) += 1;
                }
            }
        }
        let mut out: Vec<EnvSetSummary> = set_counts
            .into_iter()
            .map(|(name, var_count)| EnvSetSummary { name, var_count })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn delete_set(&self, tenant_id: &str, set_name: &str) -> Result<(), EnvVarError> {
        let prefix = set_prefix(tenant_id, set_name);
        let mut records = self.records.write().map_err(|_| EnvVarError::StoreUnavailable)?;
        let keys_to_remove: HashSet<String> = records
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        if keys_to_remove.is_empty() {
            return Err(EnvVarError::NotFound);
        }
        for k in keys_to_remove {
            records.remove(&k);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct PostgresEnvVarStore {
    pool: PgPool,
}

impl PostgresEnvVarStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl EnvVarStore for PostgresEnvVarStore {
    async fn set(&self, tenant_id: &str, key: &str, value: &str) -> Result<EnvVarRecord, EnvVarError> {
        self.set_in(tenant_id, DEFAULT_SET, key, value).await
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<EnvVarRecord>, EnvVarError> {
        self.list_in(tenant_id, DEFAULT_SET).await
    }

    async fn get(&self, tenant_id: &str, key: &str) -> Result<Option<String>, EnvVarError> {
        self.get_in(tenant_id, DEFAULT_SET, key).await
    }

    async fn delete(&self, tenant_id: &str, key: &str) -> Result<(), EnvVarError> {
        self.delete_in(tenant_id, DEFAULT_SET, key).await
    }

    async fn set_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
        value: &str,
    ) -> Result<EnvVarRecord, EnvVarError> {
        sqlx::query(
            r#"INSERT INTO af_env_vars (tenant_id, env_set, key, value)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (tenant_id, env_set, key)
               DO UPDATE SET value = EXCLUDED.value, updated_at = now()"#,
        )
        .bind(tenant_id)
        .bind(set_name)
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|_| EnvVarError::StoreUnavailable)?;

        Ok(EnvVarRecord { key: key.to_string(), value: value.to_string() })
    }

    async fn list_in(
        &self,
        tenant_id: &str,
        set_name: &str,
    ) -> Result<Vec<EnvVarRecord>, EnvVarError> {
        sqlx::query_as::<_, (String, String)>(
            "SELECT key, value FROM af_env_vars WHERE tenant_id = $1 AND env_set = $2 ORDER BY key ASC",
        )
        .bind(tenant_id)
        .bind(set_name)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(|(key, value)| EnvVarRecord { key, value }).collect())
        .map_err(|_| EnvVarError::StoreUnavailable)
    }

    async fn get_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
    ) -> Result<Option<String>, EnvVarError> {
        sqlx::query_as::<_, (String,)>(
            "SELECT value FROM af_env_vars WHERE tenant_id = $1 AND env_set = $2 AND key = $3",
        )
        .bind(tenant_id)
        .bind(set_name)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(|(v,)| v))
        .map_err(|_| EnvVarError::StoreUnavailable)
    }

    async fn delete_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
    ) -> Result<(), EnvVarError> {
        let res = sqlx::query(
            "DELETE FROM af_env_vars WHERE tenant_id = $1 AND env_set = $2 AND key = $3",
        )
        .bind(tenant_id)
        .bind(set_name)
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(|_| EnvVarError::StoreUnavailable)?;

        if res.rows_affected() == 0 {
            Err(EnvVarError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn list_sets(&self, tenant_id: &str) -> Result<Vec<EnvSetSummary>, EnvVarError> {
        sqlx::query_as::<_, (String, i64)>(
            "SELECT env_set, COUNT(*)::bigint FROM af_env_vars WHERE tenant_id = $1 GROUP BY env_set ORDER BY env_set ASC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(name, count)| EnvSetSummary { name, var_count: count as usize })
                .collect()
        })
        .map_err(|_| EnvVarError::StoreUnavailable)
    }

    async fn delete_set(&self, tenant_id: &str, set_name: &str) -> Result<(), EnvVarError> {
        let res = sqlx::query(
            "DELETE FROM af_env_vars WHERE tenant_id = $1 AND env_set = $2",
        )
        .bind(tenant_id)
        .bind(set_name)
        .execute(&self.pool)
        .await
        .map_err(|_| EnvVarError::StoreUnavailable)?;

        if res.rows_affected() == 0 {
            Err(EnvVarError::NotFound)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub enum PlatformEnvVarStore {
    Memory(MemoryEnvVarStore),
    Postgres(PostgresEnvVarStore),
}

impl Default for PlatformEnvVarStore {
    fn default() -> Self {
        Self::Memory(MemoryEnvVarStore::default())
    }
}

impl PlatformEnvVarStore {
    pub fn memory() -> Self {
        Self::default()
    }

    pub fn postgres(store: PostgresEnvVarStore) -> Self {
        Self::Postgres(store)
    }
}

impl EnvVarStore for PlatformEnvVarStore {
    async fn set(&self, tenant_id: &str, key: &str, value: &str) -> Result<EnvVarRecord, EnvVarError> {
        match self {
            Self::Memory(s) => s.set(tenant_id, key, value).await,
            Self::Postgres(s) => s.set(tenant_id, key, value).await,
        }
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<EnvVarRecord>, EnvVarError> {
        match self {
            Self::Memory(s) => s.list(tenant_id).await,
            Self::Postgres(s) => s.list(tenant_id).await,
        }
    }

    async fn get(&self, tenant_id: &str, key: &str) -> Result<Option<String>, EnvVarError> {
        match self {
            Self::Memory(s) => s.get(tenant_id, key).await,
            Self::Postgres(s) => s.get(tenant_id, key).await,
        }
    }

    async fn delete(&self, tenant_id: &str, key: &str) -> Result<(), EnvVarError> {
        match self {
            Self::Memory(s) => s.delete(tenant_id, key).await,
            Self::Postgres(s) => s.delete(tenant_id, key).await,
        }
    }

    async fn set_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
        value: &str,
    ) -> Result<EnvVarRecord, EnvVarError> {
        match self {
            Self::Memory(s) => s.set_in(tenant_id, set_name, key, value).await,
            Self::Postgres(s) => s.set_in(tenant_id, set_name, key, value).await,
        }
    }

    async fn list_in(
        &self,
        tenant_id: &str,
        set_name: &str,
    ) -> Result<Vec<EnvVarRecord>, EnvVarError> {
        match self {
            Self::Memory(s) => s.list_in(tenant_id, set_name).await,
            Self::Postgres(s) => s.list_in(tenant_id, set_name).await,
        }
    }

    async fn get_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
    ) -> Result<Option<String>, EnvVarError> {
        match self {
            Self::Memory(s) => s.get_in(tenant_id, set_name, key).await,
            Self::Postgres(s) => s.get_in(tenant_id, set_name, key).await,
        }
    }

    async fn delete_in(
        &self,
        tenant_id: &str,
        set_name: &str,
        key: &str,
    ) -> Result<(), EnvVarError> {
        match self {
            Self::Memory(s) => s.delete_in(tenant_id, set_name, key).await,
            Self::Postgres(s) => s.delete_in(tenant_id, set_name, key).await,
        }
    }

    async fn list_sets(&self, tenant_id: &str) -> Result<Vec<EnvSetSummary>, EnvVarError> {
        match self {
            Self::Memory(s) => s.list_sets(tenant_id).await,
            Self::Postgres(s) => s.list_sets(tenant_id).await,
        }
    }

    async fn delete_set(&self, tenant_id: &str, set_name: &str) -> Result<(), EnvVarError> {
        match self {
            Self::Memory(s) => s.delete_set(tenant_id, set_name).await,
            Self::Postgres(s) => s.delete_set(tenant_id, set_name).await,
        }
    }
}

/// Replace `{{env.KEY}}` patterns in a JSON value using the specified set (defaults to "default").
pub async fn resolve_env_in_json(
    value: &serde_json::Value,
    store: &impl EnvVarStore,
    tenant_id: &str,
    set_name: &str,
) -> serde_json::Value {
    resolve_value(value, store, tenant_id, set_name).await
}

async fn resolve_value(
    value: &serde_json::Value,
    store: &impl EnvVarStore,
    tenant_id: &str,
    set_name: &str,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            serde_json::Value::String(resolve_string(s, store, tenant_id, set_name).await)
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), Box::pin(resolve_value(v, store, tenant_id, set_name)).await);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                out.push(Box::pin(resolve_value(v, store, tenant_id, set_name)).await);
            }
            serde_json::Value::Array(out)
        }
        other => other.clone(),
    }
}

async fn resolve_string(s: &str, store: &impl EnvVarStore, tenant_id: &str, set_name: &str) -> String {
    let mut output = String::new();
    let mut search = s;

    while let Some(start) = search.find("{{env.") {
        output.push_str(&search[..start]);
        let rest = &search[start + "{{env.".len()..];
        if let Some(end) = rest.find("}}") {
            let key = &rest[..end];
            match store.get_in(tenant_id, set_name, key).await {
                Ok(Some(val)) => output.push_str(&val),
                _ => {
                    output.push_str(&search[start..start + "{{env.".len() + end + "}}".len()]);
                }
            }
            search = &rest[end + "}}".len()..];
        } else {
            output.push_str(&search[start..]);
            search = "";
            break;
        }
    }
    output.push_str(search);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_and_list() {
        let store = MemoryEnvVarStore::default();
        store.set("t1", "API_URL", "https://example.com").await.unwrap();
        store.set("t1", "TIMEOUT", "30").await.unwrap();
        let list = store.list("t1").await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].key, "API_URL");
        assert_eq!(list[1].key, "TIMEOUT");
    }

    #[tokio::test]
    async fn get_returns_value() {
        let store = MemoryEnvVarStore::default();
        store.set("t1", "KEY", "value123").await.unwrap();
        assert_eq!(store.get("t1", "KEY").await.unwrap(), Some("value123".to_string()));
        assert_eq!(store.get("t1", "MISSING").await.unwrap(), None);
    }

    #[tokio::test]
    async fn set_overwrites() {
        let store = MemoryEnvVarStore::default();
        store.set("t1", "X", "old").await.unwrap();
        store.set("t1", "X", "new").await.unwrap();
        assert_eq!(store.get("t1", "X").await.unwrap(), Some("new".to_string()));
        assert_eq!(store.list("t1").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn delete_removes_key() {
        let store = MemoryEnvVarStore::default();
        store.set("t1", "KEY", "v").await.unwrap();
        store.delete("t1", "KEY").await.unwrap();
        assert!(store.list("t1").await.unwrap().is_empty());
        assert!(store.delete("t1", "KEY").await.is_err());
    }

    #[tokio::test]
    async fn named_sets_are_isolated() {
        let store = MemoryEnvVarStore::default();
        store.set_in("t1", "production", "API_URL", "https://prod.example.com").await.unwrap();
        store.set_in("t1", "staging", "API_URL", "https://staging.example.com").await.unwrap();
        store.set_in("t1", "production", "TIMEOUT", "60").await.unwrap();

        let prod = store.list_in("t1", "production").await.unwrap();
        assert_eq!(prod.len(), 2);
        let staging = store.list_in("t1", "staging").await.unwrap();
        assert_eq!(staging.len(), 1);

        let sets = store.list_sets("t1").await.unwrap();
        assert_eq!(sets.len(), 2);
        assert!(sets.iter().any(|s| s.name == "production" && s.var_count == 2));
        assert!(sets.iter().any(|s| s.name == "staging" && s.var_count == 1));
    }

    #[tokio::test]
    async fn delete_set_removes_all_vars() {
        let store = MemoryEnvVarStore::default();
        store.set_in("t1", "dev", "A", "1").await.unwrap();
        store.set_in("t1", "dev", "B", "2").await.unwrap();
        store.delete_set("t1", "dev").await.unwrap();
        assert!(store.list_in("t1", "dev").await.unwrap().is_empty());
        assert!(store.delete_set("t1", "dev").await.is_err());
    }

    #[tokio::test]
    async fn resolve_env_in_json_replaces_patterns() {
        let store = MemoryEnvVarStore::default();
        store.set("t1", "BASE_URL", "https://api.example.com").await.unwrap();

        let input = serde_json::json!({ "url": "{{env.BASE_URL}}/path" });
        let resolved = resolve_env_in_json(&input, &store, "t1", DEFAULT_SET).await;
        assert_eq!(resolved["url"], "https://api.example.com/path");
    }

    #[tokio::test]
    async fn resolve_uses_named_set() {
        let store = MemoryEnvVarStore::default();
        store.set_in("t1", "production", "HOST", "prod.example.com").await.unwrap();
        store.set_in("t1", "staging", "HOST", "staging.example.com").await.unwrap();

        let input = serde_json::json!({ "url": "https://{{env.HOST}}" });
        let prod = resolve_env_in_json(&input, &store, "t1", "production").await;
        let staging = resolve_env_in_json(&input, &store, "t1", "staging").await;
        assert_eq!(prod["url"], "https://prod.example.com");
        assert_eq!(staging["url"], "https://staging.example.com");
    }

    #[tokio::test]
    async fn resolve_leaves_unknown_intact() {
        let store = MemoryEnvVarStore::default();
        let input = serde_json::json!({ "x": "{{env.MISSING}}" });
        let resolved = resolve_env_in_json(&input, &store, "t1", DEFAULT_SET).await;
        assert_eq!(resolved["x"], "{{env.MISSING}}");
    }
}
