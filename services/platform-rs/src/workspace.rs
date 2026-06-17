// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
}

fn next_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── In-memory ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MemoryWorkspaceStore {
    workspaces: Arc<RwLock<HashMap<String, WorkspaceRecord>>>,
    projects: Arc<RwLock<HashMap<String, ProjectRecord>>>,
}

impl MemoryWorkspaceStore {
    pub async fn create_workspace(
        &self,
        tenant_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> WorkspaceRecord {
        let record = WorkspaceRecord {
            id: next_id(),
            tenant_id: tenant_id.to_string(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
        };
        self.workspaces
            .write()
            .unwrap()
            .insert(record.id.clone(), record.clone());
        record
    }

    pub async fn list_workspaces(&self, tenant_id: &str) -> Vec<WorkspaceRecord> {
        let mut out: Vec<_> = self
            .workspaces
            .read()
            .unwrap()
            .values()
            .filter(|w| w.tenant_id == tenant_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub async fn get_workspace(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Option<WorkspaceRecord> {
        self.workspaces
            .read()
            .unwrap()
            .get(workspace_id)
            .filter(|w| w.tenant_id == tenant_id)
            .cloned()
    }

    pub async fn delete_workspace(&self, tenant_id: &str, workspace_id: &str) -> bool {
        let mut map = self.workspaces.write().unwrap();
        if map
            .get(workspace_id)
            .is_some_and(|w| w.tenant_id == tenant_id)
        {
            map.remove(workspace_id);
            true
        } else {
            false
        }
    }

    pub async fn create_project(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Option<ProjectRecord> {
        self.get_workspace(tenant_id, workspace_id).await.as_ref()?;
        let record = ProjectRecord {
            id: next_id(),
            tenant_id: tenant_id.to_string(),
            workspace_id: workspace_id.to_string(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
        };
        self.projects
            .write()
            .unwrap()
            .insert(record.id.clone(), record.clone());
        Some(record)
    }

    pub async fn list_projects(&self, tenant_id: &str, workspace_id: &str) -> Vec<ProjectRecord> {
        let mut out: Vec<_> = self
            .projects
            .read()
            .unwrap()
            .values()
            .filter(|p| p.tenant_id == tenant_id && p.workspace_id == workspace_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub async fn delete_project(&self, tenant_id: &str, project_id: &str) -> bool {
        let mut map = self.projects.write().unwrap();
        if map
            .get(project_id)
            .is_some_and(|p| p.tenant_id == tenant_id)
        {
            map.remove(project_id);
            true
        } else {
            false
        }
    }
}

// ── Postgres ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PostgresWorkspaceStore {
    pool: PgPool,
}

impl PostgresWorkspaceStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_workspace(
        &self,
        tenant_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> WorkspaceRecord {
        let id = next_id();
        let now = unix_now();
        let _ = sqlx::query(
            r#"INSERT INTO af_workspaces (id, tenant_id, name, description, created_at)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(name)
        .bind(description)
        .bind(now)
        .execute(&self.pool)
        .await;
        WorkspaceRecord {
            id,
            tenant_id: tenant_id.to_string(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
        }
    }

    pub async fn list_workspaces(&self, tenant_id: &str) -> Vec<WorkspaceRecord> {
        sqlx::query_as::<_, (String, String, String, Option<String>)>(
            r#"SELECT id, tenant_id, name, description FROM af_workspaces
               WHERE tenant_id = $1 ORDER BY name ASC"#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, tenant_id, name, description)| WorkspaceRecord {
            id,
            tenant_id,
            name,
            description,
        })
        .collect()
    }

    pub async fn get_workspace(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Option<WorkspaceRecord> {
        sqlx::query_as::<_, (String, String, String, Option<String>)>(
            r#"SELECT id, tenant_id, name, description FROM af_workspaces
               WHERE tenant_id = $1 AND id = $2"#,
        )
        .bind(tenant_id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|(id, tenant_id, name, description)| WorkspaceRecord {
            id,
            tenant_id,
            name,
            description,
        })
    }

    pub async fn delete_workspace(&self, tenant_id: &str, workspace_id: &str) -> bool {
        sqlx::query(r#"DELETE FROM af_workspaces WHERE tenant_id = $1 AND id = $2"#)
            .bind(tenant_id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .unwrap_or(false)
    }

    pub async fn create_project(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Option<ProjectRecord> {
        // Verify workspace exists for this tenant
        self.get_workspace(tenant_id, workspace_id).await.as_ref()?;
        let id = next_id();
        let now = unix_now();
        let ok = sqlx::query(
            r#"INSERT INTO af_projects (id, tenant_id, workspace_id, name, description, created_at)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(workspace_id)
        .bind(name)
        .bind(description)
        .bind(now)
        .execute(&self.pool)
        .await
        .is_ok();
        if ok {
            Some(ProjectRecord {
                id,
                tenant_id: tenant_id.to_string(),
                workspace_id: workspace_id.to_string(),
                name: name.to_string(),
                description: description.map(|s| s.to_string()),
            })
        } else {
            None
        }
    }

    pub async fn list_projects(&self, tenant_id: &str, workspace_id: &str) -> Vec<ProjectRecord> {
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            r#"SELECT id, tenant_id, workspace_id, name, description FROM af_projects
               WHERE tenant_id = $1 AND workspace_id = $2 ORDER BY name ASC"#,
        )
        .bind(tenant_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(
            |(id, tenant_id, workspace_id, name, description)| ProjectRecord {
                id,
                tenant_id,
                workspace_id,
                name,
                description,
            },
        )
        .collect()
    }

    pub async fn delete_project(&self, tenant_id: &str, project_id: &str) -> bool {
        sqlx::query(r#"DELETE FROM af_projects WHERE tenant_id = $1 AND id = $2"#)
            .bind(tenant_id)
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .unwrap_or(false)
    }
}

// ── Platform enum ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformWorkspaceStore {
    Memory(MemoryWorkspaceStore),
    Postgres(PostgresWorkspaceStore),
}

impl Default for PlatformWorkspaceStore {
    fn default() -> Self {
        Self::Memory(MemoryWorkspaceStore::default())
    }
}

impl PlatformWorkspaceStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryWorkspaceStore::default())
    }

    pub fn postgres(store: PostgresWorkspaceStore) -> Self {
        Self::Postgres(store)
    }

    pub async fn create_workspace(
        &self,
        tenant_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> WorkspaceRecord {
        match self {
            Self::Memory(s) => s.create_workspace(tenant_id, name, description).await,
            Self::Postgres(s) => s.create_workspace(tenant_id, name, description).await,
        }
    }

    pub async fn list_workspaces(&self, tenant_id: &str) -> Vec<WorkspaceRecord> {
        match self {
            Self::Memory(s) => s.list_workspaces(tenant_id).await,
            Self::Postgres(s) => s.list_workspaces(tenant_id).await,
        }
    }

    pub async fn get_workspace(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Option<WorkspaceRecord> {
        match self {
            Self::Memory(s) => s.get_workspace(tenant_id, workspace_id).await,
            Self::Postgres(s) => s.get_workspace(tenant_id, workspace_id).await,
        }
    }

    pub async fn delete_workspace(&self, tenant_id: &str, workspace_id: &str) -> bool {
        match self {
            Self::Memory(s) => s.delete_workspace(tenant_id, workspace_id).await,
            Self::Postgres(s) => s.delete_workspace(tenant_id, workspace_id).await,
        }
    }

    pub async fn create_project(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Option<ProjectRecord> {
        match self {
            Self::Memory(s) => {
                s.create_project(tenant_id, workspace_id, name, description)
                    .await
            }
            Self::Postgres(s) => {
                s.create_project(tenant_id, workspace_id, name, description)
                    .await
            }
        }
    }

    pub async fn list_projects(&self, tenant_id: &str, workspace_id: &str) -> Vec<ProjectRecord> {
        match self {
            Self::Memory(s) => s.list_projects(tenant_id, workspace_id).await,
            Self::Postgres(s) => s.list_projects(tenant_id, workspace_id).await,
        }
    }

    pub async fn delete_project(&self, tenant_id: &str, project_id: &str) -> bool {
        match self {
            Self::Memory(s) => s.delete_project(tenant_id, project_id).await,
            Self::Postgres(s) => s.delete_project(tenant_id, project_id).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_list_workspaces() {
        let store = MemoryWorkspaceStore::default();
        store
            .create_workspace("tenant-1", "Engineering", None)
            .await;
        store
            .create_workspace("tenant-1", "Marketing", Some("Marketing team"))
            .await;
        store.create_workspace("tenant-2", "Other", None).await;

        let list = store.list_workspaces("tenant-1").await;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Engineering");
        assert_eq!(list[1].name, "Marketing");
        assert_eq!(list[1].description.as_deref(), Some("Marketing team"));

        assert_eq!(store.list_workspaces("tenant-2").await.len(), 1);
    }

    #[tokio::test]
    async fn create_and_list_projects() {
        let store = MemoryWorkspaceStore::default();
        let ws = store
            .create_workspace("tenant-1", "Engineering", None)
            .await;
        let p = store
            .create_project("tenant-1", &ws.id, "Backend", None)
            .await
            .unwrap();
        store
            .create_project("tenant-1", &ws.id, "Frontend", None)
            .await
            .unwrap();

        let projects = store.list_projects("tenant-1", &ws.id).await;
        assert_eq!(projects.len(), 2);

        assert!(store
            .create_project("tenant-1", "nonexistent", "X", None)
            .await
            .is_none());

        assert!(store.delete_project("tenant-1", &p.id).await);
        assert_eq!(store.list_projects("tenant-1", &ws.id).await.len(), 1);
    }

    #[tokio::test]
    async fn delete_workspace() {
        let store = MemoryWorkspaceStore::default();
        let ws = store.create_workspace("tenant-1", "Temp", None).await;
        assert_eq!(store.list_workspaces("tenant-1").await.len(), 1);
        assert!(store.delete_workspace("tenant-1", &ws.id).await);
        assert!(store.list_workspaces("tenant-1").await.is_empty());
        assert!(!store.delete_workspace("tenant-1", &ws.id).await);
    }
}
