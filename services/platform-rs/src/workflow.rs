// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use workflow_core::WorkflowGraph;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRecord {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub latest_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(default = "default_tenant_visibility")]
    pub visibility: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sla_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runs_per_hour: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_runs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
}

fn default_tenant_visibility() -> String { "tenant".to_string() }

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateWorkflowRequest {
    pub tenant_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UpdateWorkflowRequest {
    pub tenant_id: String,
    pub name: String,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub sla_seconds: Option<u64>,
    #[serde(default)]
    pub max_runs_per_hour: Option<u32>,
    #[serde(default)]
    pub max_concurrent_runs: Option<u32>,
    #[serde(default)]
    pub budget_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ArchiveWorkflowRequest {
    pub tenant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RestoreWorkflowRequest {
    pub tenant_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowVersionRecord {
    pub id: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub version: i32,
    pub status: String,
    pub graph: WorkflowGraph,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CreateWorkflowVersionRequest {
    pub tenant_id: String,
    pub graph: WorkflowGraph,
    pub status: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PublishWorkflowVersionRequest {
    pub tenant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    MissingTenant,
    MissingWorkspace,
    MissingProject,
    MissingWorkflow,
    MissingWorkflowVersion,
    MissingName,
    InvalidGraph,
    InvalidStatus,
    InvalidLimit,
    ArchivedWorkflow,
    DraftVersion,
    NoPublishedVersion,
    LockedWorkflow,
    NotFound,
    StoreUnavailable,
}

#[allow(async_fn_in_trait)]
pub trait WorkflowVersionStore: Clone + Send + Sync + 'static {
    async fn create_workflow(
        &self,
        request: CreateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn list_workflows(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowRecord>, WorkflowError>;

    async fn get_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn update_workflow(
        &self,
        workflow_id: &str,
        request: UpdateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn archive_workflow(
        &self,
        workflow_id: &str,
        request: ArchiveWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn restore_workflow(
        &self,
        workflow_id: &str,
        request: RestoreWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn get_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError>;

    async fn list_versions(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowVersionRecord>, WorkflowError>;

    async fn create_version(
        &self,
        workflow_id: &str,
        request: CreateWorkflowVersionRequest,
    ) -> Result<WorkflowVersionRecord, WorkflowError>;

    async fn publish_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError>;

    async fn pin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn unpin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn lock_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn unlock_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError>;

    async fn set_workflow_visibility(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        visibility: &str,
    ) -> Result<WorkflowRecord, WorkflowError>;
}

pub struct WorkflowService<S> {
    store: S,
}

impl<S> WorkflowService<S>
where
    S: WorkflowVersionStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn create_workflow(
        &self,
        request: CreateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if request.tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if request.workspace_id.is_empty() {
            return Err(WorkflowError::MissingWorkspace);
        }
        if request.project_id.is_empty() {
            return Err(WorkflowError::MissingProject);
        }
        if request.name.trim().is_empty() {
            return Err(WorkflowError::MissingName);
        }
        self.store.create_workflow(request).await
    }

    pub async fn list_workflows(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        status: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<WorkflowRecord>, WorkflowError> {
        if tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if status.is_some_and(|status| !matches!(status, "draft" | "published" | "archived")) {
            return Err(WorkflowError::InvalidStatus);
        }
        let limit = normalize_limit(limit)?;
        self.store
            .list_workflows(tenant_id, project_id, status, limit)
            .await
    }

    pub async fn get_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        self.store.get_workflow(tenant_id, workflow_id).await
    }

    pub async fn update_workflow(
        &self,
        workflow_id: &str,
        request: UpdateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        if request.tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if request.name.trim().is_empty() {
            return Err(WorkflowError::MissingName);
        }
        self.store.update_workflow(workflow_id, request).await
    }

    pub async fn archive_workflow(
        &self,
        workflow_id: &str,
        request: ArchiveWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        if request.tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        self.store.archive_workflow(workflow_id, request).await
    }

    pub async fn restore_workflow(
        &self,
        workflow_id: &str,
        request: RestoreWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        if request.tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        self.store.restore_workflow(workflow_id, request).await
    }

    pub async fn get_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        if tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if workflow_version_id.is_empty() {
            return Err(WorkflowError::MissingWorkflowVersion);
        }
        self.store.get_version(tenant_id, workflow_version_id).await
    }

    pub async fn list_versions(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        status: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<WorkflowVersionRecord>, WorkflowError> {
        if tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        if status.is_some_and(|status| !matches!(status, "draft" | "published")) {
            return Err(WorkflowError::InvalidStatus);
        }
        let limit = normalize_limit(limit)?;
        self.store
            .list_versions(tenant_id, workflow_id, status, limit)
            .await
    }

    pub async fn create_version(
        &self,
        workflow_id: &str,
        request: CreateWorkflowVersionRequest,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        if request.tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        let status = request.status.as_deref().unwrap_or("draft");
        if !matches!(status, "draft" | "published") {
            return Err(WorkflowError::InvalidStatus);
        }
        // Validate graph structure (cycles, etc.)
        if request.graph.validate().is_err() {
            return Err(WorkflowError::InvalidGraph);
        }
        self.store.create_version(workflow_id, request).await
    }

    pub async fn publish_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        if tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if workflow_version_id.is_empty() {
            return Err(WorkflowError::MissingWorkflowVersion);
        }
        self.store
            .publish_version(tenant_id, workflow_version_id)
            .await
    }

    pub async fn pin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        self.store.pin_workflow(tenant_id, workflow_id).await
    }

    pub async fn unpin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if tenant_id.is_empty() {
            return Err(WorkflowError::MissingTenant);
        }
        if workflow_id.is_empty() {
            return Err(WorkflowError::MissingWorkflow);
        }
        self.store.unpin_workflow(tenant_id, workflow_id).await
    }

    pub async fn lock_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if tenant_id.is_empty() { return Err(WorkflowError::MissingTenant); }
        if workflow_id.is_empty() { return Err(WorkflowError::MissingWorkflow); }
        self.store.lock_workflow(tenant_id, workflow_id).await
    }

    pub async fn unlock_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if tenant_id.is_empty() { return Err(WorkflowError::MissingTenant); }
        if workflow_id.is_empty() { return Err(WorkflowError::MissingWorkflow); }
        self.store.unlock_workflow(tenant_id, workflow_id).await
    }

    pub async fn set_workflow_visibility(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        visibility: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        if tenant_id.is_empty() { return Err(WorkflowError::MissingTenant); }
        if workflow_id.is_empty() { return Err(WorkflowError::MissingWorkflow); }
        if visibility != "tenant" && visibility != "private" {
            return Err(WorkflowError::InvalidStatus);
        }
        self.store.set_workflow_visibility(tenant_id, workflow_id, visibility).await
    }
}

fn normalize_limit(limit: Option<usize>) -> Result<usize, WorkflowError> {
    let limit = limit.unwrap_or(100);
    if !(1..=100).contains(&limit) {
        return Err(WorkflowError::InvalidLimit);
    }
    Ok(limit)
}

#[derive(Clone, Default)]
pub struct MemoryWorkflowVersionStore {
    workflows: Arc<RwLock<HashMap<String, WorkflowRecord>>>,
    versions: Arc<RwLock<HashMap<String, WorkflowVersionRecord>>>,
}

impl MemoryWorkflowVersionStore {
    pub fn with_dev_seed() -> Self {
        let store = Self::default();
        store.insert_workflow(WorkflowRecord {
            id: "workflow-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            project_id: "project-1".to_string(),
            name: "Dev Lead Workflow".to_string(),
            status: "published".to_string(),
            latest_version_id: Some("version-1".to_string()),
            tags: vec![],
            description: None,
            pinned: false,
            readme: None,
            updated_at: 0,
            created_at: 0,
            folder: None,
            locked: false,
            created_by: None,
            visibility: "tenant".to_string(),
            sla_seconds: None,
            max_runs_per_hour: None,
            max_concurrent_runs: None,
            budget_usd: None,
        });
        store.insert(WorkflowVersionRecord {
            id: "version-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            version: 1,
            status: "published".to_string(),
            graph: dev_graph("version-1"),
            message: None,
        });
        store
    }

    pub fn insert(&self, record: WorkflowVersionRecord) {
        let mut versions = self.versions.write().expect("lock workflow version store");
        versions.insert(key(&record.tenant_id, &record.id), record);
    }

    pub fn insert_workflow(&self, record: WorkflowRecord) {
        let mut workflows = self.workflows.write().expect("lock workflow store");
        workflows.insert(key(&record.tenant_id, &record.id), record);
    }
}

impl WorkflowVersionStore for MemoryWorkflowVersionStore {
    async fn create_workflow(
        &self,
        request: CreateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let id = next_id();
        let record = WorkflowRecord {
            id: id.clone(),
            tenant_id: request.tenant_id,
            workspace_id: request.workspace_id,
            project_id: request.project_id,
            name: request.name,
            status: "draft".to_string(),
            latest_version_id: None,
            tags: vec![],
            description: request.description,
            pinned: false,
            readme: None,
            updated_at: 0,
            created_at: 0,
            folder: request.folder,
            locked: false,
            created_by: request.created_by,
            visibility: "tenant".to_string(),
            sla_seconds: None,
            max_runs_per_hour: None,
            max_concurrent_runs: None,
            budget_usd: None,
        };
        let mut workflows = self
            .workflows
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        workflows.insert(key(&record.tenant_id, &id), record.clone());
        Ok(record)
    }

    async fn list_workflows(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowRecord>, WorkflowError> {
        let workflows = self
            .workflows
            .read()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let mut records = workflows
            .values()
            .filter(|record| {
                record.tenant_id == tenant_id
                    && project_id.is_none_or(|project_id| record.project_id == project_id)
                    && status.is_none_or(|status| record.status == status)
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| right.id.cmp(&left.id));
        records.truncate(limit);
        Ok(records)
    }

    async fn get_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let workflows = self
            .workflows
            .read()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        workflows
            .get(&key(tenant_id, workflow_id))
            .cloned()
            .ok_or(WorkflowError::NotFound)
    }

    async fn update_workflow(
        &self,
        workflow_id: &str,
        request: UpdateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self
            .workflows
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows
            .get_mut(&key(&request.tenant_id, workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        record.name = request.name;
        if let Some(tags) = request.tags {
            record.tags = tags;
        }
        if request.description.is_some() {
            record.description = request.description;
        }
        if request.readme.is_some() {
            record.readme = request.readme;
        }
        if request.folder.is_some() {
            record.folder = request.folder;
        }
        if request.sla_seconds.is_some() {
            record.sla_seconds = request.sla_seconds;
        }
        if request.max_runs_per_hour.is_some() {
            record.max_runs_per_hour = request.max_runs_per_hour;
        }
        if request.max_concurrent_runs.is_some() {
            record.max_concurrent_runs = request.max_concurrent_runs;
        }
        if request.budget_usd.is_some() {
            record.budget_usd = request.budget_usd;
        }
        Ok(record.clone())
    }

    async fn archive_workflow(
        &self,
        workflow_id: &str,
        request: ArchiveWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self
            .workflows
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows
            .get_mut(&key(&request.tenant_id, workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        record.status = "archived".to_string();
        Ok(record.clone())
    }

    async fn restore_workflow(
        &self,
        workflow_id: &str,
        request: RestoreWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self
            .workflows
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows
            .get_mut(&key(&request.tenant_id, workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        record.status = if record.latest_version_id.is_some() {
            "published".to_string()
        } else {
            "draft".to_string()
        };
        Ok(record.clone())
    }

    async fn get_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        let versions = self
            .versions
            .read()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        versions
            .get(&key(tenant_id, workflow_version_id))
            .cloned()
            .ok_or(WorkflowError::NotFound)
    }

    async fn list_versions(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowVersionRecord>, WorkflowError> {
        let versions = self
            .versions
            .read()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let mut records = versions
            .values()
            .filter(|record| {
                record.tenant_id == tenant_id
                    && record.workflow_id == workflow_id
                    && status.is_none_or(|status| record.status == status)
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| right.version.cmp(&left.version));
        records.truncate(limit);
        Ok(records)
    }

    async fn create_version(
        &self,
        workflow_id: &str,
        request: CreateWorkflowVersionRequest,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        let mut versions = self
            .versions
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let workflows = self
            .workflows
            .read()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let workflow = workflows
            .get(&key(&request.tenant_id, workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        if workflow.status == "archived" {
            return Err(WorkflowError::ArchivedWorkflow);
        }
        let version = versions
            .values()
            .filter(|record| {
                record.tenant_id == request.tenant_id && record.workflow_id == workflow_id
            })
            .map(|record| record.version)
            .max()
            .unwrap_or(0)
            + 1;
        let id = next_id();
        let mut graph = request.graph;
        graph.workflow_version_id = id.clone();
        graph.validate().map_err(|_| WorkflowError::InvalidGraph)?;

        let record = WorkflowVersionRecord {
            id: id.clone(),
            tenant_id: request.tenant_id,
            workflow_id: workflow_id.to_string(),
            version,
            status: request.status.unwrap_or_else(|| "draft".to_string()),
            graph,
            message: request.message,
        };
        versions.insert(key(&record.tenant_id, &id), record.clone());
        let tenant_id_for_update = record.tenant_id.clone();
        drop(versions);
        drop(workflows);
        // Update latest_version_id on the workflow so getWorkflow returns it
        if let Ok(mut wfs) = self.workflows.write() {
            if let Some(wf) = wfs.get_mut(&key(&tenant_id_for_update, workflow_id)) {
                wf.latest_version_id = Some(id);
            }
        }
        Ok(record)
    }

    async fn publish_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        let mut versions = self
            .versions
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = versions
            .get_mut(&key(tenant_id, workflow_version_id))
            .ok_or(WorkflowError::NotFound)?;
        let workflows = self
            .workflows
            .read()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let workflow = workflows
            .get(&key(tenant_id, &record.workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        if workflow.status == "archived" {
            return Err(WorkflowError::ArchivedWorkflow);
        }
        drop(workflows);

        record.status = "published".to_string();
        let record = record.clone();

        let mut workflows = self
            .workflows
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let workflow = workflows
            .get_mut(&key(tenant_id, &record.workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        workflow.status = "published".to_string();
        workflow.latest_version_id = Some(record.id.clone());

        Ok(record)
    }

    async fn pin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self
            .workflows
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows
            .get_mut(&key(tenant_id, workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        record.pinned = true;
        Ok(record.clone())
    }

    async fn unpin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self
            .workflows
            .write()
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows
            .get_mut(&key(tenant_id, workflow_id))
            .ok_or(WorkflowError::NotFound)?;
        record.pinned = false;
        Ok(record.clone())
    }

    async fn lock_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self.workflows.write().map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows.get_mut(&key(tenant_id, workflow_id)).ok_or(WorkflowError::NotFound)?;
        record.locked = true;
        Ok(record.clone())
    }

    async fn unlock_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self.workflows.write().map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows.get_mut(&key(tenant_id, workflow_id)).ok_or(WorkflowError::NotFound)?;
        record.locked = false;
        Ok(record.clone())
    }

    async fn set_workflow_visibility(&self, tenant_id: &str, workflow_id: &str, visibility: &str) -> Result<WorkflowRecord, WorkflowError> {
        let mut workflows = self.workflows.write().map_err(|_| WorkflowError::StoreUnavailable)?;
        let record = workflows.get_mut(&key(tenant_id, workflow_id)).ok_or(WorkflowError::NotFound)?;
        record.visibility = visibility.to_string();
        Ok(record.clone())
    }
}

#[derive(Clone)]
pub enum PlatformWorkflowVersionStore {
    Memory(MemoryWorkflowVersionStore),
    Postgres(PostgresWorkflowVersionStore),
}

impl PlatformWorkflowVersionStore {
    pub fn memory_with_dev_seed() -> Self {
        Self::Memory(MemoryWorkflowVersionStore::with_dev_seed())
    }

    pub fn postgres(store: PostgresWorkflowVersionStore) -> Self {
        Self::Postgres(store)
    }
}

impl WorkflowVersionStore for PlatformWorkflowVersionStore {
    async fn create_workflow(
        &self,
        request: CreateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.create_workflow(request).await,
            Self::Postgres(store) => store.create_workflow(request).await,
        }
    }

    async fn restore_workflow(
        &self,
        workflow_id: &str,
        request: RestoreWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.restore_workflow(workflow_id, request).await,
            Self::Postgres(store) => store.restore_workflow(workflow_id, request).await,
        }
    }

    async fn list_workflows(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowRecord>, WorkflowError> {
        match self {
            Self::Memory(store) => {
                store
                    .list_workflows(tenant_id, project_id, status, limit)
                    .await
            }
            Self::Postgres(store) => {
                store
                    .list_workflows(tenant_id, project_id, status, limit)
                    .await
            }
        }
    }

    async fn get_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.get_workflow(tenant_id, workflow_id).await,
            Self::Postgres(store) => store.get_workflow(tenant_id, workflow_id).await,
        }
    }

    async fn update_workflow(
        &self,
        workflow_id: &str,
        request: UpdateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.update_workflow(workflow_id, request).await,
            Self::Postgres(store) => store.update_workflow(workflow_id, request).await,
        }
    }

    async fn archive_workflow(
        &self,
        workflow_id: &str,
        request: ArchiveWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.archive_workflow(workflow_id, request).await,
            Self::Postgres(store) => store.archive_workflow(workflow_id, request).await,
        }
    }

    async fn get_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.get_version(tenant_id, workflow_version_id).await,
            Self::Postgres(store) => store.get_version(tenant_id, workflow_version_id).await,
        }
    }

    async fn create_version(
        &self,
        workflow_id: &str,
        request: CreateWorkflowVersionRequest,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.create_version(workflow_id, request).await,
            Self::Postgres(store) => store.create_version(workflow_id, request).await,
        }
    }

    async fn list_versions(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowVersionRecord>, WorkflowError> {
        match self {
            Self::Memory(store) => {
                store
                    .list_versions(tenant_id, workflow_id, status, limit)
                    .await
            }
            Self::Postgres(store) => {
                store
                    .list_versions(tenant_id, workflow_id, status, limit)
                    .await
            }
        }
    }

    async fn publish_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        match self {
            Self::Memory(store) => store.publish_version(tenant_id, workflow_version_id).await,
            Self::Postgres(store) => store.publish_version(tenant_id, workflow_version_id).await,
        }
    }

    async fn pin_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(s) => s.pin_workflow(tenant_id, workflow_id).await,
            Self::Postgres(s) => s.pin_workflow(tenant_id, workflow_id).await,
        }
    }

    async fn unpin_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(s) => s.unpin_workflow(tenant_id, workflow_id).await,
            Self::Postgres(s) => s.unpin_workflow(tenant_id, workflow_id).await,
        }
    }

    async fn lock_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(s) => s.lock_workflow(tenant_id, workflow_id).await,
            Self::Postgres(s) => s.lock_workflow(tenant_id, workflow_id).await,
        }
    }

    async fn unlock_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(s) => s.unlock_workflow(tenant_id, workflow_id).await,
            Self::Postgres(s) => s.unlock_workflow(tenant_id, workflow_id).await,
        }
    }

    async fn set_workflow_visibility(&self, tenant_id: &str, workflow_id: &str, visibility: &str) -> Result<WorkflowRecord, WorkflowError> {
        match self {
            Self::Memory(s) => s.set_workflow_visibility(tenant_id, workflow_id, visibility).await,
            Self::Postgres(s) => s.set_workflow_visibility(tenant_id, workflow_id, visibility).await,
        }
    }
}

