// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use sqlx::PgPool;

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn next_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PasswordReset {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub used_at: Option<i64>,
}

impl PasswordReset {
    pub fn is_valid(&self) -> bool {
        self.used_at.is_none() && self.expires_at > unix_now()
    }
}

#[derive(Debug)]
pub enum ResetError {
    NotFound,
    AlreadyUsed,
    Expired,
    StoreUnavailable,
}

pub trait PasswordResetStore: Send + Sync {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> PasswordReset;
    fn find_by_token(&self, token: &str) -> Option<PasswordReset>;
    fn mark_used(&self, token: &str) -> Result<PasswordReset, ResetError>;
    fn delete_for_user(&self, user_id: &str);
}

// ── Memory impl ───────────────────────────────────────────────────────────────

pub struct MemoryPasswordResetStore {
    records: RwLock<HashMap<String, PasswordReset>>,
}

impl MemoryPasswordResetStore {
    pub fn new() -> Self {
        Self { records: RwLock::new(HashMap::new()) }
    }
}

impl PasswordResetStore for MemoryPasswordResetStore {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> PasswordReset {
        let record = PasswordReset {
            id: next_id(),
            user_id: user_id.to_string(),
            email: email.to_string(),
            token: uuid::Uuid::new_v4().to_string(),
            created_at: unix_now(),
            expires_at: unix_now() + expires_hours * 3600,
            used_at: None,
        };
        self.records.write().unwrap().insert(record.token.clone(), record.clone());
        record
    }

    fn find_by_token(&self, token: &str) -> Option<PasswordReset> {
        self.records.read().unwrap().get(token).cloned()
    }

    fn mark_used(&self, token: &str) -> Result<PasswordReset, ResetError> {
        let mut map = self.records.write().unwrap();
        let record = map.get_mut(token).ok_or(ResetError::NotFound)?;
        if record.used_at.is_some() { return Err(ResetError::AlreadyUsed); }
        if record.expires_at <= unix_now() { return Err(ResetError::Expired); }
        record.used_at = Some(unix_now());
        Ok(record.clone())
    }

    fn delete_for_user(&self, user_id: &str) {
        self.records.write().unwrap().retain(|_, v| v.user_id != user_id);
    }
}

// ── Postgres impl ─────────────────────────────────────────────────────────────

pub struct PostgresPasswordResetStore {
    pool: PgPool,
}

impl PostgresPasswordResetStore {
    pub fn new(pool: PgPool) -> Self { Self { pool } }
}

impl PasswordResetStore for PostgresPasswordResetStore {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> PasswordReset {
        let record = PasswordReset {
            id: next_id(),
            user_id: user_id.to_string(),
            email: email.to_string(),
            token: uuid::Uuid::new_v4().to_string(),
            created_at: unix_now(),
            expires_at: unix_now() + expires_hours * 3600,
            used_at: None,
        };
        let pool = self.pool.clone();
        let r = record.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _ = sqlx::query(
                    "INSERT INTO af_password_resets (id, user_id, email, token, created_at, expires_at) \
                     VALUES ($1, $2, $3, $4, $5, $6)"
                )
                .bind(&r.id).bind(&r.user_id).bind(&r.email)
                .bind(&r.token).bind(r.created_at).bind(r.expires_at)
                .execute(&pool).await;
            })
        });
        record
    }

    fn find_by_token(&self, token: &str) -> Option<PasswordReset> {
        let pool = self.pool.clone();
        let token = token.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query_as::<_, PostgresResetRow>(
                    "SELECT id, user_id, email, token, created_at, expires_at, used_at \
                     FROM af_password_resets WHERE token = $1"
                )
                .bind(&token)
                .fetch_optional(&pool).await.ok().flatten().map(|r| r.into_record())
            })
        })
    }

    fn mark_used(&self, token: &str) -> Result<PasswordReset, ResetError> {
        let existing = self.find_by_token(token).ok_or(ResetError::NotFound)?;
        if existing.used_at.is_some() { return Err(ResetError::AlreadyUsed); }
        if existing.expires_at <= unix_now() { return Err(ResetError::Expired); }
        let pool = self.pool.clone();
        let token = token.to_string();
        let now = unix_now();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query("UPDATE af_password_resets SET used_at = $1 WHERE token = $2")
                    .bind(now).bind(&token).execute(&pool).await.ok();
            })
        });
        Ok(PasswordReset { used_at: Some(now), ..existing })
    }

    fn delete_for_user(&self, user_id: &str) {
        let pool = self.pool.clone();
        let uid = user_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _ = sqlx::query("DELETE FROM af_password_resets WHERE user_id = $1")
                    .bind(&uid).execute(&pool).await;
            })
        });
    }
}

#[derive(sqlx::FromRow)]
struct PostgresResetRow {
    id: String,
    user_id: String,
    email: String,
    token: String,
    created_at: i64,
    expires_at: i64,
    #[sqlx(default)]
    used_at: Option<i64>,
}

impl PostgresResetRow {
    fn into_record(self) -> PasswordReset {
        PasswordReset {
            id: self.id, user_id: self.user_id, email: self.email,
            token: self.token, created_at: self.created_at,
            expires_at: self.expires_at, used_at: self.used_at,
        }
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

pub enum PlatformPasswordResetStore {
    Memory(Arc<MemoryPasswordResetStore>),
    Postgres(PostgresPasswordResetStore),
}

impl PlatformPasswordResetStore {
    pub fn memory() -> Self {
        Self::Memory(Arc::new(MemoryPasswordResetStore::new()))
    }
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresPasswordResetStore::new(pool))
    }
}

impl PasswordResetStore for PlatformPasswordResetStore {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> PasswordReset {
        match self { Self::Memory(s) => s.create(user_id, email, expires_hours), Self::Postgres(s) => s.create(user_id, email, expires_hours) }
    }
    fn find_by_token(&self, token: &str) -> Option<PasswordReset> {
        match self { Self::Memory(s) => s.find_by_token(token), Self::Postgres(s) => s.find_by_token(token) }
    }
    fn mark_used(&self, token: &str) -> Result<PasswordReset, ResetError> {
        match self { Self::Memory(s) => s.mark_used(token), Self::Postgres(s) => s.mark_used(token) }
    }
    fn delete_for_user(&self, user_id: &str) {
        match self { Self::Memory(s) => s.delete_for_user(user_id), Self::Postgres(s) => s.delete_for_user(user_id) }
    }
}
