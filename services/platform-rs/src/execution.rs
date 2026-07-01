// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::token_usage::TokenUsageStore;
use execution_core::{
    run_workflow_with_progress, ExecutionReport, NodeProgressCallback, NodeReport,
};
use execution_core::{ExecutionStatus, NodeStatus};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use sqlx::types::Json;
use trigix_executor::approval::ApprovalGate;
use workflow_core::{Node, WorkflowGraph};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartExecutionRequest {
    pub tenant_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub graph: WorkflowGraph,
    pub input_json: String,
    pub label: Option<String>,
    pub callback_url: Option<String>,
    pub trigger_type: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub retried_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub graph: WorkflowGraph,
    pub input_json: String,
    pub status: ExecutionStatus,
    pub node_results: Vec<NodeExecutionRecord>,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Final output: the last succeeded non-trigger node's output_json.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_type: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub starred: bool,
    /// Total nodes in this execution's graph (set at start).
    #[serde(default, skip_serializing_if = "crate::execution::is_zero_u32")]
    pub node_count: u32,
    /// Number of nodes that have completed (any terminal status).
    #[serde(default, skip_serializing_if = "crate::execution::is_zero_u32")]
    pub completed_node_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retried_from: Option<String>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExecutionRecord {
    pub node_id: String,
    pub node_type: String,
    pub status: NodeStatus,
    pub output_json: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub started_at_ms: u64,
    #[serde(default)]
    pub retry_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionSummary {
    pub id: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub status: ExecutionStatus,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_type: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub starred: bool,
    #[serde(default, skip_serializing_if = "crate::execution::is_zero_u32")]
    pub node_count: u32,
    #[serde(default, skip_serializing_if = "crate::execution::is_zero_u32")]
    pub completed_node_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retried_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    MissingTenant,
    MissingWorkflow,
    MissingWorkflowVersion,
    WorkflowVersionMismatch,
    InvalidGraph,
    InvalidInput,
    InputSchemaViolation(String),
    NotFound,
    StoreUnavailable,
    ExecutorUnavailable,
}

pub trait ExecutionStore: Clone + Send + Sync + 'static {
    fn create(
        &self,
        request: StartExecutionRequest,
    ) -> impl std::future::Future<Output = Result<ExecutionRecord, ExecutionError>> + Send;

    fn get(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> impl std::future::Future<Output = Result<ExecutionRecord, ExecutionError>> + Send;

    fn list(
        &self,
        tenant_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<ExecutionSummary>, ExecutionError>> + Send;

    fn complete(
        &self,
        tenant_id: &str,
        execution_id: &str,
        report: ExecutionReport,
    ) -> impl std::future::Future<Output = Result<ExecutionRecord, ExecutionError>> + Send;

    fn fail(
        &self,
        tenant_id: &str,
        execution_id: &str,
        error: String,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    fn cancel(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    /// Write a single node's result into a running execution (for live progress updates via SSE).
    /// Called after each node completes during execution; `complete()` will overwrite with final data.
    fn append_node_result(
        &self,
        tenant_id: &str,
        execution_id: &str,
        node_result: NodeExecutionRecord,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    /// Return all running/waiting-approval executions started before `now_unix - older_than_secs`.
    fn list_stale_running(
        &self,
        older_than_secs: u64,
        now_unix: u64,
    ) -> impl std::future::Future<Output = Result<Vec<ExecutionSummary>, ExecutionError>> + Send;

    fn delete(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    fn set_label(
        &self,
        tenant_id: &str,
        execution_id: &str,
        label: Option<String>,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    fn set_note(
        &self,
        tenant_id: &str,
        execution_id: &str,
        note: Option<String>,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    fn set_starred(
        &self,
        tenant_id: &str,
        execution_id: &str,
        starred: bool,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    /// Count live (running + waiting_approval) executions for a tenant.
    fn count_running_by_tenant(
        &self,
        tenant_id: &str,
    ) -> impl std::future::Future<Output = Result<u64, ExecutionError>> + Send;

    /// Count all live executions across all tenants (for global concurrency limit).
    fn count_all_running(
        &self,
    ) -> impl std::future::Future<Output = Result<u64, ExecutionError>> + Send;

    /// Count live executions for a specific workflow (for per-workflow concurrency limit).
    fn count_running_by_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> impl std::future::Future<Output = Result<u64, ExecutionError>> + Send;
}

pub trait ExecutorClient: Clone + Send + Sync + 'static {
    fn start(
        &self,
        record: &ExecutionRecord,
    ) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;
}

pub struct ExecutionService<S, E> {
    store: S,
    executor: E,
}

impl<S, E> ExecutionService<S, E>
where
    S: ExecutionStore,
    E: ExecutorClient,
{
    pub fn new(store: S, executor: E) -> Self {
        Self { store, executor }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub async fn start(
        &self,
        request: StartExecutionRequest,
    ) -> Result<ExecutionRecord, ExecutionError> {
        validate_start_request(&request)?;
        let record = self.store.create(request).await?;

        let executor = self.executor.clone();
        let store = self.store.clone();
        let record_clone = record.clone();
        tokio::spawn(async move {
            if executor.start(&record_clone).await.is_err() {
                let _ = store
                    .fail(
                        &record_clone.tenant_id,
                        &record_clone.id,
                        "Execution failed".to_string(),
                    )
                    .await;
            }
        });

        Ok(record)
    }

    pub async fn get(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<ExecutionRecord, ExecutionError> {
        self.store.get(tenant_id, execution_id).await
    }

    pub async fn cancel(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        self.store.cancel(tenant_id, execution_id).await
    }

    pub async fn list(&self, tenant_id: &str) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store.list(tenant_id).await
    }

    /// Cancel all running or waiting-approval executions for the tenant. Returns the count cancelled.
    pub async fn cancel_all_running(&self, tenant_id: &str) -> Result<usize, ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        let summaries = self.store.list(tenant_id).await?;
        let live: Vec<_> = summaries
            .into_iter()
            .filter(|s| {
                s.status == ExecutionStatus::Running || s.status == ExecutionStatus::WaitingApproval
            })
            .collect();
        let count = live.len();
        for summary in live {
            let _ = self.store.cancel(tenant_id, &summary.id).await;
        }
        Ok(count)
    }

    /// Cancel all running executions whose `started_at` is older than `now - timeout_secs`.
    /// Called by the background timeout guard; scans across all tenants.
    pub async fn cancel_stale_running(
        &self,
        timeout_secs: u64,
        now: u64,
    ) -> Result<usize, ExecutionError> {
        let stale = self.store.list_stale_running(timeout_secs, now).await?;
        let count = stale.len();
        for summary in stale {
            let _ = self.store.cancel(&summary.tenant_id, &summary.id).await;
        }
        Ok(count)
    }

    pub async fn delete(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store.delete(tenant_id, execution_id).await
    }

    pub async fn set_label(
        &self,
        tenant_id: &str,
        execution_id: &str,
        label: Option<String>,
    ) -> Result<(), ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store.set_label(tenant_id, execution_id, label).await
    }

    pub async fn set_note(
        &self,
        tenant_id: &str,
        execution_id: &str,
        note: Option<String>,
    ) -> Result<(), ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store.set_note(tenant_id, execution_id, note).await
    }

    pub async fn set_starred(
        &self,
        tenant_id: &str,
        execution_id: &str,
        starred: bool,
    ) -> Result<(), ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store
            .set_starred(tenant_id, execution_id, starred)
            .await
    }

    pub async fn count_running_by_tenant(&self, tenant_id: &str) -> Result<u64, ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store.count_running_by_tenant(tenant_id).await
    }

    pub async fn count_all_running(&self) -> Result<u64, ExecutionError> {
        self.store.count_all_running().await
    }

    pub async fn count_running_by_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<u64, ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store
            .count_running_by_workflow(tenant_id, workflow_id)
            .await
    }
}

#[derive(Clone, Default)]
pub struct MemoryExecutionStore {
    records: Arc<RwLock<HashMap<String, ExecutionRecord>>>,
}

impl ExecutionStore for MemoryExecutionStore {
    async fn create(
        &self,
        request: StartExecutionRequest,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let id = next_id();
        let node_count = request.graph.nodes.len() as u32;
        let record = ExecutionRecord {
            id: id.clone(),
            tenant_id: request.tenant_id,
            workflow_id: request.workflow_id,
            workflow_version_id: request.workflow_version_id,
            graph: request.graph,
            input_json: request.input_json,
            status: ExecutionStatus::Running,
            node_results: Vec::new(),
            started_at: unix_now(),
            finished_at: None,
            label: request.label,
            output_json: None,
            callback_url: request.callback_url,
            trigger_type: request.trigger_type,
            dry_run: request.dry_run,
            note: None,
            starred: false,
            node_count,
            completed_node_count: 0,
            retried_from: request.retried_from,
        };

        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        records.insert(key(&record.tenant_id, &id), record.clone());
        Ok(record)
    }

    async fn get(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let records = self
            .records
            .read()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        records
            .get(&key(tenant_id, execution_id))
            .cloned()
            .ok_or(ExecutionError::NotFound)
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        let records = self
            .records
            .read()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let mut summaries = records
            .values()
            .filter(|record| record.tenant_id == tenant_id)
            .map(execution_summary)
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then(right.id.cmp(&left.id))
        });
        Ok(summaries)
    }

    async fn complete(
        &self,
        tenant_id: &str,
        execution_id: &str,
        report: ExecutionReport,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let record = records
            .get_mut(&key(tenant_id, execution_id))
            .ok_or(ExecutionError::NotFound)?;
        // Don't overwrite a terminal status set externally (e.g. cancelled).
        if record.status == ExecutionStatus::Cancelled {
            return Ok(record.clone());
        }
        let node_results = build_node_execution_records(&record.graph, &report);
        record.output_json = extract_workflow_output(&node_results);
        record.status = report.status;
        record.node_results = node_results;
        record.finished_at = Some(unix_now());
        Ok(record.clone())
    }

    async fn fail(
        &self,
        tenant_id: &str,
        execution_id: &str,
        error: String,
    ) -> Result<(), ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let record = records
            .get_mut(&key(tenant_id, execution_id))
            .ok_or(ExecutionError::NotFound)?;
        if record.status == ExecutionStatus::Cancelled {
            return Ok(());
        }
        record.status = ExecutionStatus::Failed;
        record.finished_at = Some(unix_now());
        record.node_results = vec![NodeExecutionRecord {
            node_id: "_executor".to_string(),
            node_type: "_executor".to_string(),
            status: NodeStatus::Failed,
            output_json: None,
            error: Some(error),
            duration_ms: 0,
            started_at_ms: 0,
            retry_count: 0,
        }];
        Ok(())
    }

    async fn cancel(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let record = records
            .get_mut(&key(tenant_id, execution_id))
            .ok_or(ExecutionError::NotFound)?;
        // Only cancel if still in a non-terminal state.
        if matches!(
            record.status,
            ExecutionStatus::Running | ExecutionStatus::WaitingApproval
        ) {
            record.status = ExecutionStatus::Cancelled;
            record.finished_at = Some(unix_now());
        }
        Ok(())
    }

    async fn append_node_result(
        &self,
        tenant_id: &str,
        execution_id: &str,
        node_result: NodeExecutionRecord,
    ) -> Result<(), ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        if let Some(record) = records.get_mut(&key(tenant_id, execution_id)) {
            // Replace existing entry for this node_id or append
            if let Some(pos) = record
                .node_results
                .iter()
                .position(|nr| nr.node_id == node_result.node_id)
            {
                record.node_results[pos] = node_result;
            } else {
                record.node_results.push(node_result);
            }
        }
        Ok(())
    }

    async fn list_stale_running(
        &self,
        older_than_secs: u64,
        now_unix: u64,
    ) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        let cutoff = now_unix.saturating_sub(older_than_secs);
        let records = self
            .records
            .read()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let summaries = records
            .values()
            .filter(|r| {
                matches!(
                    r.status,
                    ExecutionStatus::Running | ExecutionStatus::WaitingApproval
                ) && r.started_at < cutoff
            })
            .map(execution_summary)
            .collect();
        Ok(summaries)
    }

    async fn delete(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        if records.remove(&key(tenant_id, execution_id)).is_some() {
            Ok(())
        } else {
            Err(ExecutionError::NotFound)
        }
    }

    async fn set_label(
        &self,
        tenant_id: &str,
        execution_id: &str,
        label: Option<String>,
    ) -> Result<(), ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let rec = records
            .get_mut(&key(tenant_id, execution_id))
            .ok_or(ExecutionError::NotFound)?;
        rec.label = label;
        Ok(())
    }

    async fn set_note(
        &self,
        tenant_id: &str,
        execution_id: &str,
        note: Option<String>,
    ) -> Result<(), ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let rec = records
            .get_mut(&key(tenant_id, execution_id))
            .ok_or(ExecutionError::NotFound)?;
        rec.note = note;
        Ok(())
    }

    async fn set_starred(
        &self,
        tenant_id: &str,
        execution_id: &str,
        starred: bool,
    ) -> Result<(), ExecutionError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let rec = records
            .get_mut(&key(tenant_id, execution_id))
            .ok_or(ExecutionError::NotFound)?;
        rec.starred = starred;
        Ok(())
    }

    async fn count_running_by_tenant(&self, tenant_id: &str) -> Result<u64, ExecutionError> {
        let records = self
            .records
            .read()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let count = records
            .values()
            .filter(|r| {
                r.tenant_id == tenant_id
                    && matches!(
                        r.status,
                        ExecutionStatus::Running | ExecutionStatus::WaitingApproval
                    )
            })
            .count() as u64;
        Ok(count)
    }

    async fn count_all_running(&self) -> Result<u64, ExecutionError> {
        let records = self
            .records
            .read()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let count = records
            .values()
            .filter(|r| {
                matches!(
                    r.status,
                    ExecutionStatus::Running | ExecutionStatus::WaitingApproval
                )
            })
            .count() as u64;
        Ok(count)
    }

    async fn count_running_by_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<u64, ExecutionError> {
        let records = self
            .records
            .read()
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        let count = records
            .values()
            .filter(|r| {
                r.tenant_id == tenant_id
                    && r.workflow_id == workflow_id
                    && matches!(
                        r.status,
                        ExecutionStatus::Running | ExecutionStatus::WaitingApproval
                    )
            })
            .count() as u64;
        Ok(count)
    }
}

