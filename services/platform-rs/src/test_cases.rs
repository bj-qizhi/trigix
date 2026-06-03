// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;

use crate::execution::unix_now;

fn next_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("tc_{:x}", ts)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub name: String,
    pub input_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Deserialize)]
pub struct CreateTestCaseRequest {
    pub tenant_id: String,
    pub name: String,
    #[serde(default = "default_json")]
    pub input_json: String,
    #[serde(default)]
    pub expected_output: Option<String>,
}
fn default_json() -> String {
    "{}".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateTestCaseRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub input_json: Option<String>,
    #[serde(default)]
    pub expected_output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestCaseError {
    NotFound,
    StoreUnavailable,
}

impl std::fmt::Display for TestCaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "test_case_not_found"),
            Self::StoreUnavailable => write!(f, "test_case_store_unavailable"),
        }
    }
}

// ── In-memory ──────────────────────────────────────────────────────────────

#[derive(Default, Clone)]
pub struct MemoryTestCaseStore {
    inner: Arc<RwLock<HashMap<String, TestCase>>>,
}

impl MemoryTestCaseStore {
    pub async fn create(&self, tc: TestCase) -> Result<TestCase, TestCaseError> {
        let mut m = self
            .inner
            .write()
            .map_err(|_| TestCaseError::StoreUnavailable)?;
        m.insert(tc.id.clone(), tc.clone());
        Ok(tc)
    }

    pub async fn get(&self, id: &str) -> Result<TestCase, TestCaseError> {
        let m = self
            .inner
            .read()
            .map_err(|_| TestCaseError::StoreUnavailable)?;
        m.get(id).cloned().ok_or(TestCaseError::NotFound)
    }

    pub async fn list(&self, tenant_id: &str, workflow_id: &str) -> Vec<TestCase> {
        let m = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let mut cases: Vec<_> = m
            .values()
            .filter(|tc| tc.tenant_id == tenant_id && tc.workflow_id == workflow_id)
            .cloned()
            .collect();
        cases.sort_by_key(|tc| tc.created_at);
        cases
    }

    pub async fn update(
        &self,
        id: &str,
        req: UpdateTestCaseRequest,
    ) -> Result<TestCase, TestCaseError> {
        let mut m = self
            .inner
            .write()
            .map_err(|_| TestCaseError::StoreUnavailable)?;
        let tc = m.get_mut(id).ok_or(TestCaseError::NotFound)?;
        if let Some(name) = req.name {
            tc.name = name;
        }
        if let Some(input) = req.input_json {
            tc.input_json = input;
        }
        if req.expected_output.is_some() {
            tc.expected_output = req.expected_output;
        }
        tc.updated_at = unix_now();
        Ok(tc.clone())
    }

    pub async fn delete(&self, id: &str) -> Result<(), TestCaseError> {
        let mut m = self
            .inner
            .write()
            .map_err(|_| TestCaseError::StoreUnavailable)?;
        m.remove(id).map(|_| ()).ok_or(TestCaseError::NotFound)
    }
}

// ── Postgres ───────────────────────────────────────────────────────────────

pub struct PostgresTestCaseStore {
    pool: PgPool,
}

impl PostgresTestCaseStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, tc: TestCase) -> Result<TestCase, TestCaseError> {
        sqlx::query(
            "INSERT INTO af_test_cases (id, tenant_id, workflow_id, name, input_json, expected_output, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&tc.id)
        .bind(&tc.tenant_id)
        .bind(&tc.workflow_id)
        .bind(&tc.name)
        .bind(&tc.input_json)
        .bind(&tc.expected_output)
        .bind(tc.created_at as i64)
        .bind(tc.updated_at as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| TestCaseError::StoreUnavailable)?;
        Ok(tc)
    }

