// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    /// First 12 chars of the raw key for display purposes.
    pub prefix: String,
    #[serde(skip_serializing)]
    pub key_hash: String,
    pub created_at: u64,
}

/// Generates a new random API key in the form `af_<uuid1><uuid2>`.
pub fn generate_api_key() -> String {
    let a = uuid::Uuid::new_v4().to_string().replace('-', "");
    let b = uuid::Uuid::new_v4().to_string().replace('-', "");
    format!("af_{a}{b}")
}

pub fn hash_api_key(key: &str) -> String {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    hex::encode(h.finalize())
}

fn next_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── In-memory ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MemoryApiKeyStore {
    // key: id → record
    keys: Arc<RwLock<HashMap<String, ApiKeyRecord>>>,
}

impl MemoryApiKeyStore {
    pub async fn create(&self, tenant_id: &str, name: &str, raw_key: &str) -> ApiKeyRecord {
        let record = ApiKeyRecord {
            id: next_id(),
            tenant_id: tenant_id.to_string(),
            name: name.to_string(),
            prefix: raw_key.chars().take(12).collect(),
            key_hash: hash_api_key(raw_key),
            created_at: unix_now(),
        };
        self.keys
            .write()
            .unwrap()
            .insert(record.id.clone(), record.clone());
        record
    }

    pub async fn list(&self, tenant_id: &str) -> Vec<ApiKeyRecord> {
        let mut out: Vec<_> = self
            .keys
            .read()
            .unwrap()
            .values()
            .filter(|r| r.tenant_id == tenant_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        out
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        let mut map = self.keys.write().unwrap();
        if map.get(id).map_or(false, |r| r.tenant_id == tenant_id) {
            map.remove(id);
            true
        } else {
            false
        }
    }

    /// Validate a raw key; returns the associated record if found.
    pub async fn validate(&self, raw_key: &str) -> Option<ApiKeyRecord> {
        let hash = hash_api_key(raw_key);
        self.keys
            .read()
            .unwrap()
            .values()
            .find(|r| r.key_hash == hash)
            .cloned()
    }
}

// ── Postgres ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PostgresApiKeyStore {
    pool: PgPool,
}

impl PostgresApiKeyStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, tenant_id: &str, name: &str, raw_key: &str) -> ApiKeyRecord {
        let id = next_id();
        let now = unix_now();
        let prefix: String = raw_key.chars().take(12).collect();
        let key_hash = hash_api_key(raw_key);
        let _ = sqlx::query(
            r#"INSERT INTO af_api_keys (id, tenant_id, name, prefix, key_hash, created_at)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(name)
        .bind(&prefix)
        .bind(&key_hash)
        .bind(now as i64)
        .execute(&self.pool)
        .await;
        ApiKeyRecord {
            id,
            tenant_id: tenant_id.to_string(),
            name: name.to_string(),
            prefix,
            key_hash,
            created_at: now,
        }
    }

    pub async fn list(&self, tenant_id: &str) -> Vec<ApiKeyRecord> {
        sqlx::query_as::<_, (String, String, String, String, String, i64)>(
            r#"SELECT id, tenant_id, name, prefix, key_hash, created_at
               FROM af_api_keys WHERE tenant_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(
            |(id, tenant_id, name, prefix, key_hash, created_at)| ApiKeyRecord {
                id,
                tenant_id,
                name,
                prefix,
                key_hash,
                created_at: created_at as u64,
            },
        )
        .collect()
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        sqlx::query(r#"DELETE FROM af_api_keys WHERE tenant_id = $1 AND id = $2"#)
            .bind(tenant_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .unwrap_or(false)
    }

    pub async fn validate(&self, raw_key: &str) -> Option<ApiKeyRecord> {
        let hash = hash_api_key(raw_key);
        sqlx::query_as::<_, (String, String, String, String, String, i64)>(
            r#"SELECT id, tenant_id, name, prefix, key_hash, created_at
               FROM af_api_keys WHERE key_hash = $1"#,
        )
        .bind(&hash)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(
            |(id, tenant_id, name, prefix, key_hash, created_at)| ApiKeyRecord {
                id,
                tenant_id,
                name,
                prefix,
                key_hash,
                created_at: created_at as u64,
            },
        )
    }
}

// ── Platform enum ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformApiKeyStore {
    Memory(MemoryApiKeyStore),
    Postgres(PostgresApiKeyStore),
}

impl Default for PlatformApiKeyStore {
    fn default() -> Self {
        Self::Memory(MemoryApiKeyStore::default())
    }
}

impl PlatformApiKeyStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryApiKeyStore::default())
    }
    pub fn postgres(s: PostgresApiKeyStore) -> Self {
        Self::Postgres(s)
    }

    pub async fn create(&self, tenant_id: &str, name: &str, raw_key: &str) -> ApiKeyRecord {
        match self {
            Self::Memory(s) => s.create(tenant_id, name, raw_key).await,
            Self::Postgres(s) => s.create(tenant_id, name, raw_key).await,
        }
    }

    pub async fn list(&self, tenant_id: &str) -> Vec<ApiKeyRecord> {
        match self {
            Self::Memory(s) => s.list(tenant_id).await,
            Self::Postgres(s) => s.list(tenant_id).await,
        }
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        match self {
            Self::Memory(s) => s.delete(tenant_id, id).await,
            Self::Postgres(s) => s.delete(tenant_id, id).await,
        }
    }

    pub async fn validate(&self, raw_key: &str) -> Option<ApiKeyRecord> {
        match self {
            Self::Memory(s) => s.validate(raw_key).await,
            Self::Postgres(s) => s.validate(raw_key).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_list_api_keys() {
        let store = MemoryApiKeyStore::default();
        let raw = generate_api_key();
        let rec = store.create("tenant-1", "CI Key", &raw).await;
        assert!(rec.prefix.starts_with("af_"));
        assert_eq!(rec.name, "CI Key");

        let list = store.list("tenant-1").await;
        assert_eq!(list.len(), 1);
        assert!(store.list("tenant-2").await.is_empty());
    }

    #[tokio::test]
    async fn validate_correct_key() {
        let store = MemoryApiKeyStore::default();
        let raw = generate_api_key();
        store.create("tenant-1", "My Key", &raw).await;

        let found = store.validate(&raw).await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().tenant_id, "tenant-1");

        let not_found = store.validate("af_wrongkey").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn revoke_api_key() {
        let store = MemoryApiKeyStore::default();
        let raw = generate_api_key();
        let rec = store.create("tenant-1", "Temp", &raw).await;
        assert!(store.delete("tenant-1", &rec.id).await);
        assert!(store.validate(&raw).await.is_none());
        assert!(!store.delete("tenant-1", &rec.id).await);
    }
}
