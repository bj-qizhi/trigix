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

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub name: Option<String>,
    pub tenant_id: String,
    pub created_at: i64,
    #[serde(default)]
    pub email_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub tenant_id: String,
    pub created_at: i64,
    pub email_verified: bool,
}

impl From<&User> for PublicUser {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            email: u.email.clone(),
            name: u.name.clone(),
            tenant_id: u.tenant_id.clone(),
            created_at: u.created_at,
            email_verified: u.email_verified,
        }
    }
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum UserError {
    EmailAlreadyExists,
    NotFound,
    InvalidCredentials,
    HashError(String),
}

impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmailAlreadyExists => write!(f, "email already registered"),
            Self::NotFound => write!(f, "user not found"),
            Self::InvalidCredentials => write!(f, "invalid email or password"),
            Self::HashError(e) => write!(f, "password hash error: {e}"),
        }
    }
}

// ── Store trait ───────────────────────────────────────────────────────────────

pub trait UserStore: Send + Sync {
    fn create(&self, email: &str, password: &str, name: Option<&str>, tenant_id: &str) -> Result<User, UserError>;
    fn find_by_email(&self, email: &str) -> Option<User>;
    fn find_by_id(&self, id: &str) -> Option<User>;
    fn verify_password(&self, email: &str, password: &str) -> Result<User, UserError>;
    fn update_name(&self, id: &str, name: &str) -> Result<User, UserError>;
    fn update_password(&self, id: &str, old_password: &str, new_password: &str) -> Result<(), UserError>;
    fn reset_password(&self, id: &str, new_password: &str) -> Result<(), UserError>;
    fn mark_email_verified(&self, id: &str) -> Result<(), UserError>;
    fn list_by_tenant(&self, tenant_id: &str) -> Vec<PublicUser>;
    fn delete_user(&self, id: &str) -> Result<(), UserError>;
}

// ── Memory store ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct MemoryUserStore {
    // keyed by user_id
    users: RwLock<HashMap<String, User>>,
}

impl UserStore for MemoryUserStore {
    fn create(&self, email: &str, password: &str, name: Option<&str>, tenant_id: &str) -> Result<User, UserError> {
        let mut map = self.users.write().unwrap();
        if map.values().any(|u| u.email == email) {
            return Err(UserError::EmailAlreadyExists);
        }
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        let user = User {
            id: uuid::Uuid::new_v4().to_string(),
            email: email.to_owned(),
            password_hash: hash,
            name: name.map(|s| s.to_owned()),
            tenant_id: tenant_id.to_owned(),
            created_at: unix_now(),
            email_verified: false,
        };
        map.insert(user.id.clone(), user.clone());
        Ok(user)
    }

    fn find_by_email(&self, email: &str) -> Option<User> {
        self.users.read().unwrap().values().find(|u| u.email == email).cloned()
    }

    fn find_by_id(&self, id: &str) -> Option<User> {
        self.users.read().unwrap().get(id).cloned()
    }

    fn verify_password(&self, email: &str, password: &str) -> Result<User, UserError> {
        let user = self.find_by_email(email).ok_or(UserError::InvalidCredentials)?;
        let valid = bcrypt::verify(password, &user.password_hash)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        if valid { Ok(user) } else { Err(UserError::InvalidCredentials) }
    }

    fn update_name(&self, id: &str, name: &str) -> Result<User, UserError> {
        let mut map = self.users.write().unwrap();
        let user = map.get_mut(id).ok_or(UserError::NotFound)?;
        user.name = Some(name.to_owned());
        Ok(user.clone())
    }

    fn update_password(&self, id: &str, old_password: &str, new_password: &str) -> Result<(), UserError> {
        let mut map = self.users.write().unwrap();
        let user = map.get_mut(id).ok_or(UserError::NotFound)?;
        let valid = bcrypt::verify(old_password, &user.password_hash)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        if !valid { return Err(UserError::InvalidCredentials); }
        user.password_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        Ok(())
    }

    fn reset_password(&self, id: &str, new_password: &str) -> Result<(), UserError> {
        let mut map = self.users.write().unwrap();
        let user = map.get_mut(id).ok_or(UserError::NotFound)?;
        user.password_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        Ok(())
    }

    fn mark_email_verified(&self, id: &str) -> Result<(), UserError> {
        let mut map = self.users.write().unwrap();
        let user = map.get_mut(id).ok_or(UserError::NotFound)?;
        user.email_verified = true;
        Ok(())
    }