#[derive(Clone)]
pub struct PostgresWorkflowVersionStore {
    pool: PgPool,
}

impl PostgresWorkflowVersionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl WorkflowVersionStore for PostgresWorkflowVersionStore {
    async fn create_workflow(
        &self,
        request: CreateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let id = next_id();
        sqlx::query(
            r#"
            INSERT INTO af_workflows (id, tenant_id, workspace_id, project_id, name, status, folder, created_by)
            VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7)
            "#,
        )
        .bind(&id)
        .bind(&request.tenant_id)
        .bind(&request.workspace_id)
        .bind(&request.project_id)
        .bind(&request.name)
        .bind(&request.folder)
        .bind(&request.created_by)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?;

        Ok(WorkflowRecord {
            id,
            tenant_id: request.tenant_id,
            workspace_id: request.workspace_id,
            project_id: request.project_id,
            name: request.name,
            status: "draft".to_string(),
            latest_version_id: None,
            tags: vec![],
            description: None,
            pinned: false,
            readme: None,
            updated_at: 0,
            created_at: 0,
            folder: request.folder,
            locked: false,
            created_by: request.created_by,
            visibility: "tenant".to_string(),
            sla_seconds: None,
            max_runs_per_hour: None,
            max_concurrent_runs: None,
            budget_usd: None,
        })
    }

    async fn list_workflows(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowRecord>, WorkflowError> {
        let rows = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"
            SELECT id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd
            FROM af_workflows
            WHERE tenant_id = $1
              AND ($2::text IS NULL OR project_id = $2)
              AND ($3::text IS NULL OR status = $3)
            ORDER BY pinned DESC, updated_at DESC, id DESC
            LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(status)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?;

        Ok(rows
            .into_iter()
            .map(PostgresWorkflowRow::into_record)
            .collect())
    }

    async fn get_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"
            SELECT id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd
            FROM af_workflows
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;

        Ok(row.into_record())
    }

    async fn restore_workflow(
        &self,
        workflow_id: &str,
        request: RestoreWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"
            UPDATE af_workflows
            SET status = CASE WHEN latest_version_id IS NULL THEN 'draft' ELSE 'published' END,
                updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd
            "#,
        )
        .bind(&request.tenant_id)
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;

        Ok(row.into_record())
    }

    async fn update_workflow(
        &self,
        workflow_id: &str,
        request: UpdateWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let new_tags: Option<Vec<String>> = request.tags;
        let new_description: Option<String> = request.description;
        let new_readme: Option<String> = request.readme;
        let new_folder: Option<String> = request.folder;
        let new_sla: Option<i64> = request.sla_seconds.map(|s| s as i64);
        let new_max_runs: Option<i32> = request.max_runs_per_hour.map(|v| v as i32);
        let new_max_concurrent: Option<i32> = request.max_concurrent_runs.map(|v| v as i32);
        let new_budget: Option<f64> = request.budget_usd;
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"
            UPDATE af_workflows
            SET name = $3,
                tags = COALESCE($4, tags),
                description = COALESCE($5, description),
                readme = COALESCE($6, readme),
                folder = COALESCE($7, folder),
                sla_seconds = COALESCE($8, sla_seconds),
                max_runs_per_hour = COALESCE($9, max_runs_per_hour),
                max_concurrent_runs = COALESCE($10, max_concurrent_runs),
                budget_usd = COALESCE($11, budget_usd),
                updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd
            "#,
        )
        .bind(&request.tenant_id)
        .bind(workflow_id)
        .bind(&request.name)
        .bind(new_tags)
        .bind(new_description)
        .bind(new_readme)
        .bind(new_folder)
        .bind(new_sla)
        .bind(new_max_runs)
        .bind(new_max_concurrent)
        .bind(new_budget)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;

        Ok(row.into_record())
    }

    async fn archive_workflow(
        &self,
        workflow_id: &str,
        request: ArchiveWorkflowRequest,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"
            UPDATE af_workflows
            SET status = 'archived', updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd
            "#,
        )
        .bind(&request.tenant_id)
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;

        Ok(row.into_record())
    }

    async fn get_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowVersionRow>(
            r#"
            SELECT id, tenant_id, workflow_id, version, status, graph_json, message
            FROM af_workflow_versions
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(workflow_version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;

        row.try_into_record()
    }

    async fn create_version(
        &self,
        workflow_id: &str,
        request: CreateWorkflowVersionRequest,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        let workflow_status = sqlx::query_scalar::<_, String>(
            r#"SELECT status FROM af_workflows WHERE tenant_id = $1 AND id = $2"#,
        )
        .bind(&request.tenant_id)
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;

        if workflow_status == "archived" {
            return Err(WorkflowError::ArchivedWorkflow);
        }

        let version = sqlx::query_scalar::<_, Option<i32>>(
            r#"SELECT MAX(version) FROM af_workflow_versions WHERE tenant_id = $1 AND workflow_id = $2"#,
        )
        .bind(&request.tenant_id)
        .bind(workflow_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .unwrap_or(0) + 1;

        let id = next_id();
        let mut graph = request.graph;
        graph.workflow_version_id = id.clone();
        graph.validate().map_err(|_| WorkflowError::InvalidGraph)?;
        let graph_json = serde_json::to_value(&graph).map_err(|_| WorkflowError::InvalidGraph)?;
        let status = request.status.unwrap_or_else(|| "draft".to_string());

        sqlx::query(
            r#"
            INSERT INTO af_workflow_versions (id, tenant_id, workflow_id, version, graph_json, status, message, published_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, CASE WHEN $6 = 'published' THEN now() ELSE NULL END)
            "#,
        )
        .bind(&id)
        .bind(&request.tenant_id)
        .bind(workflow_id)
        .bind(version)
        .bind(sqlx::types::Json(graph_json))
        .bind(&status)
        .bind(&request.message)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?;

        if status == "published" {
            sqlx::query(
                r#"
                UPDATE af_workflows
                SET latest_version_id = $3, status = 'published', updated_at = now()
                WHERE tenant_id = $1 AND id = $2
                "#,
            )
            .bind(&request.tenant_id)
            .bind(workflow_id)
            .bind(&id)
            .execute(&self.pool)
            .await
            .map_err(|_| WorkflowError::StoreUnavailable)?;
        }

        self.get_version(&request.tenant_id, &id).await
    }

    async fn list_versions(
        &self,
        tenant_id: &str,
        workflow_id: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkflowVersionRecord>, WorkflowError> {
        let rows = sqlx::query_as::<_, PostgresWorkflowVersionRow>(
            r#"
            SELECT id, tenant_id, workflow_id, version, status, graph_json, message
            FROM af_workflow_versions
            WHERE tenant_id = $1 AND workflow_id = $2
              AND ($3::text IS NULL OR status = $3)
            ORDER BY version DESC
            LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .bind(status)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?;

        rows.into_iter()
            .map(PostgresWorkflowVersionRow::try_into_record)
            .collect()
    }

    async fn publish_version(
        &self,
        tenant_id: &str,
        workflow_version_id: &str,
    ) -> Result<WorkflowVersionRecord, WorkflowError> {
        let version = self.get_version(tenant_id, workflow_version_id).await?;
        let workflow = self.get_workflow(tenant_id, &version.workflow_id).await?;
        if workflow.status == "archived" {
            return Err(WorkflowError::ArchivedWorkflow);
        }

        sqlx::query(
            r#"
            UPDATE af_workflow_versions
            SET status = 'published', published_at = COALESCE(published_at, now())
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(workflow_version_id)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?;

        sqlx::query(
            r#"
            UPDATE af_workflows
            SET latest_version_id = $3, status = 'published', updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(&version.workflow_id)
        .bind(workflow_version_id)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?;

        self.get_version(tenant_id, workflow_version_id).await
    }

    async fn pin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"
            UPDATE af_workflows SET pinned = TRUE, updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd
            "#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;
        Ok(row.into_record())
    }

    async fn unpin_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"
            UPDATE af_workflows SET pinned = FALSE, updated_at = now()
            WHERE tenant_id = $1 AND id = $2
            RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd
            "#,
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;
        Ok(row.into_record())
    }

    async fn lock_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"UPDATE af_workflows SET locked = TRUE, updated_at = now()
               WHERE tenant_id = $1 AND id = $2
               RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd"#,
        )
        .bind(tenant_id).bind(workflow_id)
        .fetch_optional(&self.pool).await.map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;
        Ok(row.into_record())
    }

    async fn unlock_workflow(&self, tenant_id: &str, workflow_id: &str) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"UPDATE af_workflows SET locked = FALSE, updated_at = now()
               WHERE tenant_id = $1 AND id = $2
               RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd"#,
        )
        .bind(tenant_id).bind(workflow_id)
        .fetch_optional(&self.pool).await.map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;
        Ok(row.into_record())
    }

    async fn set_workflow_visibility(&self, tenant_id: &str, workflow_id: &str, visibility: &str) -> Result<WorkflowRecord, WorkflowError> {
        let row = sqlx::query_as::<_, PostgresWorkflowRow>(
            r#"UPDATE af_workflows SET visibility = $3, updated_at = now()
               WHERE tenant_id = $1 AND id = $2
               RETURNING id, tenant_id, workspace_id, project_id, name, status, latest_version_id, tags, description, pinned, readme, EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, folder, locked, created_by, visibility, sla_seconds, max_runs_per_hour, max_concurrent_runs, budget_usd"#,
        )
        .bind(tenant_id).bind(workflow_id).bind(visibility)
        .fetch_optional(&self.pool).await.map_err(|_| WorkflowError::StoreUnavailable)?
        .ok_or(WorkflowError::NotFound)?;
        Ok(row.into_record())
    }
}

