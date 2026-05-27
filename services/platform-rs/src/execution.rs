use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use agentflow_executor::approval::ApprovalGate;
use agentflow_executor::runtime::{
    run_workflow, ExecutionContext, ExecutionReport, NodeExecutionResult, NodeExecutor,
};
use execution_core::{ExecutionStatus, NodeStatus};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use sqlx::types::Json;
use workflow_core::{Node, NodeType, WorkflowGraph};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartExecutionRequest {
    pub tenant_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub graph: WorkflowGraph,
    pub input_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExecutionRecord {
    pub node_id: String,
    pub node_type: String,
    pub status: NodeStatus,
    pub output_json: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionSummary {
    pub id: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub status: ExecutionStatus,
    pub started_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    MissingTenant,
    MissingWorkflow,
    MissingWorkflowVersion,
    WorkflowVersionMismatch,
    InvalidGraph,
    InvalidInput,
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

    pub async fn cancel(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<(), ExecutionError> {
        self.store.cancel(tenant_id, execution_id).await
    }

    pub async fn list(&self, tenant_id: &str) -> Result<Vec<ExecutionSummary>, ExecutionError> {
        if tenant_id.is_empty() {
            return Err(ExecutionError::MissingTenant);
        }
        self.store.list(tenant_id).await
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
        summaries.sort_by(|left, right| right.started_at.cmp(&left.started_at).then(right.id.cmp(&left.id)));
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
        if matches!(record.status, ExecutionStatus::Running | ExecutionStatus::WaitingApproval) {
            record.status = ExecutionStatus::Cancelled;
            record.finished_at = Some(unix_now());
        }
        Ok(())
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
}

impl<S> InlineExecutorClient<S>
where
    S: ExecutionStore,
{
    pub fn new(store: S, approval_gate: std::sync::Arc<ApprovalGate>) -> Self {
        Self { store, approval_gate }
    }
}

impl<S> ExecutorClient for InlineExecutorClient<S>
where
    S: ExecutionStore,
{
    async fn start(&self, record: &ExecutionRecord) -> Result<(), ExecutionError> {
        let gate = std::sync::Arc::clone(&self.approval_gate);
        let mut node_executor = ApprovalAwareEchoExecutor { gate };
        let report = run_workflow(
            record.id.clone(),
            &record.graph,
            record.input_json.clone(),
            &mut node_executor,
        )
        .await
        .map_err(|_| ExecutionError::ExecutorUnavailable)?;

        self.store
            .complete(&record.tenant_id, &record.id, report)
            .await?;
        Ok(())
    }
}

/// Echo executor that handles approval nodes via the gate; all other nodes return a stub output.
struct ApprovalAwareEchoExecutor {
    gate: std::sync::Arc<ApprovalGate>,
}

impl NodeExecutor for ApprovalAwareEchoExecutor {
    fn execute<'a>(
        &'a mut self,
        node: &'a Node,
        context: &'a ExecutionContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>> {
        Box::pin(async move {
            if node.node_type == NodeType::Approval {
                let rx = self.gate.register(context.execution_id.clone()).await;
                return match rx.await {
                    Ok(true) => NodeExecutionResult::succeeded(r#"{"approved":true}"#.to_string()),
                    Ok(false) => NodeExecutionResult::failed("Rejected by approver".to_string()),
                    Err(_) => NodeExecutionResult::failed("Approval gate was closed".to_string()),
                };
            }
            NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{}\"}}", node.id))
        })
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

        self.store
            .complete(&record.tenant_id, &record.id, report)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub enum PlatformExecutorClient {
    Inline(InlineExecutorClient<PlatformExecutionStore>),
    Http(HttpExecutorClient<PlatformExecutionStore>),
    Noop(NoopExecutorClient),
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

    pub fn http(base_url: impl Into<String>, store: PlatformExecutionStore) -> Self {
        Self::Http(HttpExecutorClient::new(base_url, store))
    }

    pub fn noop() -> Self {
        Self::Noop(NoopExecutorClient)
    }
}

impl ExecutorClient for PlatformExecutorClient {
    async fn start(&self, record: &ExecutionRecord) -> Result<(), ExecutionError> {
        match self {
            Self::Inline(client) => client.start(record).await,
            Self::Http(client) => client.start(record).await,
            Self::Noop(client) => client.start(record).await,
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
        let graph_json = serde_json::to_value(&request.graph)
            .map_err(|_| ExecutionError::InvalidGraph)?;
        let id = next_id();
        let now = unix_now();

        sqlx::query(
            r#"
            INSERT INTO af_executions
              (id, tenant_id, workflow_id, workflow_version_id, status, input_json, graph_json, started_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
                   status, input_json, graph_json, started_at, finished_at
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
            SELECT id, tenant_id, workflow_id, workflow_version_id, status, started_at
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
        let now = unix_now() as i64;

        sqlx::query(
            r#"UPDATE af_executions SET status = $3, finished_at = $4 WHERE tenant_id = $1 AND id = $2"#,
        )
        .bind(tenant_id)
        .bind(execution_id)
        .bind(status_to_str(&report.status))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|_| ExecutionError::StoreUnavailable)?;

        sqlx::query(r#"DELETE FROM af_node_executions WHERE tenant_id = $1 AND execution_id = $2"#)
            .bind(tenant_id)
            .bind(execution_id)
            .execute(&self.pool)
            .await
            .map_err(|_| ExecutionError::StoreUnavailable)?;

        for node in node_results {
            let output_json = parse_optional_json(node.output_json.as_deref())?;
            sqlx::query(
                r#"
                INSERT INTO af_node_executions (id, tenant_id, execution_id, node_id, node_type, status, output_json, error)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
}

impl PostgresExecutionStore {
    async fn list_node_results(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<Vec<NodeExecutionRecord>, ExecutionError> {
        let rows = sqlx::query_as::<_, PostgresNodeExecutionRow>(
            r#"
            SELECT node_id, node_type, status, output_json, error
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
}

#[derive(sqlx::FromRow)]
struct PostgresExecutionSummaryRow {
    id: String,
    tenant_id: String,
    workflow_id: String,
    workflow_version_id: String,
    status: String,
    started_at: i64,
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
}

impl PostgresNodeExecutionRow {
    fn try_into_record(self) -> Result<NodeExecutionRecord, ExecutionError> {
        Ok(NodeExecutionRecord {
            node_id: self.node_id,
            node_type: self.node_type,
            status: node_status_from_str(&self.status)?,
            output_json: self.output_json.map(|value| value.to_string()),
            error: self.error,
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
    }
}

fn unix_now() -> u64 {
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
            }
        })
        .collect()
}

fn node_type_to_str(node_type: &workflow_core::NodeType) -> &'static str {
    match node_type {
        workflow_core::NodeType::Trigger => "trigger",
        workflow_core::NodeType::Http => "http",
        workflow_core::NodeType::Agent => "agent",
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
    }
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
        let executor = InlineExecutorClient::new(store.clone(), std::sync::Arc::new(ApprovalGate::default()));
        let service = ExecutionService::new(store, executor);

        let record = service.start(valid_request()).await.unwrap();
        assert_eq!(record.status, ExecutionStatus::Running);

        let loaded = poll_until_done!(service, "tenant-1", &record.id);
        assert_eq!(loaded.status, ExecutionStatus::Succeeded);
        assert_eq!(loaded.node_results.len(), 2);
        assert_eq!(loaded.node_results[0].node_id, "trigger");
        assert_eq!(loaded.node_results[0].node_type, "trigger");
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
        let service = ExecutionService::new(
            MemoryExecutionStore::default(),
            NoopExecutorClient::default(),
        );
        let mut request = valid_request();
        request.graph.workflow_version_id = "other-version".to_string();

        let err = service.start(request).await.unwrap_err();

        assert_eq!(err, ExecutionError::WorkflowVersionMismatch);
    }

    #[tokio::test]
    async fn rejects_invalid_graph() {
        let service = ExecutionService::new(
            MemoryExecutionStore::default(),
            NoopExecutorClient::default(),
        );
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
        let executor = InlineExecutorClient::new(store.clone(), std::sync::Arc::new(ApprovalGate::default()));
        let service = ExecutionService::new(store, executor);

        let record = service.start(valid_request()).await.unwrap();
        assert!(record.started_at > 0, "started_at should be a unix timestamp");
        assert!(record.finished_at.is_none(), "finished_at should be None when running");

        let done = poll_until_done!(service, "tenant-1", &record.id);
        assert_eq!(done.status, ExecutionStatus::Succeeded);
        assert!(done.finished_at.is_some(), "finished_at should be set after completion");
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
                        id: "agent".to_string(),
                        node_type: NodeType::Agent,
                        config: None,
                    },
                ],
                edges: vec![Edge {
                    source: "trigger".to_string(),
                    target: "agent".to_string(),
                    condition_label: None,
                }],
            },
            input_json: "{\"lead_id\":\"lead-1\"}".to_string(),
        }
    }
}
