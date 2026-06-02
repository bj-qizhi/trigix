// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;

use crate::execution::unix_now;

fn next_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("form_{:x}", ts)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormToken {
    pub token: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub title: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub created_at: u64,
}

#[derive(Debug, Deserialize)]
pub struct PublishFormRequest {
    pub tenant_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    NotFound,
    StoreUnavailable,
}

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "form_token_not_found"),
            Self::StoreUnavailable => write!(f, "form_store_unavailable"),
        }
    }
}

// ── In-memory ──────────────────────────────────────────────────────────────

#[derive(Default, Clone)]
pub struct MemoryFormStore {
    inner: Arc<RwLock<HashMap<String, FormToken>>>,
}

impl MemoryFormStore {
    pub async fn publish(&self, token: FormToken) -> Result<(), FormError> {
        let mut m = self.inner.write().map_err(|_| FormError::StoreUnavailable)?;
        m.insert(token.token.clone(), token);
        Ok(())
    }

    pub async fn get(&self, token: &str) -> Result<FormToken, FormError> {
        let m = self.inner.read().map_err(|_| FormError::StoreUnavailable)?;
        m.get(token).cloned().ok_or(FormError::NotFound)
    }

    pub async fn list_by_workflow(&self, tenant_id: &str, workflow_id: &str) -> Vec<FormToken> {
        let m = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let mut forms: Vec<_> = m.values()
            .filter(|t| t.tenant_id == tenant_id && t.workflow_id == workflow_id)
            .cloned()
            .collect();
        forms.sort_by_key(|f| f.created_at);
        forms
    }

    pub async fn delete(&self, token: &str) -> Result<(), FormError> {
        let mut m = self.inner.write().map_err(|_| FormError::StoreUnavailable)?;
        m.remove(token).map(|_| ()).ok_or(FormError::NotFound)
    }
}

// ── Postgres ───────────────────────────────────────────────────────────────

pub struct PostgresFormStore {
    pool: PgPool,
}

impl PostgresFormStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn publish(&self, token: FormToken) -> Result<(), FormError> {
        sqlx::query(
            "INSERT INTO af_form_tokens (token, tenant_id, workflow_id, title, description, input_schema, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (token) DO NOTHING",
        )
        .bind(&token.token)
        .bind(&token.tenant_id)
        .bind(&token.workflow_id)
        .bind(&token.title)
        .bind(&token.description)
        .bind(&token.input_schema)
        .bind(token.created_at as i64)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|_| FormError::StoreUnavailable)
    }

    pub async fn get(&self, token: &str) -> Result<FormToken, FormError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            token: String,
            tenant_id: String,
            workflow_id: String,
            title: String,
            description: Option<String>,
            input_schema: serde_json::Value,
            created_at: i64,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT token, tenant_id, workflow_id, title, description, input_schema, created_at \
             FROM af_form_tokens WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| FormError::StoreUnavailable)?
        .ok_or(FormError::NotFound)?;
        Ok(FormToken {
            token: row.token,
            tenant_id: row.tenant_id,
            workflow_id: row.workflow_id,
            title: row.title,
            description: row.description,
            input_schema: row.input_schema,
            created_at: row.created_at as u64,
        })
    }

    pub async fn list_by_workflow(&self, tenant_id: &str, workflow_id: &str) -> Vec<FormToken> {
        #[derive(sqlx::FromRow)]
        struct Row {
            token: String,
            tenant_id: String,
            workflow_id: String,
            title: String,
            description: Option<String>,
            input_schema: serde_json::Value,
            created_at: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT token, tenant_id, workflow_id, title, description, input_schema, created_at \
             FROM af_form_tokens WHERE tenant_id = $1 AND workflow_id = $2 ORDER BY created_at ASC",
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        rows.into_iter().map(|r| FormToken {
            token: r.token,
            tenant_id: r.tenant_id,
            workflow_id: r.workflow_id,
            title: r.title,
            description: r.description,
            input_schema: r.input_schema,
            created_at: r.created_at as u64,
        }).collect()
    }

    pub async fn delete(&self, token: &str) -> Result<(), FormError> {
        let result = sqlx::query("DELETE FROM af_form_tokens WHERE token = $1")
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|_| FormError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            Err(FormError::NotFound)
        } else {
            Ok(())
        }
    }
}

// ── Platform enum ──────────────────────────────────────────────────────────

pub enum PlatformFormStore {
    Memory(MemoryFormStore),
    Postgres(PostgresFormStore),
}

impl Default for PlatformFormStore {
    fn default() -> Self {
        Self::Memory(MemoryFormStore::default())
    }
}

impl PlatformFormStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryFormStore::default())
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresFormStore::new(pool))
    }

    pub async fn publish_form(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        request: PublishFormRequest,
        input_schema: serde_json::Value,
    ) -> Result<FormToken, FormError> {
        let token = next_token();
        let record = FormToken {
            token,
            tenant_id: tenant_id.to_string(),
            workflow_id: workflow_id.to_string(),
            title: request.title,
            description: request.description,
            input_schema,
            created_at: unix_now(),
        };
        match self {
            Self::Memory(s) => s.publish(record.clone()).await?,
            Self::Postgres(s) => s.publish(record.clone()).await?,
        }
        Ok(record)
    }

    pub async fn get(&self, token: &str) -> Result<FormToken, FormError> {
        match self {
            Self::Memory(s) => s.get(token).await,
            Self::Postgres(s) => s.get(token).await,
        }
    }

    pub async fn list_by_workflow(&self, tenant_id: &str, workflow_id: &str) -> Vec<FormToken> {
        match self {
            Self::Memory(s) => s.list_by_workflow(tenant_id, workflow_id).await,
            Self::Postgres(s) => s.list_by_workflow(tenant_id, workflow_id).await,
        }
    }

    pub async fn delete(&self, token: &str) -> Result<(), FormError> {
        match self {
            Self::Memory(s) => s.delete(token).await,
            Self::Postgres(s) => s.delete(token).await,
        }
    }
}