#[derive(Clone)]
pub enum PlatformExecutionStore {
    Memory(MemoryExecutionStore),
    Postgres(PostgresExecutionStore),
}

impl PlatformExecutionStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryExecutionStore::default())
    }

    pub fn postgres(store: PostgresExecutionStore) -> Self {
        Self::Postgres(store)
    }
}

impl ExecutionStore for PlatformExecutionStore {
    async fn create(
        &self,
        request: StartExecutionRequest,
    ) -> Result<ExecutionRecord, ExecutionError> {
        match self {
            Self::Memory(store) => store.create(request).await,
            Self::Postgres(store) => store.create(request).await,
        }
    }

    async fn get(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<ExecutionRecord, ExecutionError> {
        match self {
            Self::Memory(store) => store.get(tenant_id, execution_id).await,
            Self::Postgres(store) => store.get(tenant_id, execution_id).await,
        }
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        match self {
            Self::Memory(store) => store.list(tenant_id).await,
            Self::Postgres(store) => store.list(tenant_id).await,
        }
    }

    async fn complete(
        &self,
        tenant_id: &str,
        execution_id: &str,
        report: ExecutionReport,
    ) -> Result<ExecutionRecord, ExecutionError> {
        match self {
            Self::Memory(store) => store.complete(tenant_id, execution_id, report).await,
            Self::Postgres(store) => store.complete(tenant_id, execution_id, report).await,
        }
    }

    async fn fail(
        &self,
        tenant_id: &str,
        execution_id: &str,
        error: String,
    ) -> Result<(), ExecutionError> {
        match self {
            Self::Memory(store) => store.fail(tenant_id, execution_id, error).await,
            Self::Postgres(store) => store.fail(tenant_id, execution_id, error).await,
        }
    }

    async fn cancel(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        match self {
            Self::Memory(store) => store.cancel(tenant_id, execution_id).await,
            Self::Postgres(store) => store.cancel(tenant_id, execution_id).await,
        }
    }

    async fn append_node_result(
        &self,
        tenant_id: &str,
        execution_id: &str,
        node_result: NodeExecutionRecord,
    ) -> Result<(), ExecutionError> {
        match self {
            Self::Memory(store) => {
                store
                    .append_node_result(tenant_id, execution_id, node_result)
                    .await
            }
            Self::Postgres(store) => {
                store
                    .append_node_result(tenant_id, execution_id, node_result)
                    .await
            }
        }
    }

    async fn list_stale_running(
        &self,
        older_than_secs: u64,
        now_unix: u64,
    ) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        match self {
            Self::Memory(store) => store.list_stale_running(older_than_secs, now_unix).await,
            Self::Postgres(store) => store.list_stale_running(older_than_secs, now_unix).await,
        }
    }

    async fn delete(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        match self {
            Self::Memory(store) => store.delete(tenant_id, execution_id).await,
            Self::Postgres(store) => store.delete(tenant_id, execution_id).await,
        }
    }

    async fn set_label(
        &self,
        tenant_id: &str,
        execution_id: &str,
        label: Option<String>,
    ) -> Result<(), ExecutionError> {
        match self {
            Self::Memory(store) => store.set_label(tenant_id, execution_id, label).await,
            Self::Postgres(store) => store.set_label(tenant_id, execution_id, label).await,
        }
    }

    async fn set_note(
        &self,
        tenant_id: &str,
        execution_id: &str,
        note: Option<String>,
    ) -> Result<(), ExecutionError> {
        match self {
            Self::Memory(store) => store.set_note(tenant_id, execution_id, note).await,
            Self::Postgres(store) => store.set_note(tenant_id, execution_id, note).await,
        }
    }

    async fn set_starred(
        &self,
        tenant_id: &str,
        execution_id: &str,
        starred: bool,
    ) -> Result<(), ExecutionError> {
        match self {
            Self::Memory(store) => store.set_starred(tenant_id, execution_id, starred).await,
            Self::Postgres(store) => store.set_starred(tenant_id, execution_id, starred).await,
        }
    }

    async fn count_running_by_tenant(&self, tenant_id: &str) -> Result<u64, ExecutionError> {
        match self {
            Self::Memory(store) => store.count_running_by_tenant(tenant_id).await,
            Self::Postgres(store) => store.count_running_by_tenant(tenant_id).await,
        }
    }

    async fn count_all_running(&self) -> Result<u64, ExecutionError> {
        match self {
            Self::Memory(store) => store.count_all_running().await,
            Self::Postgres(store) => store.count_all_running().await,
        }
    }

