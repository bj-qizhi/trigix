// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── Domain types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub id: String,
    pub email: String,
    pub token: String,
    pub role: String,
    pub tenant_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub used_at: Option<i64>,
}

impl Invitation {
    pub fn is_valid(&self) -> bool {
        self.used_at.is_none() && self.expires_at > unix_now()
    }
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum InviteError {
    NotFound,
    AlreadyUsed,
    Expired,
}

impl std::fmt::Display for InviteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound    => write!(f, "invitation not found"),
            Self::AlreadyUsed => write!(f, "invitation already used"),
            Self::Expired     => write!(f, "invitation has expired"),
        }
    }
}

// ── Store trait ───────────────────────────────────────────────────────────────

pub trait InviteStore: Send + Sync {
    fn create(&self, email: &str, role: &str, tenant_id: &str, expires_hours: u64) -> Invitation;
    fn find_by_token(&self, token: &str) -> Option<Invitation>;
    fn mark_used(&self, token: &str) -> Result<Invitation, InviteError>;
    fn list_by_tenant(&self, tenant_id: &str) -> Vec<Invitation>;
    fn delete(&self, id: &str) -> Result<(), InviteError>;
}

// ── Memory store ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct MemoryInviteStore {
    invitations: RwLock<HashMap<String, Invitation>>,
}

impl InviteStore for MemoryInviteStore {
    fn create(&self, email: &str, role: &str, tenant_id: &str, expires_hours: u64) -> Invitation {
        let now = unix_now();
        let inv = Invitation {
            id: uuid::Uuid::new_v4().to_string(),
            email: email.to_owned(),
            token: uuid::Uuid::new_v4().to_string(),
            role: role.to_owned(),
            tenant_id: tenant_id.to_owned(),
            created_at: now,
            expires_at: now + (expires_hours as i64 * 3600),
            used_at: None,
        };
        self.invitations.write().unwrap().insert(inv.token.clone(), inv.clone());
        inv
    }

    fn find_by_token(&self, token: &str) -> Option<Invitation> {
        self.invitations.read().unwrap().get(token).cloned()
    }

    fn mark_used(&self, token: &str) -> Result<Invitation, InviteError> {
        let mut map = self.invitations.write().unwrap();
        let inv = map.get_mut(token).ok_or(InviteError::NotFound)?;
        if inv.used_at.is_some() { return Err(InviteError::AlreadyUsed); }
        if inv.expires_at <= unix_now() { return Err(InviteError::Expired); }
        inv.used_at = Some(unix_now());
        Ok(inv.clone())
    }

    fn list_by_tenant(&self, tenant_id: &str) -> Vec<Invitation> {
        let mut list: Vec<_> = self.invitations.read().unwrap().values()
            .filter(|inv| inv.tenant_id == tenant_id)
            .cloned()
            .collect();
        list.sort_by_key(|inv| std::cmp::Reverse(inv.created_at));
        list
    }