    fn list_by_tenant(&self, tenant_id: &str) -> Vec<PublicUser> {
        self.users.read().unwrap().values()
            .filter(|u| u.tenant_id == tenant_id)
            .map(|u| PublicUser::from(u))
            .collect()
    }

    fn delete_user(&self, id: &str) -> Result<(), UserError> {
        let mut map = self.users.write().unwrap();
        if map.remove(id).is_some() { Ok(()) } else { Err(UserError::NotFound) }
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

pub struct PostgresUserStore {
    pool: sqlx::PgPool,
}

impl PostgresUserStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    email: String,
    password_hash: String,
    name: Option<String>,
    tenant_id: String,
    created_at: i64,
    #[sqlx(default)]
    email_verified: bool,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        Self {
            id: r.id,
            email: r.email,
            password_hash: r.password_hash,
            name: r.name,
            tenant_id: r.tenant_id,
            created_at: r.created_at,
            email_verified: r.email_verified,
        }
    }
}

impl UserStore for PostgresUserStore {
    fn create(&self, email: &str, password: &str, name: Option<&str>, tenant_id: &str) -> Result<User, UserError> {
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = unix_now();
        let pool = self.pool.clone();
        let email = email.to_owned();
        let name_owned = name.map(|s| s.to_owned());
        let tenant = tenant_id.to_owned();
        let id2 = id.clone();
        let email2 = email.clone();
        let hash2 = hash.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
                    "INSERT INTO af_users (id, email, password_hash, name, tenant_id, created_at, email_verified) \
                     VALUES ($1, $2, $3, $4, $5, $6, FALSE) \
                     ON CONFLICT (email) DO NOTHING \
                     RETURNING id, email, password_hash, name, tenant_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, email_verified"
                )
                .bind(&id2).bind(&email2).bind(&hash2).bind(&name_owned).bind(&tenant).bind(now)
                .fetch_optional(&pool).await
                .map_err(|e| UserError::HashError(e.to_string()))?;

                match row {
                    Some(r) => Ok(User::from(r)),
                    None => Err(UserError::EmailAlreadyExists),
                }
            })
        })
    }

    fn find_by_email(&self, email: &str) -> Option<User> {
        let pool = self.pool.clone();
        let email = email.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, UserRow>(
                    "SELECT id, email, password_hash, name, tenant_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, email_verified FROM af_users WHERE email = $1"
                ).bind(&email).fetch_optional(&pool).await.ok().flatten().map(User::from)
            })
        })
    }

    fn find_by_id(&self, id: &str) -> Option<User> {
        let pool = self.pool.clone();
        let id = id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, UserRow>(
                    "SELECT id, email, password_hash, name, tenant_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, email_verified FROM af_users WHERE id = $1"
                ).bind(&id).fetch_optional(&pool).await.ok().flatten().map(User::from)
            })
        })
    }

    fn verify_password(&self, email: &str, password: &str) -> Result<User, UserError> {
        let user = self.find_by_email(email).ok_or(UserError::InvalidCredentials)?;
        let valid = bcrypt::verify(password, &user.password_hash)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        if valid { Ok(user) } else { Err(UserError::InvalidCredentials) }
    }

    fn update_name(&self, id: &str, name: &str) -> Result<User, UserError> {
        let pool = self.pool.clone();
        let id = id.to_owned();
        let name = name.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
                    "UPDATE af_users SET name = $1 WHERE id = $2 RETURNING id, email, password_hash, name, tenant_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, email_verified"
                ).bind(&name).bind(&id).fetch_optional(&pool).await
                .map_err(|e| UserError::HashError(e.to_string()))?;
                row.map(User::from).ok_or(UserError::NotFound)
            })
        })
    }

    fn update_password(&self, id: &str, old_password: &str, new_password: &str) -> Result<(), UserError> {
        let user = self.find_by_id(id).ok_or(UserError::NotFound)?;
        let valid = bcrypt::verify(old_password, &user.password_hash)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        if !valid { return Err(UserError::InvalidCredentials); }
        self.reset_password(id, new_password)
    }

    fn reset_password(&self, id: &str, new_password: &str) -> Result<(), UserError> {
        let new_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .map_err(|e| UserError::HashError(e.to_string()))?;
        let pool = self.pool.clone();
        let id = id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query("UPDATE af_users SET password_hash = $1 WHERE id = $2")
                    .bind(&new_hash).bind(&id).execute(&pool).await
                    .map_err(|e| UserError::HashError(e.to_string()))
                    .map(|_| ())
            })
        })
    }

    fn mark_email_verified(&self, id: &str) -> Result<(), UserError> {
        let pool = self.pool.clone();
        let id = id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let result = sqlx::query("UPDATE af_users SET email_verified = TRUE WHERE id = $1")
                    .bind(&id).execute(&pool).await
                    .map_err(|e| UserError::HashError(e.to_string()))?;
                if result.rows_affected() == 0 { Err(UserError::NotFound) } else { Ok(()) }
            })
        })
    }

    fn list_by_tenant(&self, tenant_id: &str) -> Vec<PublicUser> {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, UserRow>(
                    "SELECT id, email, password_hash, name, tenant_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, email_verified FROM af_users WHERE tenant_id = $1 ORDER BY created_at"
                ).bind(&tenant_id).fetch_all(&pool).await.unwrap_or_default()
                .into_iter().map(|r| PublicUser::from(&User::from(r))).collect()
            })
        })
    }

    fn delete_user(&self, id: &str) -> Result<(), UserError> {
        let pool = self.pool.clone();
        let id = id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let result = sqlx::query("DELETE FROM af_users WHERE id = $1")
                    .bind(&id).execute(&pool).await
                    .map_err(|e| UserError::HashError(e.to_string()))?;
                if result.rows_affected() == 0 { Err(UserError::NotFound) } else { Ok(()) }
            })
        })
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