    async fn count_running_by_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<u64, ExecutionError> {
        match self {
            Self::Memory(store) => {
                store
                    .count_running_by_workflow(tenant_id, workflow_id)
                    .await
            }
            Self::Postgres(store) => {
                store
                    .count_running_by_workflow(tenant_id, workflow_id)
                    .await
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct NoopExecutorClient;

impl ExecutorClient for NoopExecutorClient {
    async fn start(&self, _record: &ExecutionRecord) -> Result<(), ExecutionError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct InlineExecutorClient<S> {
    store: S,
    approval_gate: std::sync::Arc<ApprovalGate>,
    token_usage_store: Option<std::sync::Arc<crate::token_usage::PlatformTokenUsageStore>>,
    meter: Option<std::sync::Arc<crate::billing::StripeMeter>>,
}

impl<S> InlineExecutorClient<S>
where
    S: ExecutionStore,
{
    pub fn new(store: S, approval_gate: std::sync::Arc<ApprovalGate>) -> Self {
        Self {
            store,
            approval_gate,
            token_usage_store: None,
            meter: None,
        }
    }

    pub fn with_token_usage(
        mut self,
        store: std::sync::Arc<crate::token_usage::PlatformTokenUsageStore>,
    ) -> Self {
        self.token_usage_store = Some(store);
        self
    }

    /// Attaches a Stripe meter so each run's token usage is reported for
    /// metered billing. `None` (the default) disables metered reporting.
    pub fn with_meter(
        mut self,
        meter: Option<std::sync::Arc<crate::billing::StripeMeter>>,
    ) -> Self {
        self.meter = meter;
        self
    }
}

/// Progress callback that writes each completed node result into the execution store
/// so live SSE connections can observe incremental updates.
struct StoreProgressCallback<S> {
    store: S,
    tenant_id: String,
    execution_id: String,
    nodes_by_id: std::sync::Arc<HashMap<String, String>>,
}

impl<S: ExecutionStore + Clone + 'static> NodeProgressCallback for StoreProgressCallback<S> {
    fn on_node_complete(&self, report: &NodeReport) {
        let store = self.store.clone();
        let tenant_id = self.tenant_id.clone();
        let execution_id = self.execution_id.clone();
        let node_type = self
            .nodes_by_id
            .get(&report.node_id)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let node_result = NodeExecutionRecord {
            node_id: report.node_id.clone(),
            node_type,
            status: report.status.clone(),
            output_json: report.output_json.clone(),
            error: report.error.clone(),
            duration_ms: report.duration_ms,
            started_at_ms: report.started_at_ms,
            retry_count: report.retry_count,
        };
        // Push the single completed node to any live SSE subscribers
        // immediately (real-time), bypassing the DB-poll latency. The DB write
        // below is still the source of truth for reconnect/snapshot.
        if let Ok(data) = serde_json::to_string(&node_result) {
            crate::execution_bus::publish(&self.execution_id, "node", data);
        }
        tokio::spawn(async move {
            let _ = store
                .append_node_result(&tenant_id, &execution_id, node_result)
                .await;
        });
    }
}

impl<S> ExecutorClient for InlineExecutorClient<S>
where
    S: ExecutionStore + Clone + 'static,
{
    async fn start(&self, record: &ExecutionRecord) -> Result<(), ExecutionError> {
        let gate = std::sync::Arc::clone(&self.approval_gate);
        let ai_url = std::env::var("AI_RUNTIME_BASE_URL").ok();
        let node_executor = trigix_executor::executor::DispatchingNodeExecutor::new(ai_url)
            .with_approval_gate(gate);

        let nodes_by_id: HashMap<String, String> = record
            .graph
            .nodes
            .iter()
            .map(|n| (n.id.clone(), node_type_to_str(&n.node_type).to_string()))
            .collect();
        let progress = StoreProgressCallback {
            store: self.store.clone(),
            tenant_id: record.tenant_id.clone(),
            execution_id: record.id.clone(),
            nodes_by_id: std::sync::Arc::new(nodes_by_id),
        };

        // Install a token sink so LLM nodes can stream deltas to the execution
        // bus in real time while this run executes. The final node output is
        // unchanged whether or not streaming happens.
        let stream_exec_id = record.id.clone();
        let token_sink: execution_core::TokenSink =
            std::sync::Arc::new(move |node_id: &str, delta: &str| {
                let data = serde_json::json!({ "node_id": node_id, "delta": delta }).to_string();
                crate::execution_bus::publish(&stream_exec_id, "token", data);
            });
        let report = execution_core::TOKEN_SINK
            .scope(
                Some(token_sink),
                run_workflow_with_progress(
                    record.id.clone(),
                    &record.graph,
                    record.input_json.clone(),
                    &node_executor,
                    &progress,
                    record.dry_run,
                ),
            )
            .await
            .map_err(|_| ExecutionError::ExecutorUnavailable)?;

        let completed = self
            .store
            .complete(&record.tenant_id, &record.id, report)
            .await?;
        // Push the terminal snapshot to live subscribers immediately, then drop
        // the channel so the bus map doesn't grow unbounded.
        if let Ok(data) = serde_json::to_string(&completed) {
            crate::execution_bus::publish(&record.id, "update", data);
        }
        crate::execution_bus::close(&record.id);
        // Decrement running counter regardless of final status.
        crate::http::METRIC_EXEC_RUNNING.fetch_sub(1, AtomicOrdering::Relaxed);
        match completed.status {
            ExecutionStatus::Succeeded => {
                crate::http::METRIC_EXEC_SUCCEEDED.fetch_add(1, AtomicOrdering::Relaxed)
            }
            ExecutionStatus::Failed => {
                crate::http::METRIC_EXEC_FAILED.fetch_add(1, AtomicOrdering::Relaxed)
            }
            ExecutionStatus::Cancelled => {
                crate::http::METRIC_EXEC_CANCELLED.fetch_add(1, AtomicOrdering::Relaxed)
            }
            _ => 0,
        };
        fire_callback_if_set(&completed);
        fire_failure_alert_if_set(&completed);
        if let Some(usage_store) = &self.token_usage_store {
            let now = unix_now();
            let usage_records = crate::token_usage::extract_token_usage(
                &completed.tenant_id,
                &completed.id,
                &completed.node_results,
                now,
            );
            // Report this run's total token usage to Stripe for metered billing.
            // Resolve the customer here (in the async, non-detached context) so
            // the Postgres lookup may block-in-place; only the HTTP send is
            // detached and fire-and-forget.
            if let Some(meter) = &self.meter {
                let total_tokens: i64 = usage_records.iter().map(|r| r.total_tokens).sum();
                if total_tokens > 0 {
                    if let Some(customer_id) = meter.customer_for(&completed.tenant_id) {
                        let meter = std::sync::Arc::clone(meter);
                        tokio::spawn(async move {
                            meter.report(&customer_id, total_tokens).await;
                        });
                    }
                }
            }
            for rec in usage_records {
                let store = std::sync::Arc::clone(usage_store);
                tokio::spawn(async move {
                    store.record(rec).await;
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct HttpExecutorClient<S> {
    endpoint: String,
    http: reqwest::Client,
    store: S,
}

impl<S> HttpExecutorClient<S>
where
    S: ExecutionStore,
{
    pub fn new(base_url: impl Into<String>, store: S) -> Self {
        Self {
            endpoint: format!(
                "{}/v1/executions:run",
                base_url.into().trim_end_matches('/')
            ),
            http: reqwest::Client::new(),
            store,
        }
    }
}

impl<S> ExecutorClient for HttpExecutorClient<S>
where
    S: ExecutionStore,
{
    async fn start(&self, record: &ExecutionRecord) -> Result<(), ExecutionError> {
        let request = RemoteRunExecutionRequest {
            execution_id: record.id.clone(),
            graph: record.graph.clone(),
            input_json: record.input_json.clone(),
        };

        let response = self
            .http
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|_| ExecutionError::ExecutorUnavailable)?;

        if !response.status().is_success() {
            return Err(ExecutionError::ExecutorUnavailable);
        }

        let report = response
            .json::<ExecutionReport>()
            .await
            .map_err(|_| ExecutionError::ExecutorUnavailable)?;

        let completed = self
            .store
            .complete(&record.tenant_id, &record.id, report)
            .await?;
        fire_callback_if_set(&completed);
        Ok(())
    }
}

// ── Queue executor client (Redis Streams) ─────────────────────────────────────

/// Pushes execution jobs to a Redis Stream. A separate worker loop consumes them.
#[derive(Clone)]
pub struct QueueExecutorClient {
    cache: std::sync::Arc<crate::cache::CacheClient>,
}

impl QueueExecutorClient {
    pub fn new(cache: crate::cache::CacheClient) -> Self {
        Self {
            cache: std::sync::Arc::new(cache),
        }
    }
}

impl ExecutorClient for QueueExecutorClient {
    async fn start(&self, record: &ExecutionRecord) -> Result<(), ExecutionError> {
        let job = serde_json::to_string(record).map_err(|_| ExecutionError::ExecutorUnavailable)?;
        let stream = crate::cache::keys::exec_queue_stream();
        self.cache
            .xadd(stream, &[("job", &job)])
            .await
            .ok_or(ExecutionError::ExecutorUnavailable)?;
        Ok(())
    }
}

#[derive(Clone)]
pub enum PlatformExecutorClient {
    Inline(InlineExecutorClient<PlatformExecutionStore>),
    Http(HttpExecutorClient<PlatformExecutionStore>),
    Noop(NoopExecutorClient),
    Queue(QueueExecutorClient),
}

impl PlatformExecutorClient {
    pub fn inline(store: PlatformExecutionStore) -> Self {
        Self::inline_with_gate(store, std::sync::Arc::new(ApprovalGate::default()))
    }

    pub fn inline_with_gate(
        store: PlatformExecutionStore,
        gate: std::sync::Arc<ApprovalGate>,
    ) -> Self {
        Self::Inline(InlineExecutorClient::new(store, gate))
    }

    pub fn inline_with_gate_and_usage(
        store: PlatformExecutionStore,
        gate: std::sync::Arc<ApprovalGate>,
        token_usage_store: std::sync::Arc<crate::token_usage::PlatformTokenUsageStore>,
        meter: Option<std::sync::Arc<crate::billing::StripeMeter>>,
    ) -> Self {
        Self::Inline(
            InlineExecutorClient::new(store, gate)
                .with_token_usage(token_usage_store)
                .with_meter(meter),
        )
    }

    pub fn http(base_url: impl Into<String>, store: PlatformExecutionStore) -> Self {
        Self::Http(HttpExecutorClient::new(base_url, store))
    }

    pub fn noop() -> Self {
        Self::Noop(NoopExecutorClient)
    }

    pub fn queue(cache: crate::cache::CacheClient) -> Self {
        Self::Queue(QueueExecutorClient::new(cache))
    }
}

impl ExecutorClient for PlatformExecutorClient {
    async fn start(&self, record: &ExecutionRecord) -> Result<(), ExecutionError> {
        match self {
            Self::Inline(client) => client.start(record).await,
            Self::Http(client) => client.start(record).await,
            Self::Noop(client) => client.start(record).await,
            Self::Queue(client) => client.start(record).await,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RemoteRunExecutionRequest {
    execution_id: String,
    graph: WorkflowGraph,
    input_json: String,
}

#[derive(Clone)]
pub struct PostgresExecutionStore {
    pool: PgPool,
}

impl PostgresExecutionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ExecutionStore for PostgresExecutionStore {
    async fn create(
        &self,
        request: StartExecutionRequest,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let input_json: serde_json::Value =
            serde_json::from_str(&request.input_json).map_err(|_| ExecutionError::InvalidInput)?;
        let graph_json =
            serde_json::to_value(&request.graph).map_err(|_| ExecutionError::InvalidGraph)?;
        let id = next_id();
        let now = unix_now();

        let node_count = request.graph.nodes.len() as i32;
        sqlx::query(
            r#"
            INSERT INTO af_executions
              (id, tenant_id, workflow_id, workflow_version_id, status, input_json, graph_json, started_at, callback_url, trigger_type, dry_run, node_count, retried_from)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(&id)
        .bind(&request.tenant_id)
        .bind(&request.workflow_id)
        .bind(&request.workflow_version_id)
        .bind(status_to_str(&ExecutionStatus::Running))
        .bind(Json(input_json))
        .bind(Json(graph_json))
        .bind(now as i64)
        .bind(&request.callback_url)
        .bind(&request.trigger_type)
        .bind(request.dry_run)
        .bind(node_count)
        .bind(&request.retried_from)
        .execute(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;

        Ok(ExecutionRecord {
            id,
            tenant_id: request.tenant_id,
            workflow_id: request.workflow_id,
            workflow_version_id: request.workflow_version_id,
            graph: request.graph,
            input_json: request.input_json,
            status: ExecutionStatus::Running,
            node_results: Vec::new(),
            started_at: unix_now(),
            finished_at: None,
            label: request.label,
            output_json: None,
            callback_url: request.callback_url,
            trigger_type: request.trigger_type,
            dry_run: request.dry_run,
            note: None,
            starred: false,
            node_count: node_count as u32,
            completed_node_count: 0,
            retried_from: request.retried_from,
        })
    }

    async fn get(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let row = sqlx::query_as::<_, PostgresExecutionRow>(
            r#"
            SELECT id, tenant_id, workflow_id, workflow_version_id,
                   status, input_json, graph_json, started_at, finished_at, output_json, callback_url, trigger_type, dry_run, note, starred,
                   node_count, completed_node_count, retried_from
            FROM af_executions
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?
        .ok_or(ExecutionError::NotFound)?;

        let mut record = row.try_into_record()?;
        record.node_results = self.list_node_results(tenant_id, execution_id).await?;
        Ok(record)
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        let rows = sqlx::query_as::<_, PostgresExecutionSummaryRow>(
            r#"
            SELECT id, tenant_id, workflow_id, workflow_version_id, status, started_at, finished_at, label, trigger_type, dry_run, starred, node_count, completed_node_count, retried_from
            FROM af_executions
            WHERE tenant_id = $1
            ORDER BY started_at DESC, id DESC
            LIMIT 100
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;

        rows.into_iter()
            .map(PostgresExecutionSummaryRow::try_into_summary)
            .collect()
    }

    async fn complete(
        &self,
        tenant_id: &str,
        execution_id: &str,
        report: ExecutionReport,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let record = self.get(tenant_id, execution_id).await?;
        let node_results = build_node_execution_records(&record.graph, &report);
        let output_json = extract_workflow_output(&node_results);
        let now = unix_now() as i64;

        sqlx::query(
            r#"UPDATE af_executions SET status = $3, finished_at = $4, output_json = $5 WHERE tenant_id = $1 AND id = $2"#,
        )
        .bind(tenant_id)
        .bind(execution_id)
        .bind(status_to_str(&report.status))
        .bind(now)
        .bind(&output_json)
        .execute(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;

        // Upsert each node result keyed on (tenant_id, execution_id, node_id).
        // The live progress callback may have already written some of these rows
        // (and may still be racing to), so a delete-then-insert here could
        // duplicate; the unique index lets both writers converge on one row.
        for node in node_results {
            let output_json = parse_optional_json(node.output_json.as_deref())?;
            sqlx::query(
                r#"
                INSERT INTO af_node_executions (id, tenant_id, execution_id, node_id, node_type, status, output_json, error, duration_ms, started_at_ms)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (tenant_id, execution_id, node_id) DO UPDATE SET
                    node_type = EXCLUDED.node_type,
                    status = EXCLUDED.status,
                    output_json = EXCLUDED.output_json,
                    error = EXCLUDED.error,
                    duration_ms = EXCLUDED.duration_ms,
                    started_at_ms = EXCLUDED.started_at_ms
                "#,
            )
            .bind(next_id())
            .bind(tenant_id)
            .bind(execution_id)
            .bind(&node.node_id)
            .bind(&node.node_type)
            .bind(node_status_to_str(&node.status))
            .bind(output_json.map(Json))
            .bind(node.error.as_deref())
            .bind(node.duration_ms as i64)
            .bind(node.started_at_ms as i64)
            .execute(&self.pool)
            .await
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        }

        self.get(tenant_id, execution_id).await
    }

    async fn fail(
        &self,
        tenant_id: &str,
        execution_id: &str,
        _error: String,
    ) -> Result<(), ExecutionError> {
        sqlx::query(
            r#"UPDATE af_executions SET status = 'failed', finished_at = $3 WHERE tenant_id = $1 AND id = $2"#,
        )
        .bind(tenant_id)
        .bind(execution_id)
        .bind(unix_now() as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;
        Ok(())
    }

    async fn cancel(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        sqlx::query(
            r#"
            UPDATE af_executions
            SET status = 'cancelled', finished_at = $3
            WHERE tenant_id = $1 AND id = $2
              AND status IN ('running', 'waiting_approval')
            "#,
        )
        .bind(tenant_id)
        .bind(execution_id)
        .bind(unix_now() as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;
        Ok(())
    }

    async fn append_node_result(
        &self,
        tenant_id: &str,
        execution_id: &str,
        node_result: NodeExecutionRecord,
    ) -> Result<(), ExecutionError> {
        // Upsert this node's row (a later status for the same node replaces the
        // earlier one). Keyed on (tenant_id, execution_id, node_id) so this can
        // race with `complete` without leaving duplicate rows.
        let output_json = parse_optional_json(node_result.output_json.as_deref())?;
        let is_terminal = matches!(
            node_result.status,
            NodeStatus::Succeeded | NodeStatus::Failed | NodeStatus::Skipped
        );
        sqlx::query(
            r#"
            INSERT INTO af_node_executions (id, tenant_id, execution_id, node_id, node_type, status, output_json, error, duration_ms, started_at_ms)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (tenant_id, execution_id, node_id) DO UPDATE SET
                node_type = EXCLUDED.node_type,
                status = EXCLUDED.status,
                output_json = EXCLUDED.output_json,
                error = EXCLUDED.error,
                duration_ms = EXCLUDED.duration_ms,
                started_at_ms = EXCLUDED.started_at_ms
            "#,
        )
        .bind(next_id())
        .bind(tenant_id)
        .bind(execution_id)
        .bind(&node_result.node_id)
        .bind(&node_result.node_type)
        .bind(node_status_to_str(&node_result.status))
        .bind(output_json.map(sqlx::types::Json))
        .bind(node_result.error.as_deref())
        .bind(node_result.duration_ms as i64)
        .bind(node_result.started_at_ms as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;
        if is_terminal {
            sqlx::query(
                r#"UPDATE af_executions SET completed_node_count = (
                    SELECT COUNT(*) FROM af_node_executions
                    WHERE tenant_id = $1 AND execution_id = $2
                      AND status IN ('succeeded','failed','skipped')
                ) WHERE tenant_id = $1 AND id = $2"#,
            )
            .bind(tenant_id)
            .bind(execution_id)
            .execute(&self.pool)
            .await
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        }
        Ok(())
    }

    async fn list_stale_running(
        &self,
        older_than_secs: u64,
        now_unix: u64,
    ) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        let cutoff = now_unix.saturating_sub(older_than_secs) as i64;
        let rows = sqlx::query_as::<_, PostgresExecutionSummaryRow>(
            r#"
            SELECT id, tenant_id, workflow_id, workflow_version_id, status, started_at, finished_at, label, trigger_type, dry_run, starred, node_count, completed_node_count, retried_from
            FROM af_executions
            WHERE status IN ('running', 'waiting_approval') AND started_at < $1
            ORDER BY started_at ASC
            LIMIT 100
            "#,
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;

        rows.into_iter()
            .map(PostgresExecutionSummaryRow::try_into_summary)
            .collect()
    }

    async fn delete(&self, tenant_id: &str, execution_id: &str) -> Result<(), ExecutionError> {
        let result = sqlx::query("DELETE FROM af_executions WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id)
            .bind(execution_id)
            .execute(&self.pool)
            .await
            .map_err(|_| ExecutionError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            Err(ExecutionError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn set_label(
        &self,
        tenant_id: &str,
        execution_id: &str,
        label: Option<String>,
    ) -> Result<(), ExecutionError> {
        let result =
            sqlx::query("UPDATE af_executions SET label = $1 WHERE tenant_id = $2 AND id = $3")
                .bind(label)
                .bind(tenant_id)
                .bind(execution_id)
                .execute(&self.pool)
                .await
                .map_err(|_| ExecutionError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            Err(ExecutionError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn set_note(
        &self,
        tenant_id: &str,
        execution_id: &str,
        note: Option<String>,
    ) -> Result<(), ExecutionError> {
        let result =
            sqlx::query("UPDATE af_executions SET note = $1 WHERE tenant_id = $2 AND id = $3")
                .bind(note)
                .bind(tenant_id)
                .bind(execution_id)
                .execute(&self.pool)
                .await
                .map_err(|_| ExecutionError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            Err(ExecutionError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn set_starred(
        &self,
        tenant_id: &str,
        execution_id: &str,
        starred: bool,
    ) -> Result<(), ExecutionError> {
        let result =
            sqlx::query("UPDATE af_executions SET starred = $1 WHERE tenant_id = $2 AND id = $3")
                .bind(starred)
                .bind(tenant_id)
                .bind(execution_id)
                .execute(&self.pool)
                .await
                .map_err(|_| ExecutionError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            Err(ExecutionError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn count_running_by_tenant(&self, tenant_id: &str) -> Result<u64, ExecutionError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM af_executions WHERE tenant_id = $1 AND status IN ('running', 'waiting_approval')"
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;
        Ok(row.0 as u64)
    }

    async fn count_all_running(&self) -> Result<u64, ExecutionError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM af_executions WHERE status IN ('running', 'waiting_approval')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;
        Ok(row.0 as u64)
    }

    async fn count_running_by_workflow(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<u64, ExecutionError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM af_executions WHERE tenant_id = $1 AND workflow_id = $2 AND status IN ('running', 'waiting_approval')"
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;
        Ok(row.0 as u64)
    }
}

impl PostgresExecutionStore {
    async fn list_node_results(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<Vec<NodeExecutionRecord>, ExecutionError> {
        let rows = sqlx::query_as::<_, PostgresNodeExecutionRow>(
            r#"
            SELECT node_id, node_type, status, output_json, error,
                   COALESCE(duration_ms, 0) AS duration_ms,
                   COALESCE(started_at_ms, 0) AS started_at_ms
            FROM af_node_executions
            WHERE tenant_id = $1 AND execution_id = $2
            ORDER BY node_id ASC
            "#,
        )
        .bind(tenant_id)
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;

        rows.into_iter()
            .map(PostgresNodeExecutionRow::try_into_record)
            .collect()
    }
}

#[derive(sqlx::FromRow)]
struct PostgresExecutionRow {
    id: String,
    tenant_id: String,
    workflow_id: String,
    workflow_version_id: String,
    status: String,
    input_json: serde_json::Value,
    graph_json: serde_json::Value,
    started_at: i64,
    finished_at: Option<i64>,
    #[sqlx(default)]
    label: Option<String>,
    #[sqlx(default)]
    output_json: Option<String>,
    #[sqlx(default)]
    callback_url: Option<String>,
    #[sqlx(default)]
    trigger_type: Option<String>,
    #[sqlx(default)]
    dry_run: bool,
    #[sqlx(default)]
    note: Option<String>,
    #[sqlx(default)]
    starred: bool,
    #[sqlx(default)]
    node_count: i32,
    #[sqlx(default)]
    completed_node_count: i32,
    #[sqlx(default)]
    retried_from: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PostgresExecutionSummaryRow {
    id: String,
    tenant_id: String,
    workflow_id: String,
    workflow_version_id: String,
    status: String,
    started_at: i64,
    #[sqlx(default)]
    finished_at: Option<i64>,
    #[sqlx(default)]
    label: Option<String>,
    #[sqlx(default)]
    trigger_type: Option<String>,
    #[sqlx(default)]
    dry_run: bool,
    #[sqlx(default)]
    starred: bool,
    #[sqlx(default)]
    node_count: i32,
    #[sqlx(default)]
    completed_node_count: i32,
    #[sqlx(default)]
    retried_from: Option<String>,
}

impl PostgresExecutionSummaryRow {
    fn try_into_summary(self) -> Result<ExecutionSummary, ExecutionError> {
        Ok(ExecutionSummary {
            id: self.id,
            tenant_id: self.tenant_id,
            workflow_id: self.workflow_id,
            workflow_version_id: self.workflow_version_id,
            status: status_from_str(&self.status)?,
            started_at: self.started_at as u64,
            finished_at: self.finished_at.map(|v| v as u64),
            label: self.label,
            trigger_type: self.trigger_type,
            dry_run: self.dry_run,
            starred: self.starred,
            node_count: self.node_count as u32,
            completed_node_count: self.completed_node_count as u32,
            retried_from: self.retried_from,
        })
    }
}

impl PostgresExecutionRow {
    fn try_into_record(self) -> Result<ExecutionRecord, ExecutionError> {
        let graph: WorkflowGraph =
            serde_json::from_value(self.graph_json).map_err(|_| ExecutionError::InvalidGraph)?;

        Ok(ExecutionRecord {
            id: self.id,
            tenant_id: self.tenant_id,
            workflow_id: self.workflow_id,
            workflow_version_id: self.workflow_version_id,
            graph,
            input_json: self.input_json.to_string(),
            status: status_from_str(&self.status)?,
            node_results: Vec::new(),
            started_at: self.started_at as u64,
            finished_at: self.finished_at.map(|t| t as u64),
            label: self.label,
            output_json: self.output_json,
            callback_url: self.callback_url,
            trigger_type: self.trigger_type,
            dry_run: self.dry_run,
            note: self.note,
            starred: self.starred,
            node_count: self.node_count as u32,
            completed_node_count: self.completed_node_count as u32,
            retried_from: self.retried_from,
        })
    }
}

#[derive(sqlx::FromRow)]
struct PostgresNodeExecutionRow {
    node_id: String,
    node_type: String,
    status: String,
    output_json: Option<serde_json::Value>,
    error: Option<String>,
    #[sqlx(default)]
    duration_ms: i64,
    #[sqlx(default)]
    started_at_ms: i64,
}

impl PostgresNodeExecutionRow {
    fn try_into_record(self) -> Result<NodeExecutionRecord, ExecutionError> {
        Ok(NodeExecutionRecord {
            node_id: self.node_id,
            node_type: self.node_type,
            status: node_status_from_str(&self.status)?,
            output_json: self.output_json.map(|value| value.to_string()),
            error: self.error,
            duration_ms: self.duration_ms as u64,
            started_at_ms: self.started_at_ms as u64,
            retry_count: 0,
        })
    }
}

fn validate_start_request(request: &StartExecutionRequest) -> Result<(), ExecutionError> {
    if request.tenant_id.is_empty() {
        return Err(ExecutionError::MissingTenant);
    }
    if request.workflow_id.is_empty() {
        return Err(ExecutionError::MissingWorkflow);
    }
    if request.workflow_version_id.is_empty() {
        return Err(ExecutionError::MissingWorkflowVersion);
    }
    if request.graph.workflow_version_id != request.workflow_version_id {
        return Err(ExecutionError::WorkflowVersionMismatch);
    }
    request
        .graph
        .validate()
        .map_err(|_| ExecutionError::InvalidGraph)?;
    validate_input_against_schema(&request.input_json, &request.graph.input_schema)?;
    Ok(())
}

fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "null",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn validate_input_against_schema(
    input_json: &str,
    schema: &[workflow_core::InputField],
) -> Result<(), ExecutionError> {
    if schema.is_empty() {
        return Ok(());
    }
    let input: serde_json::Value =
        serde_json::from_str(input_json).map_err(|_| ExecutionError::InvalidInput)?;
    let obj = input.as_object().ok_or(ExecutionError::InvalidInput)?;

    for field in schema {
        match obj.get(&field.key) {
            None if field.required && field.default_value.is_none() => {
                return Err(ExecutionError::InputSchemaViolation(format!(
                    "required field '{}' is missing",
                    field.key
                )));
            }
            Some(v) => {
                let type_ok = match field.field_type.as_str() {
                    "string" => v.is_string(),
                    "number" => v.is_number(),
                    "boolean" => v.is_boolean(),
                    _ => true,
                };
                if !type_ok {
                    return Err(ExecutionError::InputSchemaViolation(format!(
                        "field '{}' expected type '{}', got '{}'",
                        field.key,
                        field.field_type,
                        json_type_name(v)
                    )));
                }
            }
            None => {}
        }
    }
    Ok(())
}

fn key(tenant_id: &str, execution_id: &str) -> String {
    format!("{tenant_id}:{execution_id}")
}

fn execution_summary(record: &ExecutionRecord) -> ExecutionSummary {
    ExecutionSummary {
        id: record.id.clone(),
        tenant_id: record.tenant_id.clone(),
        workflow_id: record.workflow_id.clone(),
        workflow_version_id: record.workflow_version_id.clone(),
        status: record.status.clone(),
        started_at: record.started_at,
        finished_at: record.finished_at,
        label: record.label.clone(),
        trigger_type: record.trigger_type.clone(),
        dry_run: record.dry_run,
        starred: record.starred,
        node_count: record.node_count,
        completed_node_count: record
            .node_results
            .iter()
            .filter(|nr| {
                matches!(
                    nr.status,
                    NodeStatus::Succeeded | NodeStatus::Failed | NodeStatus::Skipped
                )
            })
            .count() as u32,
        retried_from: record.retried_from.clone(),
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn next_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn status_to_str(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Running => "running",
        ExecutionStatus::WaitingApproval => "waiting_approval",
        ExecutionStatus::Succeeded => "succeeded",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::Cancelled => "cancelled",
    }
}

fn status_from_str(status: &str) -> Result<ExecutionStatus, ExecutionError> {
    match status {
        "running" => Ok(ExecutionStatus::Running),
        "waiting_approval" => Ok(ExecutionStatus::WaitingApproval),
        "succeeded" => Ok(ExecutionStatus::Succeeded),
        "failed" => Ok(ExecutionStatus::Failed),
        "cancelled" => Ok(ExecutionStatus::Cancelled),
        _ => Err(ExecutionError::StoreUnavailable),
    }
}

fn node_status_to_str(status: &NodeStatus) -> &'static str {
    match status {
        NodeStatus::Running => "running",
        NodeStatus::Succeeded => "succeeded",
        NodeStatus::Failed => "failed",
        NodeStatus::Skipped => "skipped",
    }
}

fn node_status_from_str(status: &str) -> Result<NodeStatus, ExecutionError> {
    match status {
        "running" => Ok(NodeStatus::Running),
        "succeeded" => Ok(NodeStatus::Succeeded),
        "failed" => Ok(NodeStatus::Failed),
        "skipped" => Ok(NodeStatus::Skipped),
        _ => Err(ExecutionError::StoreUnavailable),
    }
}

fn build_node_execution_records(
    graph: &WorkflowGraph,
    report: &ExecutionReport,
) -> Vec<NodeExecutionRecord> {
    let nodes_by_id: HashMap<&str, &Node> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    report
        .node_results
        .iter()
        .map(|node| {
            let node_type = nodes_by_id
                .get(node.node_id.as_str())
                .map(|node| node_type_to_str(&node.node_type))
                .unwrap_or("unknown");

            NodeExecutionRecord {
                node_id: node.node_id.clone(),
                node_type: node_type.to_string(),
                status: node.status.clone(),
                output_json: node.output_json.clone(),
                error: node.error.clone(),
                duration_ms: node.duration_ms,
                started_at_ms: node.started_at_ms,
                retry_count: node.retry_count,
            }
        })
        .collect()
}

/// Extract the workflow's final output: the last succeeded non-trigger node's output_json.
fn extract_workflow_output(node_results: &[NodeExecutionRecord]) -> Option<String> {
    node_results
        .iter()
        .rev()
        .find(|nr| {
            nr.status == NodeStatus::Succeeded
                && nr.node_type != "trigger"
                && nr.output_json.is_some()
        })
        .and_then(|nr| nr.output_json.clone())
}

fn node_type_to_str(node_type: &workflow_core::NodeType) -> &'static str {
    match node_type {
        workflow_core::NodeType::Trigger => "trigger",
        workflow_core::NodeType::Http => "http",
        workflow_core::NodeType::Agent => "agent",
        workflow_core::NodeType::Rag => "rag",
        workflow_core::NodeType::RagIngest => "rag_ingest",
        workflow_core::NodeType::Custom => "custom",
        workflow_core::NodeType::Condition => "condition",
        workflow_core::NodeType::Approval => "approval",
        workflow_core::NodeType::Map => "map",
        workflow_core::NodeType::Filter => "filter",
        workflow_core::NodeType::Aggregate => "aggregate",
        workflow_core::NodeType::Sort => "sort",
        workflow_core::NodeType::Transform => "transform",
        workflow_core::NodeType::Delay => "delay",
        workflow_core::NodeType::SubWorkflow => "sub_workflow",
        workflow_core::NodeType::Assert => "assert",
        workflow_core::NodeType::Catch => "catch",
        workflow_core::NodeType::FanOut => "fan_out",
        workflow_core::NodeType::FanIn => "fan_in",
        workflow_core::NodeType::Code => "code",
        workflow_core::NodeType::Slack => "slack",
        workflow_core::NodeType::Email => "email",
        workflow_core::NodeType::Openai => "openai",
        workflow_core::NodeType::Gemini => "gemini",
        workflow_core::NodeType::Database => "database",
        workflow_core::NodeType::Extract => "extract",
        workflow_core::NodeType::Merge => "merge",
        workflow_core::NodeType::Loop => "loop",
        workflow_core::NodeType::Graphql => "graphql",
        workflow_core::NodeType::Validate => "validate",
        workflow_core::NodeType::Note => "note",
        workflow_core::NodeType::Claude => "claude",
        workflow_core::NodeType::Split => "split",
        workflow_core::NodeType::Join => "join",
        workflow_core::NodeType::Switch => "switch",
        workflow_core::NodeType::Random => "random",
        workflow_core::NodeType::Dedupe => "dedupe",
        workflow_core::NodeType::Regex => "regex",
        workflow_core::NodeType::Csv => "csv",
        workflow_core::NodeType::Rename => "rename",
        workflow_core::NodeType::Format => "format",
        workflow_core::NodeType::Github => "github",
        workflow_core::NodeType::Webhook => "webhook",
        workflow_core::NodeType::Jira => "jira",
        workflow_core::NodeType::Notion => "notion",
        workflow_core::NodeType::Linear => "linear",
        workflow_core::NodeType::Airtable => "airtable",
        workflow_core::NodeType::ForEach => "for_each",
        workflow_core::NodeType::Discord => "discord",
        workflow_core::NodeType::Teams => "teams",
        workflow_core::NodeType::Sheets => "sheets",
        workflow_core::NodeType::Xml => "xml",
        workflow_core::NodeType::Yaml => "yaml",
        workflow_core::NodeType::Twilio => "twilio",
        workflow_core::NodeType::Stripe => "stripe",
        workflow_core::NodeType::Crypto => "crypto",
        workflow_core::NodeType::Hubspot => "hubspot",
        workflow_core::NodeType::Date => "date",
        workflow_core::NodeType::Zendesk => "zendesk",
        workflow_core::NodeType::Redis => "redis",
        workflow_core::NodeType::Elasticsearch => "elasticsearch",
        workflow_core::NodeType::Pagerduty => "pagerduty",
        workflow_core::NodeType::Handlebars => "handlebars",
        workflow_core::NodeType::Math => "math",
        workflow_core::NodeType::ArrayUtils => "array_utils",
        workflow_core::NodeType::Shopify => "shopify",
        workflow_core::NodeType::Datadog => "datadog",
        workflow_core::NodeType::Salesforce => "salesforce",
        workflow_core::NodeType::Freshdesk => "freshdesk",
        workflow_core::NodeType::Mailgun => "mailgun",
        workflow_core::NodeType::Asana => "asana",
        workflow_core::NodeType::Servicenow => "servicenow",
        workflow_core::NodeType::Confluence => "confluence",
        workflow_core::NodeType::Bitbucket => "bitbucket",
        workflow_core::NodeType::AzureDevops => "azure_devops",
        workflow_core::NodeType::Twitch => "twitch",
        workflow_core::NodeType::Figma => "figma",
        workflow_core::NodeType::Dropbox => "dropbox",
        workflow_core::NodeType::Cloudflare => "cloudflare",
        workflow_core::NodeType::Box => "box",
        workflow_core::NodeType::Okta => "okta",
        workflow_core::NodeType::Zoom => "zoom",
        workflow_core::NodeType::Spotify => "spotify",
        workflow_core::NodeType::Typeform => "typeform",
        workflow_core::NodeType::Webflow => "webflow",
        workflow_core::NodeType::Intercom => "intercom",
        workflow_core::NodeType::Pipedrive => "pipedrive",
        workflow_core::NodeType::Trello => "trello",
        workflow_core::NodeType::Monday => "monday",
        workflow_core::NodeType::Clickup => "clickup",
        workflow_core::NodeType::Amplitude => "amplitude",
        workflow_core::NodeType::Mixpanel => "mixpanel",
        workflow_core::NodeType::Segment => "segment",
        workflow_core::NodeType::Sendgrid => "sendgrid",
        workflow_core::NodeType::Braintree => "braintree",
        workflow_core::NodeType::Paypal => "paypal",
        workflow_core::NodeType::Razorpay => "razorpay",
        workflow_core::NodeType::Firebase => "firebase",
        workflow_core::NodeType::Supabase => "supabase",
        workflow_core::NodeType::Mailchimp => "mailchimp",
        workflow_core::NodeType::Activecampaign => "activecampaign",
        workflow_core::NodeType::Klaviyo => "klaviyo",
        workflow_core::NodeType::Resend => "resend",
        workflow_core::NodeType::Contentful => "contentful",
        workflow_core::NodeType::Algolia => "algolia",
        workflow_core::NodeType::Postmark => "postmark",
        workflow_core::NodeType::Vonage => "vonage",
        workflow_core::NodeType::Telegram => "telegram",
        workflow_core::NodeType::Replicate => "replicate",
        workflow_core::NodeType::Mistral => "mistral",
        workflow_core::NodeType::Whatsapp => "whatsapp",
        workflow_core::NodeType::Googledocs => "googledocs",
        workflow_core::NodeType::Perplexity => "perplexity",
        workflow_core::NodeType::Cohere => "cohere",
        workflow_core::NodeType::Googledrive => "googledrive",
        workflow_core::NodeType::Woocommerce => "woocommerce",
        workflow_core::NodeType::Pinecone => "pinecone",
        workflow_core::NodeType::Togetherai => "togetherai",
        workflow_core::NodeType::Awss3 => "awss3",
        workflow_core::NodeType::Huggingface => "huggingface",
        workflow_core::NodeType::Groq => "groq",
        workflow_core::NodeType::Openrouter => "openrouter",
        workflow_core::NodeType::Qdrant => "qdrant",
        workflow_core::NodeType::Cloudinary => "cloudinary",
        workflow_core::NodeType::Gcal => "gcal",
        workflow_core::NodeType::Docusign => "docusign",
        workflow_core::NodeType::Xero => "xero",
        workflow_core::NodeType::Calendly => "calendly",
        workflow_core::NodeType::Apify => "apify",
        workflow_core::NodeType::Ganalytics => "ganalytics",
        workflow_core::NodeType::Neon => "neon",
        workflow_core::NodeType::Copper => "copper",
        workflow_core::NodeType::AzureOpenai => "azure_openai",
        workflow_core::NodeType::Grok => "grok",
        workflow_core::NodeType::Ollama => "ollama",
        workflow_core::NodeType::Weaviate => "weaviate",
        workflow_core::NodeType::Chroma => "chroma",
        workflow_core::NodeType::Mongodb => "mongodb",
        workflow_core::NodeType::Clickhouse => "clickhouse",
        workflow_core::NodeType::Gcs => "gcs",
        workflow_core::NodeType::AzureBlob => "azure_blob",
        workflow_core::NodeType::Hash => "hash",
        workflow_core::NodeType::Jwt => "jwt",
        workflow_core::NodeType::Vertex => "vertex",
        workflow_core::NodeType::Sqs => "sqs",
        workflow_core::NodeType::Sns => "sns",
        workflow_core::NodeType::Bedrock => "bedrock",
        workflow_core::NodeType::Milvus => "milvus",
        workflow_core::NodeType::Kafka => "kafka",
        workflow_core::NodeType::Rabbitmq => "rabbitmq",
        workflow_core::NodeType::Zip => "zip",
        workflow_core::NodeType::Image => "image",
        workflow_core::NodeType::PdfExtract => "pdf_extract",
        workflow_core::NodeType::Ocr => "ocr",
        workflow_core::NodeType::Feishu => "feishu",
        workflow_core::NodeType::Dingtalk => "dingtalk",
        workflow_core::NodeType::Wecom => "wecom",
        workflow_core::NodeType::Embedding => "embedding",
        workflow_core::NodeType::Reranker => "reranker",
        workflow_core::NodeType::TextSplitter => "text_splitter",
        workflow_core::NodeType::StructuredOutput => "structured_output",
        workflow_core::NodeType::Classifier => "classifier",
        workflow_core::NodeType::ImageGen => "image_gen",
        workflow_core::NodeType::VideoGen => "video_gen",
        workflow_core::NodeType::SpeechToText => "speech_to_text",
        workflow_core::NodeType::Tts => "tts",
        workflow_core::NodeType::HtmlExtract => "html_extract",
        workflow_core::NodeType::Rss => "rss",
        workflow_core::NodeType::Mysql => "mysql",
        workflow_core::NodeType::Snowflake => "snowflake",
        workflow_core::NodeType::Bigquery => "bigquery",
        workflow_core::NodeType::Ftp => "ftp",
        workflow_core::NodeType::Sftp => "sftp",
        workflow_core::NodeType::Ssh => "ssh",
        workflow_core::NodeType::Imap => "imap",
        workflow_core::NodeType::Wait => "wait",
        workflow_core::NodeType::Sqlserver => "sqlserver",
        // 国内大模型
        workflow_core::NodeType::Deepseek => "deepseek",
        workflow_core::NodeType::Qwen => "qwen",
        workflow_core::NodeType::Zhipu => "zhipu",
        workflow_core::NodeType::Moonshot => "moonshot",
        workflow_core::NodeType::Doubao => "doubao",
        workflow_core::NodeType::Minimax => "minimax",
        workflow_core::NodeType::Ernie => "ernie",
        workflow_core::NodeType::Hunyuan => "hunyuan",
    }
}

/// POST `body` to `url` with up to `max_attempts` retries and exponential backoff.
/// Adds `X-Trigix-Event` and `X-Trigix-Attempt` headers.
/// Non-blocking: must be called inside `tokio::spawn`.
async fn fire_with_retry(url: String, body: String, event: &'static str, max_attempts: u32) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let mut backoff_secs = 2u64;
    for attempt in 1..=max_attempts {
        let result = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Trigix-Event", event)
            .header("X-Trigix-Attempt", attempt.to_string())
            .body(body.clone())
            .send()
            .await;
        match result {
            Ok(resp) if resp.status().is_success() => return,
            Ok(resp) => {
                tracing::warn!(
                    url = %url, status = %resp.status(), attempt,
                    "webhook delivery failed"
                );
            }
            Err(e) => {
                tracing::warn!(url = %url, error = %e, attempt, "webhook delivery error");
            }
        }
        if attempt < max_attempts {
            tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 4).min(60);
        }
    }
    tracing::warn!(url = %url, max_attempts, "webhook delivery gave up after all retries");
}

/// Fire a non-blocking POST to `record.callback_url` (if set) with the completed record as JSON body.
/// Retries up to 3 times with exponential backoff (2s, 8s, 32s capped at 60s).
fn fire_callback_if_set(record: &ExecutionRecord) {
    let Some(url) = record.callback_url.clone() else {
        return;
    };
    let body = serde_json::to_string(record).unwrap_or_default();
    tokio::spawn(async move {
        fire_with_retry(url, body, "execution.completed", 3).await;
    });
}

/// Fire a failure alert to the `on_failure_url` configured in the Trigger node, if the execution failed.
/// Retries up to 3 times with exponential backoff.
fn fire_failure_alert_if_set(record: &ExecutionRecord) {
    if record.status != ExecutionStatus::Failed {
        return;
    }
    let url = record
        .graph
        .nodes
        .iter()
        .find(|n| n.node_type == workflow_core::NodeType::Trigger)
        .and_then(|n| n.config.as_ref())
        .and_then(|c| c.get("on_failure_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let Some(url) = url else { return };
    if url.is_empty() {
        return;
    }
    let body = serde_json::json!({
        "execution_id": record.id,
        "workflow_id": record.workflow_id,
        "status": "failed",
        "started_at": record.started_at,
        "finished_at": record.finished_at,
    })
    .to_string();
    tokio::spawn(async move {
        fire_with_retry(url, body, "execution.failed", 3).await;
    });
}

fn parse_optional_json(value: Option<&str>) -> Result<Option<serde_json::Value>, ExecutionError> {
    value
        .map(|value| serde_json::from_str(value).map_err(|_| ExecutionError::InvalidInput))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow_core::{Edge, Node, NodeType};

    macro_rules! poll_until_done {
        ($service:expr, $tenant_id:expr, $execution_id:expr) => {{
            let mut result = None;
            for _ in 0..200 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                let r = $service.get($tenant_id, $execution_id).await.unwrap();
                if r.status != ExecutionStatus::Running {
                    result = Some(r);
                    break;
                }
            }
            result.expect("Execution did not complete in time")
        }};
    }

    #[tokio::test]
    async fn starts_and_gets_execution() {
        let executor = NoopExecutorClient;
        let service = ExecutionService::new(MemoryExecutionStore::default(), executor.clone());

        let record = service.start(valid_request()).await.unwrap();
        let loaded = service.get("tenant-1", &record.id).await.unwrap();

        assert_eq!(record.id, loaded.id);
        assert_eq!(loaded.status, ExecutionStatus::Running);
        assert!(loaded.node_results.is_empty());
        assert_eq!(loaded.graph.nodes.len(), 2);
    }

    #[tokio::test]
    async fn lists_executions_by_tenant() {
        let executor = NoopExecutorClient;
        let service = ExecutionService::new(MemoryExecutionStore::default(), executor);

        let record = service.start(valid_request()).await.unwrap();
        let summaries = service.list("tenant-1").await.unwrap();
        let other_tenant_summaries = service.list("tenant-2").await.unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, record.id);
        assert_eq!(summaries[0].status, ExecutionStatus::Running);
        assert!(other_tenant_summaries.is_empty());
    }

    #[tokio::test]
    async fn inline_executor_completes_execution() {
        let store = MemoryExecutionStore::default();
        let executor =
            InlineExecutorClient::new(store.clone(), std::sync::Arc::new(ApprovalGate::default()));
        let service = ExecutionService::new(store, executor);

        let record = service.start(valid_request()).await.unwrap();
        assert_eq!(record.status, ExecutionStatus::Running);

        let loaded = poll_until_done!(service, "tenant-1", &record.id);
        assert_eq!(loaded.status, ExecutionStatus::Succeeded);
        assert_eq!(loaded.node_results.len(), 2);
        assert_eq!(loaded.node_results[0].node_id, "trigger");
        assert_eq!(loaded.node_results[0].node_type, "trigger");
        // Transform node receives {{input.lead_id}} and outputs the resolved value.
        assert_eq!(loaded.node_results[1].node_id, "xform");
        let out: serde_json::Value = serde_json::from_str(
            loaded.node_results[1]
                .output_json
                .as_deref()
                .unwrap_or("{}"),
        )
        .unwrap();
        assert_eq!(out["lead"], "lead-1");
    }

    #[tokio::test]
    async fn node_output_template_resolves_across_nodes() {
        // Tests that {{node_id.field}} syntax works: trigger output feeds into transform.
        let store = MemoryExecutionStore::default();
        let executor =
            InlineExecutorClient::new(store.clone(), std::sync::Arc::new(ApprovalGate::default()));
        let service = ExecutionService::new(store, executor);

        let request = StartExecutionRequest {
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "ver-tmpl".to_string(),
            graph: WorkflowGraph {
                workflow_version_id: "ver-tmpl".to_string(),
                nodes: vec![
                    Node {
                        id: "trigger".to_string(),
                        node_type: NodeType::Trigger,
                        config: None,
                    },
                    Node {
                        id: "step2".to_string(),
                        node_type: NodeType::Transform,
                        // trigger outputs the raw input_json; we pick out "customer" from it
                        config: Some(serde_json::json!({
                            "template": { "greeting": "Hello {{input.customer}}", "echo": "{{trigger}}" }
                        })),
                    },
                ],
                edges: vec![Edge {
                    source: "trigger".to_string(),
                    target: "step2".to_string(),
                    condition_label: None,
                }],
                input_schema: vec![],
            },
            input_json: r#"{"customer":"Alice"}"#.to_string(),
            label: None,
            callback_url: None,
            trigger_type: None,
            dry_run: false,
            retried_from: None,
        };

        let record = service.start(request).await.unwrap();
        let done = poll_until_done!(service, "tenant-1", &record.id);
        assert_eq!(done.status, ExecutionStatus::Succeeded);

        let out: serde_json::Value =
            serde_json::from_str(done.node_results[1].output_json.as_deref().unwrap_or("{}"))
                .unwrap();
        assert_eq!(out["greeting"], "Hello Alice");
        // {{trigger}} resolves to the raw output_json of the trigger node (the input_json)
        assert!(out["echo"].as_str().unwrap().contains("Alice"));
    }

    #[tokio::test]
    async fn platform_executor_can_use_inline_mode() {
        let store = PlatformExecutionStore::memory();
        let executor = PlatformExecutorClient::inline(store.clone());
        let service = ExecutionService::new(store, executor);

        let record = service.start(valid_request()).await.unwrap();
        assert_eq!(record.status, ExecutionStatus::Running);

        let loaded = poll_until_done!(service, "tenant-1", &record.id);
        assert_eq!(loaded.status, ExecutionStatus::Succeeded);
    }

    #[tokio::test]
    async fn rejects_mismatched_workflow_version() {
        let service = ExecutionService::new(MemoryExecutionStore::default(), NoopExecutorClient);
        let mut request = valid_request();
        request.graph.workflow_version_id = "other-version".to_string();

        let err = service.start(request).await.unwrap_err();

        assert_eq!(err, ExecutionError::WorkflowVersionMismatch);
    }

    #[tokio::test]
    async fn rejects_invalid_graph() {
        let service = ExecutionService::new(MemoryExecutionStore::default(), NoopExecutorClient);
        let mut request = valid_request();
        request.graph.edges.push(Edge {
            source: "agent".to_string(),
            target: "trigger".to_string(),
            condition_label: None,
        });

        let err = service.start(request).await.unwrap_err();

        assert_eq!(err, ExecutionError::InvalidGraph);
    }

    #[tokio::test]
    async fn execution_record_has_started_at_and_finished_at() {
        let store = MemoryExecutionStore::default();
        let executor =
            InlineExecutorClient::new(store.clone(), std::sync::Arc::new(ApprovalGate::default()));
        let service = ExecutionService::new(store, executor);

        let record = service.start(valid_request()).await.unwrap();
        assert!(
            record.started_at > 0,
            "started_at should be a unix timestamp"
        );
        assert!(
            record.finished_at.is_none(),
            "finished_at should be None when running"
        );

        let done = poll_until_done!(service, "tenant-1", &record.id);
        assert_eq!(done.status, ExecutionStatus::Succeeded);
        assert!(
            done.finished_at.is_some(),
            "finished_at should be set after completion"
        );
        assert!(done.finished_at.unwrap() >= record.started_at);

        let summaries = service.list("tenant-1").await.unwrap();
        assert_eq!(summaries[0].started_at, done.started_at);
    }

    #[tokio::test]
    async fn cancel_execution_sets_cancelled_status() {
        let store = MemoryExecutionStore::default();
        let executor = NoopExecutorClient;
        let service = ExecutionService::new(store, executor);

        let record = service.start(valid_request()).await.unwrap();
        assert_eq!(record.status, ExecutionStatus::Running);

        service.cancel("tenant-1", &record.id).await.unwrap();
        let cancelled = service.get("tenant-1", &record.id).await.unwrap();
        assert_eq!(cancelled.status, ExecutionStatus::Cancelled);
        assert!(cancelled.finished_at.is_some(), "finished_at set on cancel");

        // Cancelling an already-cancelled execution is idempotent
        service.cancel("tenant-1", &record.id).await.unwrap();
        let still_cancelled = service.get("tenant-1", &record.id).await.unwrap();
        assert_eq!(still_cancelled.status, ExecutionStatus::Cancelled);
    }

    #[test]
    fn input_schema_accepts_valid_input() {
        use workflow_core::InputField;
        let schema = vec![
            InputField {
                key: "name".to_string(),
                field_type: "string".to_string(),
                required: true,
                description: String::new(),
                default_value: None,
            },
            InputField {
                key: "count".to_string(),
                field_type: "number".to_string(),
                required: true,
                description: String::new(),
                default_value: None,
            },
        ];
        let result = validate_input_against_schema(r#"{"name":"Alice","count":3}"#, &schema);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn input_schema_rejects_missing_required_field() {
        use workflow_core::InputField;
        let schema = vec![InputField {
            key: "name".to_string(),
            field_type: "string".to_string(),
            required: true,
            description: String::new(),
            default_value: None,
        }];
        let result = validate_input_against_schema(r#"{}"#, &schema);
        assert!(
            matches!(result, Err(ExecutionError::InputSchemaViolation(msg)) if msg.contains("name"))
        );
    }

    #[test]
    fn input_schema_rejects_wrong_type() {
        use workflow_core::InputField;
        let schema = vec![InputField {
            key: "count".to_string(),
            field_type: "number".to_string(),
            required: true,
            description: String::new(),
            default_value: None,
        }];
        let result = validate_input_against_schema(r#"{"count":"not-a-number"}"#, &schema);
        assert!(
            matches!(result, Err(ExecutionError::InputSchemaViolation(msg)) if msg.contains("count"))
        );
    }

    #[test]
    fn input_schema_optional_field_can_be_absent() {
        use workflow_core::InputField;
        let schema = vec![InputField {
            key: "opt".to_string(),
            field_type: "string".to_string(),
            required: false,
            description: String::new(),
            default_value: None,
        }];
        let result = validate_input_against_schema(r#"{}"#, &schema);
        assert_eq!(result, Ok(()));
    }

    fn valid_request() -> StartExecutionRequest {
        StartExecutionRequest {
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            graph: WorkflowGraph {
                workflow_version_id: "version-1".to_string(),
                nodes: vec![
                    Node {
                        id: "trigger".to_string(),
                        node_type: NodeType::Trigger,
                        config: None,
                    },
                    Node {
                        id: "xform".to_string(),
                        node_type: NodeType::Transform,
                        config: Some(serde_json::json!({
                            "template": { "lead": "{{input.lead_id}}" }
                        })),
                    },
                ],
                edges: vec![Edge {
                    source: "trigger".to_string(),
                    target: "xform".to_string(),
                    condition_label: None,
                }],
                input_schema: vec![],
            },
            input_json: "{\"lead_id\":\"lead-1\"}".to_string(),
            label: None,
            callback_url: None,
            trigger_type: None,
            dry_run: false,
            retried_from: None,
        }
    }

    #[tokio::test]
    async fn set_note_persists_and_clears() {
        let service = ExecutionService::new(MemoryExecutionStore::default(), NoopExecutorClient);
        let record = service.start(valid_request()).await.unwrap();

        service
            .set_note(
                "tenant-1",
                &record.id,
                Some("root cause: timeout".to_string()),
            )
            .await
            .unwrap();
        let loaded = service.get("tenant-1", &record.id).await.unwrap();
        assert_eq!(loaded.note.as_deref(), Some("root cause: timeout"));

        service
            .set_note("tenant-1", &record.id, None)
            .await
            .unwrap();
        let cleared = service.get("tenant-1", &record.id).await.unwrap();
        assert!(cleared.note.is_none());
    }

    #[tokio::test]
    async fn set_note_returns_not_found_for_missing() {
        let service = ExecutionService::new(MemoryExecutionStore::default(), NoopExecutorClient);
        let err = service
            .set_note("tenant-1", "no-such-id", Some("x".to_string()))
            .await
            .unwrap_err();
        assert_eq!(err, ExecutionError::NotFound);
    }

    #[tokio::test]
    async fn count_running_by_tenant_reflects_active_executions() {
        let service = ExecutionService::new(MemoryExecutionStore::default(), NoopExecutorClient);

        // No executions yet
        assert_eq!(
            service.count_running_by_tenant("tenant-1").await.unwrap(),
            0
        );

        // Start two executions for tenant-1 and one for tenant-2
        let _r1 = service.start(valid_request()).await.unwrap();
        let mut req2 = valid_request();
        req2.workflow_version_id = "version-2".to_string();
        req2.graph.workflow_version_id = "version-2".to_string();
        let _r2 = service.start(req2).await.unwrap();
        let mut req3 = valid_request();
        req3.tenant_id = "tenant-2".to_string();
        req3.workflow_version_id = "version-3".to_string();
        req3.graph.workflow_version_id = "version-3".to_string();
        let _r3 = service.start(req3).await.unwrap();

        assert_eq!(
            service.count_running_by_tenant("tenant-1").await.unwrap(),
            2
        );
        assert_eq!(
            service.count_running_by_tenant("tenant-2").await.unwrap(),
            1
        );
        assert_eq!(
            service.count_running_by_tenant("tenant-99").await.unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn count_running_excludes_completed_executions() {
        let store = MemoryExecutionStore::default();
        let gate = std::sync::Arc::new(trigix_executor::approval::ApprovalGate::default());
        let executor = InlineExecutorClient::new(store.clone(), gate);
        let service = ExecutionService::new(store, executor);

        let record = service.start(valid_request()).await.unwrap();
        // Wait for inline executor to complete
        let done = poll_until_done!(service, "tenant-1", &record.id);
        assert_eq!(done.status, ExecutionStatus::Succeeded);

        // Completed execution should not count as running
        assert_eq!(
            service.count_running_by_tenant("tenant-1").await.unwrap(),
            0
        );
    }
}