#[derive(sqlx::FromRow)]
struct PostgresWorkflowRow {
    id: String,
    tenant_id: String,
    workspace_id: String,
    project_id: String,
    name: String,
    status: String,
    latest_version_id: Option<String>,
    #[sqlx(default)]
    tags: Vec<String>,
    #[sqlx(default)]
    description: Option<String>,
    #[sqlx(default)]
    pinned: bool,
    #[sqlx(default)]
    readme: Option<String>,
    #[sqlx(default)]
    updated_at: i64,
    #[sqlx(default)]
    created_at: i64,
    #[sqlx(default)]
    folder: Option<String>,
    #[sqlx(default)]
    locked: bool,
    #[sqlx(default)]
    created_by: Option<String>,
    #[sqlx(default)]
    visibility: String,
    #[sqlx(default)]
    sla_seconds: Option<i64>,
    #[sqlx(default)]
    max_runs_per_hour: Option<i32>,
    #[sqlx(default)]
    max_concurrent_runs: Option<i32>,
    #[sqlx(default)]
    budget_usd: Option<f64>,
}

impl PostgresWorkflowRow {
    fn into_record(self) -> WorkflowRecord {
        WorkflowRecord {
            id: self.id,
            tenant_id: self.tenant_id,
            workspace_id: self.workspace_id,
            project_id: self.project_id,
            name: self.name,
            status: self.status,
            latest_version_id: self.latest_version_id,
            tags: self.tags,
            description: self.description,
            pinned: self.pinned,
            readme: self.readme,
            updated_at: self.updated_at,
            created_at: self.created_at,
            folder: self.folder,
            locked: self.locked,
            created_by: self.created_by,
            visibility: if self.visibility.is_empty() { "tenant".to_string() } else { self.visibility },
            sla_seconds: self.sla_seconds.map(|s| s as u64),
            max_runs_per_hour: self.max_runs_per_hour.map(|v| v as u32),
            max_concurrent_runs: self.max_concurrent_runs.map(|v| v as u32),
            budget_usd: self.budget_usd,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PostgresWorkflowVersionRow {
    id: String,
    tenant_id: String,
    workflow_id: String,
    version: i32,
    status: String,
    graph_json: serde_json::Value,
    #[sqlx(default)]
    message: Option<String>,
}

impl PostgresWorkflowVersionRow {
    fn try_into_record(self) -> Result<WorkflowVersionRecord, WorkflowError> {
        let graph: WorkflowGraph =
            serde_json::from_value(self.graph_json).map_err(|_| WorkflowError::InvalidGraph)?;

        Ok(WorkflowVersionRecord {
            id: self.id,
            tenant_id: self.tenant_id,
            workflow_id: self.workflow_id,
            version: self.version,
            status: self.status,
            graph,
            message: self.message,
        })
    }
}

fn key(tenant_id: &str, workflow_version_id: &str) -> String {
    format!("{tenant_id}:{workflow_version_id}")
}

fn next_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn dev_graph(workflow_version_id: &str) -> WorkflowGraph {
    WorkflowGraph {
        workflow_version_id: workflow_version_id.to_string(),
        nodes: vec![
            workflow_core::Node {
                id: "trigger".to_string(),
                node_type: workflow_core::NodeType::Trigger,
                config: None,
            },
            workflow_core::Node {
                id: "agent".to_string(),
                node_type: workflow_core::NodeType::Agent,
                config: None,
            },
        ],
        edges: vec![workflow_core::Edge {
            source: "trigger".to_string(),
            target: "agent".to_string(),
            condition_label: None,
        }],
        input_schema: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn gets_seeded_workflow_version() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());

        let record = service.get_version("tenant-1", "version-1").await.unwrap();

        assert_eq!(record.workflow_id, "workflow-1");
        assert_eq!(record.graph.nodes.len(), 2);
    }

    #[tokio::test]
    async fn creates_and_lists_workflows() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        let request = CreateWorkflowRequest {
            tenant_id: "tenant-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            project_id: "project-1".to_string(),
            name: "New Workflow".to_string(),
            description: None,
            folder: None,
            created_by: None,
        };

        let record = service.create_workflow(request).await.unwrap();
        let workflows = service
            .list_workflows("tenant-1", Some("project-1"), None, None)
            .await
            .unwrap();

        assert_eq!(record.name, "New Workflow");
        assert_eq!(record.status, "draft");
        assert_eq!(workflows.len(), 2);
        assert!(workflows.iter().any(|workflow| workflow.id == record.id));
    }

    #[tokio::test]
    async fn lists_workflows_by_status() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        let record = service
            .create_workflow(CreateWorkflowRequest {
                tenant_id: "tenant-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                project_id: "project-1".to_string(),
                name: "Draft Workflow".to_string(),
                description: None,
                folder: None,
                created_by: None,
            })
            .await
            .unwrap();

        let drafts = service
            .list_workflows("tenant-1", Some("project-1"), Some("draft"), None)
            .await
            .unwrap();
        let published = service
            .list_workflows("tenant-1", Some("project-1"), Some("published"), None)
            .await
            .unwrap();
        let invalid = service
            .list_workflows("tenant-1", Some("project-1"), Some("deleted"), None)
            .await
            .unwrap_err();
        let limited = service
            .list_workflows("tenant-1", Some("project-1"), None, Some(1))
            .await
            .unwrap();
        let invalid_limit = service
            .list_workflows("tenant-1", Some("project-1"), None, Some(0))
            .await
            .unwrap_err();

        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].id, record.id);
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].id, "workflow-1");
        assert_eq!(invalid, WorkflowError::InvalidStatus);
        assert_eq!(limited.len(), 1);
        assert_eq!(invalid_limit, WorkflowError::InvalidLimit);
    }

    #[tokio::test]
    async fn gets_seeded_workflow() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());

        let record = service
            .get_workflow("tenant-1", "workflow-1")
            .await
            .unwrap();

        assert_eq!(record.name, "Dev Lead Workflow");
        assert_eq!(record.status, "published");
        assert_eq!(record.latest_version_id, Some("version-1".to_string()));
    }

    #[tokio::test]
    async fn updates_workflow_name() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());

        let record = service
            .update_workflow(
                "workflow-1",
                UpdateWorkflowRequest {
                    tenant_id: "tenant-1".to_string(),
                    name: "Renamed Workflow".to_string(),
                    tags: None,
                    description: None,
                    readme: None,
                    folder: None,
                    sla_seconds: None,
            max_runs_per_hour: None,
            max_concurrent_runs: None,
            budget_usd: None,
                },
            )
            .await
            .unwrap();
        let loaded = service
            .get_workflow("tenant-1", "workflow-1")
            .await
            .unwrap();

        assert_eq!(record.name, "Renamed Workflow");
        assert_eq!(loaded.name, "Renamed Workflow");
        assert_eq!(loaded.status, "published");
        assert_eq!(loaded.latest_version_id, Some("version-1".to_string()));
    }

    #[tokio::test]
    async fn archives_workflow() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());

        let record = service
            .archive_workflow(
                "workflow-1",
                ArchiveWorkflowRequest {
                    tenant_id: "tenant-1".to_string(),
                },
            )
            .await
            .unwrap();
        let loaded = service
            .get_workflow("tenant-1", "workflow-1")
            .await
            .unwrap();

        assert_eq!(record.status, "archived");
        assert_eq!(loaded.status, "archived");
        assert_eq!(loaded.latest_version_id, Some("version-1".to_string()));
    }

    #[tokio::test]
    async fn restores_archived_workflow() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        service
            .archive_workflow(
                "workflow-1",
                ArchiveWorkflowRequest {
                    tenant_id: "tenant-1".to_string(),
                },
            )
            .await
            .unwrap();

        let record = service
            .restore_workflow(
                "workflow-1",
                RestoreWorkflowRequest {
                    tenant_id: "tenant-1".to_string(),
                },
            )
            .await
            .unwrap();
        let loaded = service
            .get_workflow("tenant-1", "workflow-1")
            .await
            .unwrap();

        assert_eq!(record.status, "published");
        assert_eq!(loaded.status, "published");
        assert_eq!(loaded.latest_version_id, Some("version-1".to_string()));
    }

    #[tokio::test]
    async fn rejects_version_create_for_archived_workflow() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        service
            .archive_workflow(
                "workflow-1",
                ArchiveWorkflowRequest {
                    tenant_id: "tenant-1".to_string(),
                },
            )
            .await
            .unwrap();

        let err = service
            .create_version(
                "workflow-1",
                CreateWorkflowVersionRequest {
                    tenant_id: "tenant-1".to_string(),
                    graph: dev_graph("client-supplied-id"),
                    status: None,
                    message: None,
                },
            )
            .await
            .unwrap_err();

        assert_eq!(err, WorkflowError::ArchivedWorkflow);
    }

    #[tokio::test]
    async fn rejects_publish_for_archived_workflow() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        service
            .archive_workflow(
                "workflow-1",
                ArchiveWorkflowRequest {
                    tenant_id: "tenant-1".to_string(),
                },
            )
            .await
            .unwrap();

        let err = service
            .publish_version("tenant-1", "version-1")
            .await
            .unwrap_err();

        assert_eq!(err, WorkflowError::ArchivedWorkflow);
    }

    #[tokio::test]
    async fn rejects_missing_tenant() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());

        let err = service.get_version("", "version-1").await.unwrap_err();

        assert_eq!(err, WorkflowError::MissingTenant);
    }

    #[tokio::test]
    async fn creates_workflow_version() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        let request = CreateWorkflowVersionRequest {
            tenant_id: "tenant-1".to_string(),
            graph: dev_graph("client-supplied-id"),
            status: None,
            message: None,
        };

        let record = service.create_version("workflow-1", request).await.unwrap();
        let loaded = service.get_version("tenant-1", &record.id).await.unwrap();

        assert_eq!(record.workflow_id, "workflow-1");
        assert_eq!(record.version, 2);
        assert_eq!(record.status, "draft");
        assert_eq!(record.graph.workflow_version_id, record.id);
        assert_eq!(loaded.id, record.id);
    }

    #[tokio::test]
    async fn lists_workflow_versions() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        let request = CreateWorkflowVersionRequest {
            tenant_id: "tenant-1".to_string(),
            graph: dev_graph("client-supplied-id"),
            status: None,
            message: None,
        };
        let record = service.create_version("workflow-1", request).await.unwrap();

        let versions = service
            .list_versions("tenant-1", "workflow-1", None, None)
            .await
            .unwrap();
        let drafts = service
            .list_versions("tenant-1", "workflow-1", Some("draft"), None)
            .await
            .unwrap();
        let published = service
            .list_versions("tenant-1", "workflow-1", Some("published"), None)
            .await
            .unwrap();
        let invalid = service
            .list_versions("tenant-1", "workflow-1", Some("archived"), None)
            .await
            .unwrap_err();
        let limited = service
            .list_versions("tenant-1", "workflow-1", None, Some(1))
            .await
            .unwrap();
        let invalid_limit = service
            .list_versions("tenant-1", "workflow-1", None, Some(101))
            .await
            .unwrap_err();

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].id, record.id);
        assert_eq!(versions[0].version, 2);
        assert_eq!(versions[1].version, 1);
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].id, record.id);
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].id, "version-1");
        assert_eq!(invalid, WorkflowError::InvalidStatus);
        assert_eq!(limited.len(), 1);
        assert_eq!(invalid_limit, WorkflowError::InvalidLimit);
    }

    #[tokio::test]
    async fn publishes_workflow_version() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        let workflow = service
            .create_workflow(CreateWorkflowRequest {
                tenant_id: "tenant-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                project_id: "project-1".to_string(),
                name: "New Workflow".to_string(),
                description: None,
                folder: None,
                created_by: None,
            })
            .await
            .unwrap();
        let version = service
            .create_version(
                &workflow.id,
                CreateWorkflowVersionRequest {
                    tenant_id: "tenant-1".to_string(),
                    graph: dev_graph("client-supplied-id"),
                    status: None,
                    message: None,
                },
            )
            .await
            .unwrap();

        let published = service
            .publish_version("tenant-1", &version.id)
            .await
            .unwrap();
        let workflows = service
            .list_workflows("tenant-1", Some("project-1"), None, None)
            .await
            .unwrap();
        let workflow = workflows
            .into_iter()
            .find(|record| record.id == workflow.id)
            .unwrap();

        assert_eq!(published.status, "published");
        assert_eq!(workflow.status, "published");
        assert_eq!(workflow.latest_version_id, Some(version.id));
    }

    #[tokio::test]
    async fn updates_workflow_readme() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());

        let record = service
            .update_workflow(
                "workflow-1",
                UpdateWorkflowRequest {
                    tenant_id: "tenant-1".to_string(),
                    name: "Dev Lead Workflow".to_string(),
                    tags: None,
                    description: None,
                    readme: Some("# My Workflow\n\nDocumentation goes here.".to_string()),
                    folder: None,
                    sla_seconds: None,
            max_runs_per_hour: None,
            max_concurrent_runs: None,
            budget_usd: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(record.readme.as_deref(), Some("# My Workflow\n\nDocumentation goes here."));

        let loaded = service.get_workflow("tenant-1", "workflow-1").await.unwrap();
        assert_eq!(loaded.readme.as_deref(), Some("# My Workflow\n\nDocumentation goes here."));
    }

    #[tokio::test]
    async fn readme_is_none_by_default() {
        let service = WorkflowService::new(MemoryWorkflowVersionStore::with_dev_seed());
        let record = service.get_workflow("tenant-1", "workflow-1").await.unwrap();
        assert!(record.readme.is_none());
    }
}