pub enum PlatformUserStore {
    Memory(Arc<MemoryUserStore>),
    Postgres(PostgresUserStore),
}

impl PlatformUserStore {
    pub fn memory() -> Self {
        Self::Memory(Arc::new(MemoryUserStore::default()))
    }
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self::Postgres(PostgresUserStore::new(pool))
    }
}

impl UserStore for PlatformUserStore {
    fn create(&self, email: &str, password: &str, name: Option<&str>, tenant_id: &str) -> Result<User, UserError> {
        match self { Self::Memory(s) => s.create(email, password, name, tenant_id), Self::Postgres(s) => s.create(email, password, name, tenant_id) }
    }
    fn find_by_email(&self, email: &str) -> Option<User> {
        match self { Self::Memory(s) => s.find_by_email(email), Self::Postgres(s) => s.find_by_email(email) }
    }
    fn find_by_id(&self, id: &str) -> Option<User> {
        match self { Self::Memory(s) => s.find_by_id(id), Self::Postgres(s) => s.find_by_id(id) }
    }
    fn verify_password(&self, email: &str, password: &str) -> Result<User, UserError> {
        match self { Self::Memory(s) => s.verify_password(email, password), Self::Postgres(s) => s.verify_password(email, password) }
    }
    fn update_name(&self, id: &str, name: &str) -> Result<User, UserError> {
        match self { Self::Memory(s) => s.update_name(id, name), Self::Postgres(s) => s.update_name(id, name) }
    }
    fn update_password(&self, id: &str, old: &str, new: &str) -> Result<(), UserError> {
        match self { Self::Memory(s) => s.update_password(id, old, new), Self::Postgres(s) => s.update_password(id, old, new) }
    }
    fn reset_password(&self, id: &str, new_password: &str) -> Result<(), UserError> {
        match self { Self::Memory(s) => s.reset_password(id, new_password), Self::Postgres(s) => s.reset_password(id, new_password) }
    }
    fn mark_email_verified(&self, id: &str) -> Result<(), UserError> {
        match self { Self::Memory(s) => s.mark_email_verified(id), Self::Postgres(s) => s.mark_email_verified(id) }
    }
    fn list_by_tenant(&self, tenant_id: &str) -> Vec<PublicUser> {
        match self { Self::Memory(s) => s.list_by_tenant(tenant_id), Self::Postgres(s) => s.list_by_tenant(tenant_id) }
    }
    fn delete_user(&self, id: &str) -> Result<(), UserError> {
        match self { Self::Memory(s) => s.delete_user(id), Self::Postgres(s) => s.delete_user(id) }
    }
}

impl Default for PlatformUserStore {
    fn default() -> Self { Self::memory() }
}