    pub async fn get(&self, id: &str) -> Result<TestCase, TestCaseError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            tenant_id: String,
            workflow_id: String,
            name: String,
            input_json: String,
            expected_output: Option<String>,
            created_at: i64,
            updated_at: i64,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT id, tenant_id, workflow_id, name, input_json, expected_output, created_at, updated_at \
             FROM af_test_cases WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| TestCaseError::StoreUnavailable)?
        .ok_or(TestCaseError::NotFound)?;
        Ok(TestCase {
            id: row.id,
            tenant_id: row.tenant_id,
            workflow_id: row.workflow_id,
            name: row.name,
            input_json: row.input_json,
            expected_output: row.expected_output,
            created_at: row.created_at as u64,
            updated_at: row.updated_at as u64,
        })
    }

    pub async fn list(&self, tenant_id: &str, workflow_id: &str) -> Vec<TestCase> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            tenant_id: String,
            workflow_id: String,
            name: String,
            input_json: String,
            expected_output: Option<String>,
            created_at: i64,
            updated_at: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, tenant_id, workflow_id, name, input_json, expected_output, created_at, updated_at \
             FROM af_test_cases WHERE tenant_id = $1 AND workflow_id = $2 ORDER BY created_at ASC",
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        rows.into_iter()
            .map(|r| TestCase {
                id: r.id,
                tenant_id: r.tenant_id,
                workflow_id: r.workflow_id,
                name: r.name,
                input_json: r.input_json,
                expected_output: r.expected_output,
                created_at: r.created_at as u64,
                updated_at: r.updated_at as u64,
            })
            .collect()
    }

    pub async fn update(
        &self,
        id: &str,
        req: UpdateTestCaseRequest,
    ) -> Result<TestCase, TestCaseError> {
        let now = unix_now();
        let result = sqlx::query(
            "UPDATE af_test_cases SET \
             name = COALESCE($2, name), \
             input_json = COALESCE($3, input_json), \
             expected_output = COALESCE($4, expected_output), \
             updated_at = $5 \
             WHERE id = $1",
        )
        .bind(id)
        .bind(&req.name)
        .bind(&req.input_json)
        .bind(&req.expected_output)
        .bind(now as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| TestCaseError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            return Err(TestCaseError::NotFound);
        }
        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), TestCaseError> {
        let result = sqlx::query("DELETE FROM af_test_cases WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| TestCaseError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            Err(TestCaseError::NotFound)
        } else {
            Ok(())
        }
    }
}

// ── Platform enum ──────────────────────────────────────────────────────────

pub enum PlatformTestCaseStore {
    Memory(MemoryTestCaseStore),
    Postgres(PostgresTestCaseStore),
}

impl Default for PlatformTestCaseStore {
    fn default() -> Self {
        Self::Memory(MemoryTestCaseStore::default())
    }
}

impl PlatformTestCaseStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryTestCaseStore::default())
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresTestCaseStore::new(pool))
    }

    pub async fn create(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        request: CreateTestCaseRequest,
    ) -> Result<TestCase, TestCaseError> {
        let now = unix_now();
        let tc = TestCase {
            id: next_id(),
            tenant_id: tenant_id.to_string(),
            workflow_id: workflow_id.to_string(),
            name: request.name,
            input_json: request.input_json,
            expected_output: request.expected_output,
            created_at: now,
            updated_at: now,
        };
        match self {
            Self::Memory(s) => s.create(tc).await,
            Self::Postgres(s) => s.create(tc).await,
        }
    }

    pub async fn get(&self, id: &str) -> Result<TestCase, TestCaseError> {
        match self {
            Self::Memory(s) => s.get(id).await,
            Self::Postgres(s) => s.get(id).await,
        }
    }

    pub async fn list(&self, tenant_id: &str, workflow_id: &str) -> Vec<TestCase> {
        match self {
            Self::Memory(s) => s.list(tenant_id, workflow_id).await,
            Self::Postgres(s) => s.list(tenant_id, workflow_id).await,
        }
    }

    pub async fn update(
        &self,
        id: &str,
        req: UpdateTestCaseRequest,
    ) -> Result<TestCase, TestCaseError> {
        match self {
            Self::Memory(s) => s.update(id, req).await,
            Self::Postgres(s) => s.update(id, req).await,
        }
    }

    pub async fn delete(&self, id: &str) -> Result<(), TestCaseError> {
        match self {
            Self::Memory(s) => s.delete(id).await,
            Self::Postgres(s) => s.delete(id).await,
        }
    }
}
