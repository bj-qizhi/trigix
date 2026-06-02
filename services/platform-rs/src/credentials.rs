// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRecord {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl From<&CredentialRecord> for CredentialSummary {
    fn from(r: &CredentialRecord) -> Self {
        Self {
            id: r.id.clone(),
            name: r.name.clone(),
            description: r.description.clone(),
            expires_at: r.expires_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    NotFound,
    NameTaken,
    StoreUnavailable,
}

pub trait CredentialStore: Clone + Send + Sync + 'static {
    fn create(
        &self,
        tenant_id: &str,
        name: &str,
        value: &str,
    ) -> impl std::future::Future<Output = Result<CredentialSummary, CredentialError>> + Send;

    fn list(
        &self,
        tenant_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<CredentialSummary>, CredentialError>> + Send;

    fn get_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Option<String>, CredentialError>> + Send;

    fn delete(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> impl std::future::Future<Output = Result<(), CredentialError>> + Send;

    fn update(
        &self,
        tenant_id: &str,
        id: &str,
        new_value: Option<&str>,
        description: Option<Option<&str>>,
        expires_at: Option<Option<u64>>,
    ) -> impl std::future::Future<Output = Result<CredentialSummary, CredentialError>> + Send;

    fn list_expiring(
        &self,
        tenant_id: &str,
        before_unix: u64,
    ) -> impl std::future::Future<Output = Result<Vec<CredentialSummary>, CredentialError>> + Send;
}

fn next_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("cred-{:x}", nanos)
}

fn key(tenant_id: &str, id: &str) -> String {
    format!("{}/{}", tenant_id, id)
}

#[derive(Clone, Default)]
pub struct MemoryCredentialStore {
    records: Arc<RwLock<HashMap<String, CredentialRecord>>>,
}

impl CredentialStore for MemoryCredentialStore {
    async fn create(
        &self,
        tenant_id: &str,
        name: &str,
        value: &str,
    ) -> Result<CredentialSummary, CredentialError> {
        let mut records = self.records.write().map_err(|_| CredentialError::StoreUnavailable)?;
        let prefix = format!("{}/", tenant_id);
        let already_taken = records
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .any(|(_, r)| r.name == name);
        if already_taken {
            return Err(CredentialError::NameTaken);
        }
        let now = unix_now();
        let id = next_id();
        let record = CredentialRecord {
            id: id.clone(),
            name: name.to_string(),
            value: value.to_string(),
            description: None,
            expires_at: None,
            created_at: now,
            updated_at: now,
        };
        records.insert(key(tenant_id, &id), record.clone());
        Ok(CredentialSummary::from(&record))
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<CredentialSummary>, CredentialError> {
        let prefix = format!("{}/", tenant_id);
        let records = self.records.read().map_err(|_| CredentialError::StoreUnavailable)?;
        let mut out: Vec<CredentialSummary> = records
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(_, r)| CredentialSummary::from(r))
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn get_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> Result<Option<String>, CredentialError> {
        let prefix = format!("{}/", tenant_id);
        let records = self.records.read().map_err(|_| CredentialError::StoreUnavailable)?;
        let value = records
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .find(|(_, r)| r.name == name)
            .map(|(_, r)| r.value.clone());
        Ok(value)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), CredentialError> {
        let mut records = self.records.write().map_err(|_| CredentialError::StoreUnavailable)?;
        records.remove(&key(tenant_id, id)).ok_or(CredentialError::NotFound)?;
        Ok(())
    }

    async fn update(
        &self,
        tenant_id: &str,
        id: &str,
        new_value: Option<&str>,
        description: Option<Option<&str>>,
        expires_at: Option<Option<u64>>,
    ) -> Result<CredentialSummary, CredentialError> {
        let mut records = self.records.write().map_err(|_| CredentialError::StoreUnavailable)?;
        let rec = records.get_mut(&key(tenant_id, id)).ok_or(CredentialError::NotFound)?;
        if let Some(v) = new_value { rec.value = v.to_string(); }
        if let Some(d) = description { rec.description = d.map(str::to_string); }
        if let Some(e) = expires_at { rec.expires_at = e; }
        rec.updated_at = unix_now();
        Ok(CredentialSummary::from(rec as &CredentialRecord))
    }

    async fn list_expiring(
        &self,
        tenant_id: &str,
        before_unix: u64,
    ) -> Result<Vec<CredentialSummary>, CredentialError> {
        let prefix = format!("{}/", tenant_id);
        let records = self.records.read().map_err(|_| CredentialError::StoreUnavailable)?;
        let mut out: Vec<CredentialSummary> = records
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter(|(_, r)| r.expires_at.map_or(false, |e| e <= before_unix))
            .map(|(_, r)| CredentialSummary::from(r))
            .collect();
        out.sort_by_key(|c| c.expires_at.unwrap_or(u64::MAX));
        Ok(out)
    }
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|e| e.code())
        .as_deref() == Some("23505")
}

#[derive(Clone)]
pub struct PostgresCredentialStore {
    pool: PgPool,
}

impl PostgresCredentialStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl CredentialStore for PostgresCredentialStore {
    async fn create(
        &self,
        tenant_id: &str,
        name: &str,
        value: &str,
    ) -> Result<CredentialSummary, CredentialError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = unix_now() as i64;
        let result = sqlx::query(
            "INSERT INTO af_credentials (id, tenant_id, name, secret, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $5)",
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(name)
        .bind(value)
        .bind(now)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(CredentialSummary { id, name: name.to_string(), description: None, expires_at: None, created_at: now as u64, updated_at: now as u64 }),
            Err(e) if is_unique_violation(&e) => Err(CredentialError::NameTaken),
            Err(_) => Err(CredentialError::StoreUnavailable),
        }
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<CredentialSummary>, CredentialError> {
        sqlx::query_as::<_, (String, String, Option<String>, Option<i64>, i64, i64)>(
            "SELECT id, name, description, expires_at, created_at, updated_at FROM af_credentials WHERE tenant_id = $1 ORDER BY name ASC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(|(id, name, description, expires_at, created_at, updated_at)| CredentialSummary {
            id, name, description,
            expires_at: expires_at.map(|e| e as u64),
            created_at: created_at as u64,
            updated_at: updated_at as u64,
        }).collect())
        .map_err(|_| CredentialError::StoreUnavailable)
    }

    async fn get_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> Result<Option<String>, CredentialError> {
        sqlx::query_as::<_, (String,)>(
            "SELECT secret FROM af_credentials WHERE tenant_id = $1 AND name = $2",
        )
        .bind(tenant_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(|(s,)| s))
        .map_err(|_| CredentialError::StoreUnavailable)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), CredentialError> {
        let res = sqlx::query(
            "DELETE FROM af_credentials WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|_| CredentialError::StoreUnavailable)?;

        if res.rows_affected() == 0 {
            Err(CredentialError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn update(
        &self,
        tenant_id: &str,
        id: &str,
        new_value: Option<&str>,
        description: Option<Option<&str>>,
        expires_at: Option<Option<u64>>,
    ) -> Result<CredentialSummary, CredentialError> {
        let now = unix_now() as i64;
        // Build dynamic update — always update updated_at
        if let Some(v) = new_value {
            sqlx::query("UPDATE af_credentials SET secret = $1, updated_at = $2 WHERE tenant_id = $3 AND id = $4")
                .bind(v).bind(now).bind(tenant_id).bind(id)
                .execute(&self.pool).await.map_err(|_| CredentialError::StoreUnavailable)?;
        }
        if let Some(d) = description {
            sqlx::query("UPDATE af_credentials SET description = $1, updated_at = $2 WHERE tenant_id = $3 AND id = $4")
                .bind(d).bind(now).bind(tenant_id).bind(id)
                .execute(&self.pool).await.map_err(|_| CredentialError::StoreUnavailable)?;
        }
        if let Some(e) = expires_at {
            sqlx::query("UPDATE af_credentials SET expires_at = $1, updated_at = $2 WHERE tenant_id = $3 AND id = $4")
                .bind(e.map(|v| v as i64)).bind(now).bind(tenant_id).bind(id)
                .execute(&self.pool).await.map_err(|_| CredentialError::StoreUnavailable)?;
        }
        // Always touch updated_at when any field was changed
        let row = sqlx::query_as::<_, (String, String, Option<String>, Option<i64>, i64, i64)>(
            "SELECT id, name, description, expires_at, created_at, updated_at FROM af_credentials WHERE tenant_id = $1 AND id = $2"
        )
        .bind(tenant_id).bind(id).fetch_optional(&self.pool).await.map_err(|_| CredentialError::StoreUnavailable)?
        .ok_or(CredentialError::NotFound)?;
        Ok(CredentialSummary {
            id: row.0, name: row.1, description: row.2,
            expires_at: row.3.map(|e| e as u64),
            created_at: row.4 as u64,
            updated_at: row.5 as u64,
        })
    }

    async fn list_expiring(
        &self,
        tenant_id: &str,
        before_unix: u64,
    ) -> Result<Vec<CredentialSummary>, CredentialError> {
        sqlx::query_as::<_, (String, String, Option<String>, Option<i64>, i64, i64)>(
            "SELECT id, name, description, expires_at, created_at, updated_at FROM af_credentials WHERE tenant_id = $1 AND expires_at IS NOT NULL AND expires_at <= $2 ORDER BY expires_at ASC",
        )
        .bind(tenant_id)
        .bind(before_unix as i64)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(|(id, name, description, expires_at, created_at, updated_at)| CredentialSummary {
            id, name, description,
            expires_at: expires_at.map(|e| e as u64),
            created_at: created_at as u64,
            updated_at: updated_at as u64,
        }).collect())
        .map_err(|_| CredentialError::StoreUnavailable)
    }
}

#[derive(Clone)]
pub enum PlatformCredentialStore {
    Memory(MemoryCredentialStore),
    Postgres(PostgresCredentialStore),
}

impl Default for PlatformCredentialStore {
    fn default() -> Self {
        Self::Memory(MemoryCredentialStore::default())
    }
}

impl PlatformCredentialStore {
    pub fn memory() -> Self {
        Self::default()
    }

    pub fn postgres(store: PostgresCredentialStore) -> Self {
        Self::Postgres(store)
    }
}

/// Resolve `{{credential.<name>}}` placeholders in a JSON value by looking up from the store.
/// Returns (resolved_value, any_resolved: bool).
pub async fn resolve_credentials_in_json<S: CredentialStore>(
    value: &serde_json::Value,
    store: &S,
    tenant_id: &str,
) -> (serde_json::Value, bool) {
    let mut any = false;
    let resolved = resolve_json_value(value, store, tenant_id, &mut any).await;
    (resolved, any)
}

async fn resolve_json_value<S: CredentialStore>(
    value: &serde_json::Value,
    store: &S,
    tenant_id: &str,
    any: &mut bool,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            if let Some(rest) = s.strip_prefix("{{credential.").and_then(|r| r.strip_suffix("}}")) {
                if let Ok(Some(v)) = store.get_by_name(tenant_id, rest).await {
                    *any = true;
                    return serde_json::Value::String(v);
                }
            }
            value.clone()
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), Box::pin(resolve_json_value(v, store, tenant_id, any)).await);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                out.push(Box::pin(resolve_json_value(v, store, tenant_id, any)).await);
            }
            serde_json::Value::Array(out)
        }
        _ => value.clone(),
    }
}

