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
pub struct OrgRecord {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgMember {
    pub org_id: String,
    pub user_id: String,
    pub role: String,
    pub joined_at: i64,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum OrgError {
    NotFound,
    AlreadyMember,
    NotMember,
    Forbidden,
    StoreError(String),
}

impl std::fmt::Display for OrgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "organization not found"),
            Self::AlreadyMember => write!(f, "user is already a member"),
            Self::NotMember => write!(f, "user is not a member"),
            Self::Forbidden => write!(f, "not authorized"),
            Self::StoreError(e) => write!(f, "store error: {e}"),
        }
    }
}

// ── Store trait ───────────────────────────────────────────────────────────────

pub trait OrgStore: Send + Sync {
    fn create(&self, id: &str, name: &str, owner_id: &str) -> Result<OrgRecord, OrgError>;
    fn find_by_id(&self, id: &str) -> Option<OrgRecord>;
    fn list_by_owner(&self, owner_id: &str) -> Vec<OrgRecord>;
    fn list_for_user(&self, user_id: &str) -> Vec<OrgRecord>;
    fn delete(&self, id: &str) -> bool;
    fn add_member(&self, org_id: &str, user_id: &str, role: &str) -> Result<OrgMember, OrgError>;
    fn remove_member(&self, org_id: &str, user_id: &str) -> Result<(), OrgError>;
    fn list_members(&self, org_id: &str) -> Vec<OrgMember>;
    fn get_member(&self, org_id: &str, user_id: &str) -> Option<OrgMember>;
}

// ── Memory store ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct MemoryOrgStore {
    orgs: RwLock<HashMap<String, OrgRecord>>,
    members: RwLock<Vec<OrgMember>>,
}

