// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Clone)]
pub struct EmailVerification {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub used_at: Option<i64>,
}

#[derive(Debug)]
pub enum VerificationError {
    NotFound,
    AlreadyUsed,
    Expired,
    StoreUnavailable,
}

pub trait EmailVerificationStore: Send + Sync {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> EmailVerification;
    fn find_by_token(&self, token: &str) -> Option<EmailVerification>;
    fn mark_used(&self, token: &str) -> Result<EmailVerification, VerificationError>;
    fn delete_for_user(&self, user_id: &str);
}

// ── Memory store ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct MemoryEmailVerificationStore {
    records: RwLock<HashMap<String, EmailVerification>>,
}

impl EmailVerificationStore for MemoryEmailVerificationStore {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> EmailVerification {
        let now = unix_now();
        let rec = EmailVerification {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_owned(),
            email: email.to_owned(),
            token: uuid::Uuid::new_v4().to_string(),
            created_at: now,
            expires_at: now + expires_hours * 3600,
            used_at: None,
        };
        let mut map = self.records.write().unwrap();
        self.delete_for_user_inner(&mut map, user_id);
        map.insert(rec.token.clone(), rec.clone());
        rec
    }

    fn find_by_token(&self, token: &str) -> Option<EmailVerification> {
        self.records.read().unwrap().get(token).cloned()
    }

    fn mark_used(&self, token: &str) -> Result<EmailVerification, VerificationError> {
        let mut map = self.records.write().unwrap();
        let rec = map.get_mut(token).ok_or(VerificationError::NotFound)?;
        if rec.used_at.is_some() { return Err(VerificationError::AlreadyUsed); }
        if unix_now() > rec.expires_at { return Err(VerificationError::Expired); }
        rec.used_at = Some(unix_now());
        Ok(rec.clone())
    }

    fn delete_for_user(&self, user_id: &str) {
        let mut map = self.records.write().unwrap();
        self.delete_for_user_inner(&mut map, user_id);
    }
}

impl MemoryEmailVerificationStore {
    fn delete_for_user_inner(&self, map: &mut HashMap<String, EmailVerification>, user_id: &str) {
        map.retain(|_, v| v.user_id != user_id);
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

pub struct PostgresEmailVerificationStore {
    pool: sqlx::PgPool,
}

impl PostgresEmailVerificationStore {
    pub fn new(pool: sqlx::PgPool) -> Self { Self { pool } }
}

#[derive(sqlx::FromRow)]
struct VerificationRow {
    id: String,
    user_id: String,
    email: String,
    token: String,
    created_at: i64,
    expires_at: i64,
    used_at: Option<i64>,
}

impl From<VerificationRow> for EmailVerification {
    fn from(r: VerificationRow) -> Self {
        Self {
            id: r.id, user_id: r.user_id, email: r.email, token: r.token,
            created_at: r.created_at, expires_at: r.expires_at, used_at: r.used_at,
        }
    }
}

impl EmailVerificationStore for PostgresEmailVerificationStore {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> EmailVerification {
        let now = unix_now();
        let id = uuid::Uuid::new_v4().to_string();
        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = now + expires_hours * 3600;
        let pool = self.pool.clone();
        let uid = user_id.to_owned();
        let em = email.to_owned();
        let id2 = id.clone();
        let tok = token.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let _ = sqlx::query("DELETE FROM af_email_verifications WHERE user_id = $1")
                    .bind(&uid).execute(&pool).await;
                let row: VerificationRow = sqlx::query_as::<_, VerificationRow>(
                    "INSERT INTO af_email_verifications (id, user_id, email, token, created_at, expires_at) \
                     VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, user_id, email, token, created_at, expires_at, used_at"
                )
                .bind(&id2).bind(&uid).bind(&em).bind(&tok).bind(now).bind(expires_at)
                .fetch_one(&pool).await.expect("insert email verification");
                EmailVerification::from(row)
            })
        })
    }

    fn find_by_token(&self, token: &str) -> Option<EmailVerification> {
        let pool = self.pool.clone();
        let token = token.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, VerificationRow>(
                    "SELECT id, user_id, email, token, created_at, expires_at, used_at \
                     FROM af_email_verifications WHERE token = $1"
                ).bind(&token).fetch_optional(&pool).await.ok().flatten().map(EmailVerification::from)
            })
        })
    }

    fn mark_used(&self, token: &str) -> Result<EmailVerification, VerificationError> {
        let rec = self.find_by_token(token).ok_or(VerificationError::NotFound)?;
        if rec.used_at.is_some() { return Err(VerificationError::AlreadyUsed); }
        if unix_now() > rec.expires_at { return Err(VerificationError::Expired); }
        let now = unix_now();
        let pool = self.pool.clone();
        let tok = token.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query("UPDATE af_email_verifications SET used_at = $1 WHERE token = $2")
                    .bind(now).bind(&tok).execute(&pool).await
                    .map_err(|_| VerificationError::StoreUnavailable)
            })
        })?;
        Ok(rec)
    }

    fn delete_for_user(&self, user_id: &str) {
        let pool = self.pool.clone();
        let uid = user_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let _ = sqlx::query("DELETE FROM af_email_verifications WHERE user_id = $1")
                    .bind(&uid).execute(&pool).await;
            })
        })
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

pub enum PlatformEmailVerificationStore {
    Memory(Arc<MemoryEmailVerificationStore>),
    Postgres(PostgresEmailVerificationStore),
}

impl PlatformEmailVerificationStore {
    pub fn memory() -> Self { Self::Memory(Arc::new(MemoryEmailVerificationStore::default())) }
    pub fn postgres(pool: sqlx::PgPool) -> Self { Self::Postgres(PostgresEmailVerificationStore::new(pool)) }
}

impl EmailVerificationStore for PlatformEmailVerificationStore {
    fn create(&self, user_id: &str, email: &str, expires_hours: i64) -> EmailVerification {
        match self { Self::Memory(s) => s.create(user_id, email, expires_hours), Self::Postgres(s) => s.create(user_id, email, expires_hours) }
    }
    fn find_by_token(&self, token: &str) -> Option<EmailVerification> {
        match self { Self::Memory(s) => s.find_by_token(token), Self::Postgres(s) => s.find_by_token(token) }
    }
    fn mark_used(&self, token: &str) -> Result<EmailVerification, VerificationError> {
        match self { Self::Memory(s) => s.mark_used(token), Self::Postgres(s) => s.mark_used(token) }
    }
    fn delete_for_user(&self, user_id: &str) {
        match self { Self::Memory(s) => s.delete_for_user(user_id), Self::Postgres(s) => s.delete_for_user(user_id) }
    }
}