impl CredentialStore for PlatformCredentialStore {
    async fn create(
        &self,
        tenant_id: &str,
        name: &str,
        value: &str,
    ) -> Result<CredentialSummary, CredentialError> {
        match self {
            Self::Memory(s) => s.create(tenant_id, name, value).await,
            Self::Postgres(s) => s.create(tenant_id, name, value).await,
        }
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<CredentialSummary>, CredentialError> {
        match self {
            Self::Memory(s) => s.list(tenant_id).await,
            Self::Postgres(s) => s.list(tenant_id).await,
        }
    }

    async fn get_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> Result<Option<String>, CredentialError> {
        match self {
            Self::Memory(s) => s.get_by_name(tenant_id, name).await,
            Self::Postgres(s) => s.get_by_name(tenant_id, name).await,
        }
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), CredentialError> {
        match self {
            Self::Memory(s) => s.delete(tenant_id, id).await,
            Self::Postgres(s) => s.delete(tenant_id, id).await,
        }
    }

    async fn update(
        &self,
        tenant_id: &str,
        id: &str,
        new_value: Option<&str>,
        description: Option<Option<&str>>,
        expires_at: Option<Option<u64>>,
    ) -> Result<CredentialSummary, CredentialError> {
        match self {
            Self::Memory(s) => s.update(tenant_id, id, new_value, description, expires_at).await,
            Self::Postgres(s) => s.update(tenant_id, id, new_value, description, expires_at).await,
        }
    }

    async fn list_expiring(
        &self,
        tenant_id: &str,
        before_unix: u64,
    ) -> Result<Vec<CredentialSummary>, CredentialError> {
        match self {
            Self::Memory(s) => s.list_expiring(tenant_id, before_unix).await,
            Self::Postgres(s) => s.list_expiring(tenant_id, before_unix).await,
        }
    }
}