impl OrgStore for MemoryOrgStore {
    fn create(&self, id: &str, name: &str, owner_id: &str) -> Result<OrgRecord, OrgError> {
        let org = OrgRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            owner_id: owner_id.to_owned(),
            created_at: unix_now(),
        };
        self.orgs
            .write()
            .unwrap()
            .insert(id.to_owned(), org.clone());
        // Auto-add owner as admin member
        let _ = self.add_member(id, owner_id, "admin");
        Ok(org)
    }

    fn find_by_id(&self, id: &str) -> Option<OrgRecord> {
        self.orgs.read().unwrap().get(id).cloned()
    }

    fn list_by_owner(&self, owner_id: &str) -> Vec<OrgRecord> {
        self.orgs
            .read()
            .unwrap()
            .values()
            .filter(|o| o.owner_id == owner_id)
            .cloned()
            .collect()
    }

    fn list_for_user(&self, user_id: &str) -> Vec<OrgRecord> {
        let member_org_ids: Vec<String> = self
            .members
            .read()
            .unwrap()
            .iter()
            .filter(|m| m.user_id == user_id)
            .map(|m| m.org_id.clone())
            .collect();
        let orgs = self.orgs.read().unwrap();
        member_org_ids
            .iter()
            .filter_map(|id| orgs.get(id))
            .cloned()
            .collect()
    }

    fn delete(&self, id: &str) -> bool {
        let removed = self.orgs.write().unwrap().remove(id).is_some();
        if removed {
            self.members.write().unwrap().retain(|m| m.org_id != id);
        }
        removed
    }

    fn add_member(&self, org_id: &str, user_id: &str, role: &str) -> Result<OrgMember, OrgError> {
        let mut members = self.members.write().unwrap();
        if members
            .iter()
            .any(|m| m.org_id == org_id && m.user_id == user_id)
        {
            return Err(OrgError::AlreadyMember);
        }
        let member = OrgMember {
            org_id: org_id.to_owned(),
            user_id: user_id.to_owned(),
            role: role.to_owned(),
            joined_at: unix_now(),
        };
        members.push(member.clone());
        Ok(member)
    }

    fn remove_member(&self, org_id: &str, user_id: &str) -> Result<(), OrgError> {
        let mut members = self.members.write().unwrap();
        let before = members.len();
        members.retain(|m| !(m.org_id == org_id && m.user_id == user_id));
        if members.len() < before {
            Ok(())
        } else {
            Err(OrgError::NotMember)
        }
    }

    fn list_members(&self, org_id: &str) -> Vec<OrgMember> {
        self.members
            .read()
            .unwrap()
            .iter()
            .filter(|m| m.org_id == org_id)
            .cloned()
            .collect()
    }

    fn get_member(&self, org_id: &str, user_id: &str) -> Option<OrgMember> {
        self.members
            .read()
            .unwrap()
            .iter()
            .find(|m| m.org_id == org_id && m.user_id == user_id)
            .cloned()
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

pub struct PostgresOrgStore {
    pool: sqlx::PgPool,
}

impl PostgresOrgStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct OrgRow {
    id: String,
    name: String,
    owner_id: String,
    created_at: i64,
}

impl From<OrgRow> for OrgRecord {
    fn from(r: OrgRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            owner_id: r.owner_id,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct MemberRow {
    org_id: String,
    user_id: String,
    role: String,
    joined_at: i64,
}

impl From<MemberRow> for OrgMember {
    fn from(r: MemberRow) -> Self {
        Self {
            org_id: r.org_id,
            user_id: r.user_id,
            role: r.role,
            joined_at: r.joined_at,
        }
    }
}

impl OrgStore for PostgresOrgStore {
    fn create(&self, id: &str, name: &str, owner_id: &str) -> Result<OrgRecord, OrgError> {
        let pool = self.pool.clone();
        let id = id.to_owned();
        let name = name.to_owned();
        let owner_id = owner_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, OrgRow>(
                    "INSERT INTO af_orgs (id, name, owner_id, created_at) \
                     VALUES ($1, $2, $3, EXTRACT(EPOCH FROM now())::BIGINT) \
                     RETURNING id, name, owner_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at"
                )
                .bind(&id).bind(&name).bind(&owner_id)
                .fetch_one(&pool).await
                .map(OrgRecord::from)
                .map_err(|e| OrgError::StoreError(e.to_string()))
                .inspect(|_org| {
                    // fire-and-forget owner member insert
                    let pool2 = pool.clone();
                    let id2 = id.clone();
                    let owner2 = owner_id.clone();
                    tokio::spawn(async move {
                        let _ = sqlx::query(
                            "INSERT INTO af_org_members (org_id, user_id, role) VALUES ($1, $2, 'admin') ON CONFLICT DO NOTHING"
                        ).bind(&id2).bind(&owner2).execute(&pool2).await;
                    });
                })
            })
        })
    }

    fn find_by_id(&self, id: &str) -> Option<OrgRecord> {
        let pool = self.pool.clone();
        let id = id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, OrgRow>(
                    "SELECT id, name, owner_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at FROM af_orgs WHERE id = $1"
                ).bind(&id).fetch_optional(&pool).await.ok().flatten().map(OrgRecord::from)
            })
        })
    }

    fn list_by_owner(&self, owner_id: &str) -> Vec<OrgRecord> {
        let pool = self.pool.clone();
        let owner_id = owner_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, OrgRow>(
                    "SELECT id, name, owner_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at FROM af_orgs WHERE owner_id = $1 ORDER BY created_at"
                ).bind(&owner_id).fetch_all(&pool).await.unwrap_or_default().into_iter().map(OrgRecord::from).collect()
            })
        })
    }

    fn list_for_user(&self, user_id: &str) -> Vec<OrgRecord> {
        let pool = self.pool.clone();
        let user_id = user_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, OrgRow>(
                    "SELECT o.id, o.name, o.owner_id, EXTRACT(EPOCH FROM o.created_at)::BIGINT AS created_at \
                     FROM af_orgs o JOIN af_org_members m ON o.id = m.org_id WHERE m.user_id = $1 ORDER BY o.created_at"
                ).bind(&user_id).fetch_all(&pool).await.unwrap_or_default().into_iter().map(OrgRecord::from).collect()
            })
        })
    }

    fn delete(&self, id: &str) -> bool {
        let pool = self.pool.clone();
        let id = id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query("DELETE FROM af_orgs WHERE id = $1")
                    .bind(&id)
                    .execute(&pool)
                    .await
                    .map(|r| r.rows_affected() > 0)
                    .unwrap_or(false)
            })
        })
    }

    fn add_member(&self, org_id: &str, user_id: &str, role: &str) -> Result<OrgMember, OrgError> {
        let pool = self.pool.clone();
        let org_id = org_id.to_owned();
        let user_id = user_id.to_owned();
        let role = role.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, MemberRow>(
                    "INSERT INTO af_org_members (org_id, user_id, role) VALUES ($1, $2, $3) \
                     ON CONFLICT (org_id, user_id) DO NOTHING \
                     RETURNING org_id, user_id, role, EXTRACT(EPOCH FROM joined_at)::BIGINT AS joined_at"
                )
                .bind(&org_id).bind(&user_id).bind(&role)
                .fetch_optional(&pool).await
                .map_err(|e| OrgError::StoreError(e.to_string()))?
                .map(OrgMember::from)
                .ok_or(OrgError::AlreadyMember)
            })
        })
    }

    fn remove_member(&self, org_id: &str, user_id: &str) -> Result<(), OrgError> {
        let pool = self.pool.clone();
        let org_id = org_id.to_owned();
        let user_id = user_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query("DELETE FROM af_org_members WHERE org_id = $1 AND user_id = $2")
                    .bind(&org_id)
                    .bind(&user_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| OrgError::StoreError(e.to_string()))
                    .and_then(|r| {
                        if r.rows_affected() > 0 {
                            Ok(())
                        } else {
                            Err(OrgError::NotMember)
                        }
                    })
            })
        })
    }

    fn list_members(&self, org_id: &str) -> Vec<OrgMember> {
        let pool = self.pool.clone();
        let org_id = org_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, MemberRow>(
                    "SELECT org_id, user_id, role, EXTRACT(EPOCH FROM joined_at)::BIGINT AS joined_at \
                     FROM af_org_members WHERE org_id = $1 ORDER BY joined_at"
                ).bind(&org_id).fetch_all(&pool).await.unwrap_or_default().into_iter().map(OrgMember::from).collect()
            })
        })
    }

    fn get_member(&self, org_id: &str, user_id: &str) -> Option<OrgMember> {
        let pool = self.pool.clone();
        let org_id = org_id.to_owned();
        let user_id = user_id.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, MemberRow>(
                    "SELECT org_id, user_id, role, EXTRACT(EPOCH FROM joined_at)::BIGINT AS joined_at \
                     FROM af_org_members WHERE org_id = $1 AND user_id = $2"
                ).bind(&org_id).bind(&user_id).fetch_optional(&pool).await.ok().flatten().map(OrgMember::from)
            })
        })
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