    fn delete(&self, id: &str) -> Result<(), InviteError> {
        let mut map = self.invitations.write().unwrap();
        let token = map.values().find(|inv| inv.id == id).map(|inv| inv.token.clone())
            .ok_or(InviteError::NotFound)?;
        map.remove(&token);
        Ok(())
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

pub struct PostgresInviteStore {
    pool: sqlx::PgPool,
}

impl PostgresInviteStore {
    pub fn new(pool: sqlx::PgPool) -> Self { Self { pool } }
}

#[derive(sqlx::FromRow)]
struct InvitationRow {
    id: String,
    email: String,
    token: String,
    role: String,
    tenant_id: String,
    created_at: i64,
    expires_at: i64,
    used_at: Option<i64>,
}

impl From<InvitationRow> for Invitation {
    fn from(r: InvitationRow) -> Self {
        Self { id: r.id, email: r.email, token: r.token, role: r.role,
               tenant_id: r.tenant_id, created_at: r.created_at,
               expires_at: r.expires_at, used_at: r.used_at }
    }
}

const COLS: &str = "id, email, token, role, tenant_id, created_at, expires_at, used_at";

impl InviteStore for PostgresInviteStore {
    fn create(&self, email: &str, role: &str, tenant_id: &str, expires_hours: u64) -> Invitation {
        let now = unix_now();
        let inv = Invitation {
            id: uuid::Uuid::new_v4().to_string(),
            email: email.to_owned(),
            token: uuid::Uuid::new_v4().to_string(),
            role: role.to_owned(),
            tenant_id: tenant_id.to_owned(),
            created_at: now,
            expires_at: now + (expires_hours as i64 * 3600),
            used_at: None,
        };
        let pool = self.pool.clone();
        let inv2 = inv.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let _ = sqlx::query(
                    "INSERT INTO af_invitations (id, email, token, role, tenant_id, created_at, expires_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7)"
                ).bind(&inv2.id).bind(&inv2.email).bind(&inv2.token).bind(&inv2.role)
                 .bind(&inv2.tenant_id).bind(inv2.created_at).bind(inv2.expires_at)
                 .execute(&pool).await;
            })
        });
        inv
    }

    fn find_by_token(&self, token: &str) -> Option<Invitation> {
        let pool = self.pool.clone();
        let token = token.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, InvitationRow>(
                    &format!("SELECT {COLS} FROM af_invitations WHERE token = $1")
                ).bind(&token).fetch_optional(&pool).await.ok().flatten().map(Invitation::from)
            })
        })
    }

    fn mark_used(&self, token: &str) -> Result<Invitation, InviteError> {
        let inv = self.find_by_token(token).ok_or(InviteError::NotFound)?;
        if inv.used_at.is_some() { return Err(InviteError::AlreadyUsed); }
        if inv.expires_at <= unix_now() { return Err(InviteError::Expired); }
        let pool = self.pool.clone();
        let now = unix_now();
        let token = token.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query("UPDATE af_invitations SET used_at = $1 WHERE token = $2")
                    .bind(now).bind(&token).execute(&pool).await.ok();
            })
        });
        Ok(Invitation { used_at: Some(now), ..inv })
    }

    fn list_by_tenant(&self, tenant_id: &str) -> Vec<Invitation> {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, InvitationRow>(
                    &format!("SELECT {COLS} FROM af_invitations WHERE tenant_id = $1 ORDER BY created_at DESC")
                ).bind(&tenant_id).fetch_all(&pool).await.unwrap_or_default()
                .into_iter().map(Invitation::from).collect()
            })
        })
    }

    fn delete(&self, id: &str) -> Result<(), InviteError> {
        let pool = self.pool.clone();
        let id = id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let r = sqlx::query("DELETE FROM af_invitations WHERE id = $1")
                    .bind(&id).execute(&pool).await.map_err(|_| InviteError::NotFound)?;
                if r.rows_affected() == 0 { Err(InviteError::NotFound) } else { Ok(()) }
            })
        })
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

pub enum PlatformInviteStore {
    Memory(Arc<MemoryInviteStore>),
    Postgres(PostgresInviteStore),
}

impl PlatformInviteStore {
    pub fn memory() -> Self { Self::Memory(Arc::new(MemoryInviteStore::default())) }
    pub fn postgres(pool: sqlx::PgPool) -> Self { Self::Postgres(PostgresInviteStore::new(pool)) }
}

impl InviteStore for PlatformInviteStore {
    fn create(&self, email: &str, role: &str, tenant_id: &str, expires_hours: u64) -> Invitation {
        match self { Self::Memory(s) => s.create(email, role, tenant_id, expires_hours),
                     Self::Postgres(s) => s.create(email, role, tenant_id, expires_hours) }
    }
    fn find_by_token(&self, token: &str) -> Option<Invitation> {
        match self { Self::Memory(s) => s.find_by_token(token),
                     Self::Postgres(s) => s.find_by_token(token) }
    }
    fn mark_used(&self, token: &str) -> Result<Invitation, InviteError> {
        match self { Self::Memory(s) => s.mark_used(token),
                     Self::Postgres(s) => s.mark_used(token) }
    }
    fn list_by_tenant(&self, tenant_id: &str) -> Vec<Invitation> {
        match self { Self::Memory(s) => s.list_by_tenant(tenant_id),
                     Self::Postgres(s) => s.list_by_tenant(tenant_id) }
    }
    fn delete(&self, id: &str) -> Result<(), InviteError> {
        match self { Self::Memory(s) => s.delete(id),
                     Self::Postgres(s) => s.delete(id) }
    }
}

impl Default for PlatformInviteStore {
    fn default() -> Self { Self::memory() }
}