pub enum PlatformOrgStore {
    Memory(Arc<MemoryOrgStore>),
    Postgres(PostgresOrgStore),
}

impl PlatformOrgStore {
    pub fn memory() -> Self {
        Self::Memory(Arc::new(MemoryOrgStore::default()))
    }
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self::Postgres(PostgresOrgStore::new(pool))
    }
}

impl Default for PlatformOrgStore {
    fn default() -> Self {
        Self::memory()
    }
}

impl OrgStore for PlatformOrgStore {
    fn create(&self, id: &str, name: &str, owner_id: &str) -> Result<OrgRecord, OrgError> {
        match self {
            Self::Memory(s) => s.create(id, name, owner_id),
            Self::Postgres(s) => s.create(id, name, owner_id),
        }
    }
    fn find_by_id(&self, id: &str) -> Option<OrgRecord> {
        match self {
            Self::Memory(s) => s.find_by_id(id),
            Self::Postgres(s) => s.find_by_id(id),
        }
    }
    fn list_by_owner(&self, owner_id: &str) -> Vec<OrgRecord> {
        match self {
            Self::Memory(s) => s.list_by_owner(owner_id),
            Self::Postgres(s) => s.list_by_owner(owner_id),
        }
    }
    fn list_for_user(&self, user_id: &str) -> Vec<OrgRecord> {
        match self {
            Self::Memory(s) => s.list_for_user(user_id),
            Self::Postgres(s) => s.list_for_user(user_id),
        }
    }
    fn delete(&self, id: &str) -> bool {
        match self {
            Self::Memory(s) => s.delete(id),
            Self::Postgres(s) => s.delete(id),
        }
    }
    fn add_member(&self, org_id: &str, user_id: &str, role: &str) -> Result<OrgMember, OrgError> {
        match self {
            Self::Memory(s) => s.add_member(org_id, user_id, role),
            Self::Postgres(s) => s.add_member(org_id, user_id, role),
        }
    }
    fn remove_member(&self, org_id: &str, user_id: &str) -> Result<(), OrgError> {
        match self {
            Self::Memory(s) => s.remove_member(org_id, user_id),
            Self::Postgres(s) => s.remove_member(org_id, user_id),
        }
    }
    fn list_members(&self, org_id: &str) -> Vec<OrgMember> {
        match self {
            Self::Memory(s) => s.list_members(org_id),
            Self::Postgres(s) => s.list_members(org_id),
        }
    }
    fn get_member(&self, org_id: &str, user_id: &str) -> Option<OrgMember> {
        match self {
            Self::Memory(s) => s.get_member(org_id, user_id),
            Self::Postgres(s) => s.get_member(org_id, user_id),
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MemoryOrgStore {
        MemoryOrgStore::default()
    }

    #[test]
    fn create_org_auto_adds_owner() {
        let s = store();
        let org = s.create("org-1", "Acme", "user-1").unwrap();
        assert_eq!(org.name, "Acme");
        let members = s.list_members("org-1");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, "user-1");
        assert_eq!(members[0].role, "admin");
    }

    #[test]
    fn add_and_remove_member() {
        let s = store();
        s.create("org-1", "Acme", "owner").unwrap();
        s.add_member("org-1", "member-1", "editor").unwrap();
        assert_eq!(s.list_members("org-1").len(), 2);

        s.remove_member("org-1", "member-1").unwrap();
        assert_eq!(s.list_members("org-1").len(), 1);
    }

    #[test]
    fn duplicate_member_returns_error() {
        let s = store();
        s.create("org-1", "Acme", "owner").unwrap();
        s.add_member("org-1", "user-2", "editor").unwrap();
        assert!(matches!(
            s.add_member("org-1", "user-2", "editor"),
            Err(OrgError::AlreadyMember)
        ));
    }

    #[test]
    fn list_orgs_for_user() {
        let s = store();
        s.create("org-a", "Alpha", "user-1").unwrap();
        s.create("org-b", "Beta", "user-2").unwrap();
        s.add_member("org-b", "user-1", "viewer").unwrap();
        let orgs = s.list_for_user("user-1");
        assert_eq!(orgs.len(), 2);
    }

    #[test]
    fn delete_org_removes_members() {
        let s = store();
        s.create("org-1", "Acme", "owner").unwrap();
        s.add_member("org-1", "user-2", "editor").unwrap();
        s.delete("org-1");
        assert!(s.list_members("org-1").is_empty());
    }
}
