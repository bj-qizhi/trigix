use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use agentflow_executor::approval::{ApprovalError as GateError, ApprovalGate};
use execution_core::ExecutionStatus;

use crate::audit::{PlatformAuditStore, action as audit_action};
use crate::auth::{Claims, sign_token, verify_token};
use crate::credentials::{
    CredentialError, CredentialStore, PlatformCredentialStore,
    resolve_credentials_in_json,
};
use crate::execution::{
    ExecutionError, ExecutionRecord, ExecutionService, ExecutionSummary,
    PlatformExecutionStore, PlatformExecutorClient, StartExecutionRequest,
};
use crate::scheduler::{ScheduleEntry, ScheduleStore};
use crate::webhook::{PlatformWebhookStore, WebhookError, WebhookRecord, WebhookStore};
use crate::workflow::{
    ArchiveWorkflowRequest, CreateWorkflowRequest, CreateWorkflowVersionRequest,
    PlatformWorkflowVersionStore, PublishWorkflowVersionRequest, RestoreWorkflowRequest,
    UpdateWorkflowRequest, WorkflowError, WorkflowRecord, WorkflowService, WorkflowVersionRecord,
};

type PlatformService = ExecutionService<PlatformExecutionStore, PlatformExecutorClient>;
type PlatformWorkflowService = WorkflowService<PlatformWorkflowVersionStore>;

#[derive(Clone)]
pub struct AppState {
    execution_service: Arc<PlatformService>,
    workflow_service: Arc<PlatformWorkflowService>,
    webhook_store: Arc<PlatformWebhookStore>,
    approval_gate: Arc<ApprovalGate>,
    credential_store: Arc<PlatformCredentialStore>,
    schedule_store: Arc<ScheduleStore>,
    audit_store: Arc<PlatformAuditStore>,
}

pub fn router() -> Router {
    let state = default_app_state();
    // Spawn the background schedule runner only in production (not in test helpers).
    spawn_schedule_runner(state.clone());
    build_router(state)
}

fn default_app_state() -> AppState {
    let store = PlatformExecutionStore::memory();
    let workflow_store = PlatformWorkflowVersionStore::memory_with_dev_seed();
    let gate = Arc::new(ApprovalGate::default());
    let service = ExecutionService::new(
        store.clone(),
        PlatformExecutorClient::inline_with_gate(store, Arc::clone(&gate)),
    );
    AppState {
        execution_service: Arc::new(service),
        workflow_service: Arc::new(WorkflowService::new(workflow_store)),
        webhook_store: Arc::new(PlatformWebhookStore::default()),
        approval_gate: gate,
        credential_store: Arc::new(PlatformCredentialStore::default()),
        schedule_store: Arc::new(ScheduleStore::default()),
        audit_store: Arc::new(PlatformAuditStore::default()),
    }
}

fn spawn_schedule_runner(state: AppState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            for entry in state.schedule_store.take_due() {
                let Ok(version) = state
                    .workflow_service
                    .get_version(&entry.tenant_id, &entry.workflow_version_id)
                    .await
                else {
                    continue;
                };
                let graph = resolve_graph_credentials(
                    version.graph,
                    &state.credential_store,
                    &entry.tenant_id,
                )
                .await;
                let _ = state
                    .execution_service
                    .start(StartExecutionRequest {
                        tenant_id: entry.tenant_id,
                        workflow_id: entry.workflow_id,
                        workflow_version_id: entry.workflow_version_id,
                        graph,
                        input_json: "{}".to_string(),
                    })
                    .await;
            }
        }
    });
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/executions", get(list_executions).post(start_execution))
        .route("/v1/executions/:execution_id", get(get_execution))
        .route(
            "/v1/executions/:execution_id/approve",
            get(method_not_allowed).post(approve_execution),
        )
        .route(
            "/v1/executions/:execution_id/reject",
            get(method_not_allowed).post(reject_execution),
        )
        .route(
            "/v1/executions/:execution_id/cancel",
            get(method_not_allowed).post(cancel_execution),
        )
        .route(
            "/v1/executions/:execution_id/retry",
            get(method_not_allowed).post(retry_execution),
        )
        .route("/v1/workflows", get(list_workflows).post(create_workflow))
        .route(
            "/v1/workflows/:workflow_id",
            get(get_workflow).patch(update_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/versions",
            get(list_workflow_versions).post(create_workflow_version),
        )
        .route(
            "/v1/workflows/:workflow_id/executions",
            get(method_not_allowed).post(start_execution_from_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/archive",
            get(method_not_allowed).post(archive_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/restore",
            get(method_not_allowed).post(restore_workflow),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id",
            get(get_workflow_version),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id/executions",
            get(method_not_allowed).post(start_execution_from_workflow_version),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id/publish",
            get(method_not_allowed).post(publish_workflow_version),
        )
        .route(
            "/v1/workflow-versions/:workflow_version_id/webhook",
            get(method_not_allowed).post(create_webhook),
        )
        .route(
            "/v1/webhooks/:token",
            get(method_not_allowed).post(trigger_webhook),
        )
        .route("/v1/credentials", get(list_credentials).post(create_credential))
        .route(
            "/v1/credentials/:credential_id",
            get(method_not_allowed).delete(delete_credential),
        )
        .route("/v1/schedules", get(list_schedules))
        .route("/v1/audit-log", get(list_audit_log))
        .route("/v1/workflows/:workflow_id/export", get(export_workflow))
        .route(
            "/v1/workflows/:workflow_id/duplicate",
            get(method_not_allowed).post(duplicate_workflow),
        )
        .route("/v1/workflows/import", get(method_not_allowed).post(import_workflow))
        .route("/v1/auth/token", get(method_not_allowed).post(create_token))
        .with_state(state)
}

pub fn router_with_store(store: PlatformExecutionStore) -> Router {
    let workflow_store = PlatformWorkflowVersionStore::memory_with_dev_seed();
    let gate = Arc::new(ApprovalGate::default());
    let service = ExecutionService::new(
        store.clone(),
        PlatformExecutorClient::inline_with_gate(store, Arc::clone(&gate)),
    );
    router_with_services(
        service,
        WorkflowService::new(workflow_store),
        PlatformWebhookStore::default(),
        gate,
        PlatformCredentialStore::default(),
    )
}

pub fn router_with_store_and_executor(
    store: PlatformExecutionStore,
    workflow_store: PlatformWorkflowVersionStore,
    executor: PlatformExecutorClient,
) -> Router {
    let service = ExecutionService::new(store, executor);
    router_with_services(
        service,
        WorkflowService::new(workflow_store),
        PlatformWebhookStore::default(),
        Arc::new(ApprovalGate::default()),
        PlatformCredentialStore::default(),
    )
}

pub fn router_with_services(
    execution_service: PlatformService,
    workflow_service: PlatformWorkflowService,
    webhook_store: PlatformWebhookStore,
    approval_gate: Arc<ApprovalGate>,
    credential_store: PlatformCredentialStore,
) -> Router {
    let state = AppState {
        execution_service: Arc::new(execution_service),
        workflow_service: Arc::new(workflow_service),
        webhook_store: Arc::new(webhook_store),
        approval_gate,
        credential_store: Arc::new(credential_store),
        schedule_store: Arc::new(ScheduleStore::default()),
        audit_store: Arc::new(PlatformAuditStore::default()),
    };

    build_router(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn start_execution(
    State(state): State<AppState>,
    Json(mut request): Json<StartExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    request.graph = resolve_graph_credentials(
        request.graph,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    request.graph = inject_sub_workflow_graphs(
        request.graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let record = state.execution_service.start(request).await?;
    state.audit_store.record(
        &record.tenant_id, audit_action::EXECUTION_STARTED,
        "execution", &record.id, None,
    );
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn start_execution_from_workflow_version(
    State(state): State<AppState>,
    Path(workflow_version_id): Path<String>,
    Json(request): Json<StartWorkflowVersionExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let workflow_version = state
        .workflow_service
        .get_version(&request.tenant_id, &workflow_version_id)
        .await?;
    let workflow = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_version.workflow_id)
        .await?;
    if workflow.status == "archived" {
        return Err(WorkflowError::ArchivedWorkflow.into());
    }
    let graph = resolve_graph_credentials(
        workflow_version.graph,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: request.tenant_id,
            workflow_id: workflow_version.workflow_id,
            workflow_version_id: workflow_version.id,
            graph,
            input_json: request.input_json,
        })
        .await?;
    state.audit_store.record(
        &record.tenant_id, audit_action::EXECUTION_STARTED,
        "execution", &record.id, None,
    );
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn start_execution_from_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<StartWorkflowExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let workflow = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_id)
        .await?;
    if workflow.status == "archived" {
        return Err(WorkflowError::ArchivedWorkflow.into());
    }
    let workflow_version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let workflow_version = state
        .workflow_service
        .get_version(&request.tenant_id, &workflow_version_id)
        .await?;
    let graph = resolve_graph_credentials(
        workflow_version.graph,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: request.tenant_id,
            workflow_id: workflow.id,
            workflow_version_id: workflow_version.id,
            graph,
            input_json: request.input_json,
        })
        .await?;
    state.audit_store.record(
        &record.tenant_id, audit_action::EXECUTION_STARTED,
        "execution", &record.id, None,
    );
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn resolve_graph_credentials(
    mut graph: workflow_core::WorkflowGraph,
    store: &PlatformCredentialStore,
    tenant_id: &str,
) -> workflow_core::WorkflowGraph {
    for node in &mut graph.nodes {
        if let Some(config) = node.config.take() {
            let (resolved, _) = resolve_credentials_in_json(&config, store, tenant_id).await;
            node.config = Some(resolved);
        }
    }
    graph
}

/// For each SubWorkflow node in the graph, resolve the target workflow's published version graph
/// and inject it as `_graph` in the node config so the executor can run it inline.
async fn inject_sub_workflow_graphs(
    mut graph: workflow_core::WorkflowGraph,
    workflow_service: &PlatformWorkflowService,
    credential_store: &PlatformCredentialStore,
    tenant_id: &str,
) -> workflow_core::WorkflowGraph {
    for node in &mut graph.nodes {
        if node.node_type != workflow_core::NodeType::SubWorkflow {
            continue;
        }
        let config = match node.config.as_mut() {
            Some(c) => c,
            None => continue,
        };
        let workflow_id = match config.get("workflow_id").and_then(|v| v.as_str()).map(str::to_owned) {
            Some(id) => id,
            None => continue,
        };

        let Ok(workflow) = workflow_service.get_workflow(tenant_id, &workflow_id).await else { continue };
        let Some(version_id) = workflow.latest_version_id else { continue };
        let Ok(version) = workflow_service.get_version(tenant_id, &version_id).await else { continue };

        let sub_graph = resolve_graph_credentials(version.graph, credential_store, tenant_id).await;
        let sub_graph_json = match serde_json::to_value(&sub_graph) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(obj) = config.as_object_mut() {
            obj.insert("_graph".to_string(), sub_graph_json);
        }
    }
    graph
}

async fn list_executions(
    State(state): State<AppState>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<Json<Vec<ExecutionSummary>>, ApiError> {
    let records = state.execution_service.list(&query.tenant_id).await?;
    let filtered = if let Some(workflow_id) = &query.workflow_id {
        records.into_iter().filter(|r| &r.workflow_id == workflow_id).collect()
    } else {
        records
    };
    Ok(Json(filtered))
}

async fn get_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<Json<ExecutionRecord>, ApiError> {
    let mut record = state
        .execution_service
        .get(&query.tenant_id, &execution_id)
        .await?;
    if record.status == ExecutionStatus::Running
        && state.approval_gate.is_waiting(&execution_id).await
    {
        record.status = ExecutionStatus::WaitingApproval;
    }
    Ok(Json(record))
}

async fn get_workflow_version(
    State(state): State<AppState>,
    Path(workflow_version_id): Path<String>,
    Query(query): Query<GetWorkflowVersionQuery>,
) -> Result<Json<WorkflowVersionRecord>, ApiError> {
    let record = state
        .workflow_service
        .get_version(&query.tenant_id, &workflow_version_id)
        .await?;
    Ok(Json(record))
}

async fn create_workflow(
    State(state): State<AppState>,
    Json(request): Json<CreateWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    let record = state.workflow_service.create_workflow(request).await?;
    state.audit_store.record(
        &record.tenant_id, audit_action::WORKFLOW_CREATED,
        "workflow", &record.id, None,
    );
    Ok((StatusCode::CREATED, Json(record)))
}

async fn list_workflows(
    State(state): State<AppState>,
    Query(query): Query<ListWorkflowsQuery>,
) -> Result<Json<Vec<WorkflowRecord>>, ApiError> {
    let records = state
        .workflow_service
        .list_workflows(
            &query.tenant_id,
            query.project_id.as_deref(),
            query.status.as_deref(),
            query.limit,
        )
        .await?;
    Ok(Json(records))
}

async fn get_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Query(query): Query<GetWorkflowQuery>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    let record = state
        .workflow_service
        .get_workflow(&query.tenant_id, &workflow_id)
        .await?;
    Ok(Json(record))
}

async fn update_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<UpdateWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    let tenant_id = request.tenant_id.clone();
    let record = state
        .workflow_service
        .update_workflow(&workflow_id, request)
        .await?;
    state.audit_store.record(
        &tenant_id, audit_action::WORKFLOW_UPDATED,
        "workflow", &workflow_id, None,
    );
    Ok(Json(record))
}

async fn archive_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<ArchiveWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    let tenant_id = request.tenant_id.clone();
    let record = state
        .workflow_service
        .archive_workflow(&workflow_id, request)
        .await?;
    // Remove any schedule for this workflow's latest version.
    if let Some(version_id) = &record.latest_version_id {
        if state.schedule_store.unregister(version_id) {
            state.audit_store.record(
                &tenant_id, audit_action::SCHEDULE_REMOVED,
                "workflow_version", version_id, None,
            );
        }
    }
    state.audit_store.record(
        &tenant_id, audit_action::WORKFLOW_ARCHIVED,
        "workflow", &workflow_id, None,
    );
    Ok(Json(record))
}

async fn restore_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<RestoreWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    let tenant_id = request.tenant_id.clone();
    let record = state
        .workflow_service
        .restore_workflow(&workflow_id, request)
        .await?;
    state.audit_store.record(
        &tenant_id, audit_action::WORKFLOW_RESTORED,
        "workflow", &workflow_id, None,
    );
    Ok(Json(record))
}

async fn create_workflow_version(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<CreateWorkflowVersionRequest>,
) -> Result<(StatusCode, Json<WorkflowVersionRecord>), ApiError> {
    let record = state
        .workflow_service
        .create_version(&workflow_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn publish_workflow_version(
    State(state): State<AppState>,
    Path(workflow_version_id): Path<String>,
    Json(request): Json<PublishWorkflowVersionRequest>,
) -> Result<Json<WorkflowVersionRecord>, ApiError> {
    let record = state
        .workflow_service
        .publish_version(&request.tenant_id, &workflow_version_id)
        .await?;

    // If the trigger node has an interval_secs config, register with the scheduler.
    let _ = state.schedule_store.unregister(&workflow_version_id);
    if let Some(interval_secs) = extract_trigger_interval(&record.graph) {
        state.schedule_store.register(ScheduleEntry {
            workflow_id: record.workflow_id.clone(),
            workflow_version_id: record.id.clone(),
            tenant_id: request.tenant_id.clone(),
            interval_secs,
            next_run_at: std::time::Instant::now()
                + std::time::Duration::from_secs(interval_secs),
        });
        state.audit_store.record(
            &request.tenant_id, audit_action::SCHEDULE_REGISTERED,
            "workflow_version", &record.id, None,
        );
    }
    state.audit_store.record(
        &request.tenant_id, audit_action::WORKFLOW_PUBLISHED,
        "workflow_version", &record.id, None,
    );

    Ok(Json(record))
}

async fn list_workflow_versions(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Query(query): Query<ListWorkflowVersionsQuery>,
) -> Result<Json<Vec<WorkflowVersionRecord>>, ApiError> {
    let records = state
        .workflow_service
        .list_versions(
            &query.tenant_id,
            &workflow_id,
            query.status.as_deref(),
            query.limit,
        )
        .await?;
    Ok(Json(records))
}

async fn approve_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(body): Json<ApprovalBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .approval_gate
        .resolve(&execution_id, true)
        .await
        .map_err(|e| match e {
            GateError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "NoApprovalPending".to_string(),
            },
        })?;
    state.audit_store.record(
        body.tenant_id.as_deref().unwrap_or(""),
        audit_action::EXECUTION_APPROVED,
        "execution", &execution_id, None,
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn reject_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(body): Json<ApprovalBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .approval_gate
        .resolve(&execution_id, false)
        .await
        .map_err(|e| match e {
            GateError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "NoApprovalPending".to_string(),
            },
        })?;
    state.audit_store.record(
        body.tenant_id.as_deref().unwrap_or(""),
        audit_action::EXECUTION_REJECTED,
        "execution", &execution_id, None,
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn cancel_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(body): Json<CancelExecutionBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .execution_service
        .cancel(&body.tenant_id, &execution_id)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "ExecutionNotFound".to_string(),
        })?;
    state.audit_store.record(
        &body.tenant_id,
        audit_action::EXECUTION_CANCELLED,
        "execution", &execution_id, None,
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn retry_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(body): Json<RetryExecutionBody>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let original = state
        .execution_service
        .get(&body.tenant_id, &execution_id)
        .await?;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: original.tenant_id.clone(),
            workflow_id: original.workflow_id.clone(),
            workflow_version_id: original.workflow_version_id.clone(),
            graph: original.graph.clone(),
            input_json: original.input_json.clone(),
        })
        .await?;
    state.audit_store.record(
        &body.tenant_id,
        audit_action::EXECUTION_RETRIED,
        "execution", &record.id,
        Some(serde_json::json!({ "retried_from": execution_id })),
    );
    Ok((StatusCode::CREATED, Json(record)))
}

async fn create_webhook(
    State(state): State<AppState>,
    Path(workflow_version_id): Path<String>,
    Json(body): Json<CreateWebhookBody>,
) -> Result<Json<WebhookResponse>, ApiError> {
    match state.webhook_store.get_by_version(&workflow_version_id).await {
        Ok(Some(existing)) => {
            return Ok(Json(WebhookResponse {
                url: format!("/v1/webhooks/{}", existing.token),
                token: existing.token,
            }));
        }
        Ok(None) => {}
        Err(e) => return Err(e.into()),
    }

    let version = state
        .workflow_service
        .get_version(&body.tenant_id, &workflow_version_id)
        .await?;

    let token = uuid::Uuid::new_v4().to_string().replace('-', "");
    let record = state
        .webhook_store
        .upsert(WebhookRecord {
            token,
            tenant_id: body.tenant_id,
            workflow_id: version.workflow_id,
            workflow_version_id,
        })
        .await
        .map_err(ApiError::from)?;

    Ok(Json(WebhookResponse {
        url: format!("/v1/webhooks/{}", record.token),
        token: record.token,
    }))
}

async fn trigger_webhook(
    State(state): State<AppState>,
    Path(token): Path<String>,
    body: Bytes,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let webhook = state
        .webhook_store
        .get_by_token(&token)
        .await
        .map_err(ApiError::from)?;

    let version = state
        .workflow_service
        .get_version(&webhook.tenant_id, &webhook.workflow_version_id)
        .await?;

    let input_json = if body.is_empty() {
        "{}".to_string()
    } else {
        String::from_utf8(body.to_vec()).unwrap_or_else(|_| "{}".to_string())
    };

    let graph = resolve_graph_credentials(
        version.graph,
        &state.credential_store,
        &webhook.tenant_id,
    )
    .await;

    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: webhook.tenant_id,
            workflow_id: webhook.workflow_id,
            workflow_version_id: webhook.workflow_version_id,
            graph,
            input_json,
        })
        .await?;
    state.audit_store.record(
        &record.tenant_id, audit_action::EXECUTION_STARTED,
        "execution", &record.id, None,
    );
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn list_schedules(
    State(state): State<AppState>,
    Query(query): Query<CredentialQuery>,
) -> Json<Vec<crate::scheduler::ScheduleSummary>> {
    Json(state.schedule_store.list(&query.tenant_id))
}

fn extract_trigger_interval(graph: &workflow_core::WorkflowGraph) -> Option<u64> {
    graph
        .nodes
        .iter()
        .find(|n| n.node_type == workflow_core::NodeType::Trigger)
        .and_then(|n| n.config.as_ref())
        .and_then(|c| c.get("interval_secs"))
        .and_then(|v| v.as_u64())
        .filter(|&secs| secs >= 60)
}

async fn list_credentials(
    State(state): State<AppState>,
    Query(query): Query<CredentialQuery>,
) -> Result<Json<Vec<crate::credentials::CredentialSummary>>, ApiError> {
    let list = state.credential_store.list(&query.tenant_id).await?;
    Ok(Json(list))
}

async fn create_credential(
    State(state): State<AppState>,
    Json(body): Json<CreateCredentialBody>,
) -> Result<(StatusCode, Json<crate::credentials::CredentialSummary>), ApiError> {
    let summary = state
        .credential_store
        .create(&body.tenant_id, &body.name, &body.value)
        .await?;
    state.audit_store.record(
        &body.tenant_id, audit_action::CREDENTIAL_CREATED,
        "credential", &summary.id, None,
    );
    Ok((StatusCode::CREATED, Json(summary)))
}

async fn delete_credential(
    State(state): State<AppState>,
    Path(credential_id): Path<String>,
    Query(query): Query<CredentialQuery>,
) -> Result<StatusCode, ApiError> {
    state.credential_store.delete(&query.tenant_id, &credential_id).await?;
    state.audit_store.record(
        &query.tenant_id, audit_action::CREDENTIAL_DELETED,
        "credential", &credential_id, None,
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn export_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Query(query): Query<ExportWorkflowQuery>,
) -> Result<Json<WorkflowExport>, ApiError> {
    let workflow = state
        .workflow_service
        .get_workflow(&query.tenant_id, &workflow_id)
        .await?;
    let version_id = workflow.latest_version_id.ok_or(WorkflowError::NoPublishedVersion)?;
    let version = state
        .workflow_service
        .get_version(&query.tenant_id, &version_id)
        .await?;
    let exported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(Json(WorkflowExport {
        name: workflow.name,
        graph: version.graph,
        exported_at,
    }))
}

async fn import_workflow(
    State(state): State<AppState>,
    Json(body): Json<ImportWorkflowBody>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    let name = body.name.filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Imported Workflow".to_string());
    let workflow = state
        .workflow_service
        .create_workflow(CreateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            workspace_id: body.workspace_id,
            project_id: body.project_id,
            name,
        })
        .await?;
    state
        .workflow_service
        .create_version(
            &workflow.id,
            CreateWorkflowVersionRequest {
                tenant_id: body.tenant_id.clone(),
                graph: body.graph,
                status: None,
            },
        )
        .await?;
    state.audit_store.record(
        &body.tenant_id,
        audit_action::WORKFLOW_CREATED,
        "workflow",
        &workflow.id,
        None,
    );
    Ok((StatusCode::CREATED, Json(workflow)))
}

async fn duplicate_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(body): Json<DuplicateWorkflowBody>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    let original = state
        .workflow_service
        .get_workflow(&body.tenant_id, &workflow_id)
        .await?;
    let new_name = format!("{} (copy)", original.name);
    let new_workflow = state
        .workflow_service
        .create_workflow(CreateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            workspace_id: original.workspace_id,
            project_id: original.project_id,
            name: new_name,
        })
        .await?;
    if let Some(version_id) = &original.latest_version_id {
        let version = state
            .workflow_service
            .get_version(&body.tenant_id, version_id)
            .await?;
        state
            .workflow_service
            .create_version(
                &new_workflow.id,
                CreateWorkflowVersionRequest {
                    tenant_id: body.tenant_id.clone(),
                    graph: version.graph,
                    status: None,
                },
            )
            .await?;
    }
    state.audit_store.record(
        &body.tenant_id,
        audit_action::WORKFLOW_DUPLICATED,
        "workflow",
        &new_workflow.id,
        Some(serde_json::json!({ "duplicated_from": workflow_id })),
    );
    Ok((StatusCode::CREATED, Json(new_workflow)))
}

async fn list_audit_log(
    State(state): State<AppState>,
    Query(query): Query<AuditLogQuery>,
) -> Json<Vec<crate::audit::AuditEvent>> {
    let limit = query.limit.unwrap_or(100).min(1000);
    Json(state.audit_store.list(&query.tenant_id, limit))
}

async fn method_not_allowed() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

#[derive(Debug, Deserialize)]
struct TokenRequest {
    api_key: String,
    tenant_id: Option<String>,
    workspace_id: Option<String>,
    project_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    token: String,
    tenant_id: String,
    workspace_id: String,
    project_id: String,
}

async fn create_token(Json(body): Json<TokenRequest>) -> Result<Json<TokenResponse>, ApiError> {
    let expected_key = std::env::var("DEV_API_KEY").unwrap_or_else(|_| "dev".to_string());
    if body.api_key != expected_key {
        return Err(ApiError { status: StatusCode::UNAUTHORIZED, message: "Invalid api_key".to_string() });
    }
    let tenant_id = body.tenant_id.unwrap_or_else(|| "tenant-1".to_string());
    let workspace_id = body.workspace_id.unwrap_or_else(|| "workspace-1".to_string());
    let project_id = body.project_id.unwrap_or_else(|| "project-1".to_string());
    // 7-day expiry
    let exp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 3600;
    let claims = Claims {
        sub: tenant_id.clone(),
        tenant_id: tenant_id.clone(),
        workspace_id: workspace_id.clone(),
        project_id: project_id.clone(),
        exp,
    };
    let token = sign_token(&claims).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Failed to sign token".to_string(),
    })?;
    Ok(Json(TokenResponse { token, tenant_id, workspace_id, project_id }))
}

pub fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").map(|s| s.to_string())
}

/// Decode the Authorization header and return claims, or None if no/invalid token.
/// When AUTH_REQUIRED=true in the environment, callers should reject requests without claims.
pub fn extract_claims(headers: &axum::http::HeaderMap) -> Option<Claims> {
    let token = extract_bearer(headers)?;
    verify_token(&token)
}

#[derive(Debug, Deserialize)]
struct GetExecutionQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct GetWorkflowVersionQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct ListWorkflowsQuery {
    tenant_id: String,
    project_id: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GetWorkflowQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct ListWorkflowVersionsQuery {
    tenant_id: String,
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ListExecutionsQuery {
    tenant_id: String,
    workflow_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StartWorkflowVersionExecutionRequest {
    tenant_id: String,
    input_json: String,
}

#[derive(Debug, Deserialize)]
struct StartWorkflowExecutionRequest {
    tenant_id: String,
    input_json: String,
}

#[derive(Debug, Deserialize)]
struct CredentialQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct AuditLogQuery {
    tenant_id: String,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ExportWorkflowQuery {
    tenant_id: String,
}

#[derive(Debug, Serialize)]
struct WorkflowExport {
    name: String,
    graph: workflow_core::WorkflowGraph,
    exported_at: u64,
}

#[derive(Debug, Deserialize)]
struct DuplicateWorkflowBody {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct ImportWorkflowBody {
    tenant_id: String,
    workspace_id: String,
    project_id: String,
    name: Option<String>,
    graph: workflow_core::WorkflowGraph,
}

#[derive(Debug, Deserialize)]
struct CreateCredentialBody {
    tenant_id: String,
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct ApprovalBody {
    #[allow(dead_code)]
    tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CancelExecutionBody {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct RetryExecutionBody {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateWebhookBody {
    tenant_id: String,
}

#[derive(Debug, Serialize)]
struct WebhookResponse {
    token: String,
    url: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<ExecutionError> for ApiError {
    fn from(error: ExecutionError) -> Self {
        let status = match error {
            ExecutionError::NotFound => StatusCode::NOT_FOUND,
            ExecutionError::StoreUnavailable | ExecutionError::ExecutorUnavailable => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            ExecutionError::MissingTenant
            | ExecutionError::MissingWorkflow
            | ExecutionError::MissingWorkflowVersion
            | ExecutionError::WorkflowVersionMismatch
            | ExecutionError::InvalidGraph
            | ExecutionError::InvalidInput => StatusCode::BAD_REQUEST,
        };

        Self {
            status,
            message: format!("{error:?}"),
        }
    }
}

impl From<CredentialError> for ApiError {
    fn from(e: CredentialError) -> Self {
        let (status, message) = match e {
            CredentialError::NotFound => (StatusCode::NOT_FOUND, "CredentialNotFound"),
            CredentialError::NameTaken => (StatusCode::CONFLICT, "CredentialNameTaken"),
            CredentialError::StoreUnavailable => (StatusCode::INTERNAL_SERVER_ERROR, "StoreUnavailable"),
        };
        Self { status, message: message.to_string() }
    }
}

impl From<WebhookError> for ApiError {
    fn from(e: WebhookError) -> Self {
        let (status, message) = match e {
            WebhookError::NotFound => (StatusCode::NOT_FOUND, "WebhookNotFound"),
            WebhookError::StoreUnavailable => (StatusCode::INTERNAL_SERVER_ERROR, "StoreUnavailable"),
        };
        Self { status, message: message.to_string() }
    }
}

impl From<WorkflowError> for ApiError {
    fn from(error: WorkflowError) -> Self {
        let status = match error {
            WorkflowError::NotFound => StatusCode::NOT_FOUND,
            WorkflowError::StoreUnavailable => StatusCode::INTERNAL_SERVER_ERROR,
            WorkflowError::MissingTenant
            | WorkflowError::MissingWorkspace
            | WorkflowError::MissingProject
            | WorkflowError::MissingWorkflow
            | WorkflowError::MissingName
            | WorkflowError::MissingWorkflowVersion
            | WorkflowError::InvalidGraph
            | WorkflowError::InvalidStatus
            | WorkflowError::InvalidLimit
            | WorkflowError::ArchivedWorkflow
            | WorkflowError::NoPublishedVersion => StatusCode::BAD_REQUEST,
        };

        Self {
            status,
            message: format!("{error:?}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn starts_and_gets_execution_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1",
            "workflow_id": "workflow-1",
            "workflow_version_id": "version-1",
            "graph": {
                "workflow_version_id": "version-1",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "agent", "type": "agent"}
                ],
                "edges": [
                    {"source": "trigger", "target": "agent"}
                ]
            },
            "input_json": "{\"lead_id\":\"lead-1\"}"
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["tenant_id"], "tenant-1");
        assert_eq!(payload["workflow_id"], "workflow-1");
        assert_eq!(payload["workflow_version_id"], "version-1");
        assert_eq!(payload["status"], "running");

        let execution_id = payload["id"].as_str().unwrap().to_string();

        // Wait for background execution to complete
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/executions?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload.as_array().unwrap().len(), 1);
        assert_eq!(payload[0]["id"], execution_id);
        assert_eq!(payload[0]["status"], "succeeded");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["id"], execution_id);
        assert_eq!(payload["status"], "succeeded");
        assert_eq!(payload["node_results"][0]["node_id"], "trigger");
        assert_eq!(payload["node_results"][0]["node_type"], "trigger");
        assert_eq!(payload["node_results"][0]["status"], "succeeded");
    }

    #[tokio::test]
    async fn creates_and_lists_workflows_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1",
            "workspace_id": "workspace-1",
            "project_id": "project-1",
            "name": "New Workflow"
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let workflow_id = payload["id"].as_str().unwrap();

        assert_eq!(payload["name"], "New Workflow");
        assert_eq!(payload["status"], "draft");
        assert!(payload["latest_version_id"].is_null());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows?tenant_id=tenant-1&project_id=project-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(payload
            .as_array()
            .unwrap()
            .iter()
            .any(|workflow| workflow["id"] == workflow_id));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows?tenant_id=tenant-1&project_id=project-1&status=draft")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.as_array().unwrap().len(), 1);
        assert_eq!(payload[0]["id"], workflow_id);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows?tenant_id=tenant-1&status=deleted")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn gets_workflow_over_http() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows/workflow-1?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["id"], "workflow-1");
        assert_eq!(payload["name"], "Dev Lead Workflow");
        assert_eq!(payload["status"], "published");
        assert_eq!(payload["latest_version_id"], "version-1");
    }

    #[tokio::test]
    async fn updates_workflow_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1",
            "name": "Renamed Workflow"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/workflows/workflow-1")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["id"], "workflow-1");
        assert_eq!(payload["name"], "Renamed Workflow");
        assert_eq!(payload["status"], "published");
        assert_eq!(payload["latest_version_id"], "version-1");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows/workflow-1?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["name"], "Renamed Workflow");
    }

    #[tokio::test]
    async fn archives_workflow_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/archive")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["id"], "workflow-1");
        assert_eq!(payload["status"], "archived");
        assert_eq!(payload["latest_version_id"], "version-1");

        let run_body = json!({
            "tenant_id": "tenant-1",
            "input_json": "{\"lead_id\":\"lead-from-archived-workflow\"}"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(run_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let version_run_body = json!({
            "tenant_id": "tenant-1",
            "input_json": "{\"lead_id\":\"lead-from-archived-version\"}"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflow-versions/version-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(version_run_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let restore_body = json!({
            "tenant_id": "tenant-1"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/restore")
                    .header("content-type", "application/json")
                    .body(Body::from(restore_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["id"], "workflow-1");
        assert_eq!(payload["status"], "published");
        assert_eq!(payload["latest_version_id"], "version-1");

        let run_body = json!({
            "tenant_id": "tenant-1",
            "input_json": "{\"lead_id\":\"lead-from-restored-workflow\"}"
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(run_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn starts_execution_from_workflow_version_over_http() {
        let app = router();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflow-versions/version-1?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["id"], "version-1");
        assert_eq!(payload["workflow_id"], "workflow-1");
        assert_eq!(payload["graph"]["nodes"].as_array().unwrap().len(), 2);

        let request_body = json!({
            "tenant_id": "tenant-1",
            "input_json": "{\"lead_id\":\"lead-from-version\"}"
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflow-versions/version-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["tenant_id"], "tenant-1");
        assert_eq!(payload["workflow_id"], "workflow-1");
        assert_eq!(payload["workflow_version_id"], "version-1");
        assert_eq!(payload["status"], "running");
    }

    #[tokio::test]
    async fn starts_execution_from_latest_workflow_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1",
            "input_json": "{\"lead_id\":\"lead-from-workflow\"}"
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["tenant_id"], "tenant-1");
        assert_eq!(payload["workflow_id"], "workflow-1");
        assert_eq!(payload["workflow_version_id"], "version-1");
        assert_eq!(payload["status"], "running");
    }

    #[tokio::test]
    async fn creates_workflow_version_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1",
            "graph": {
                "workflow_version_id": "client-supplied-id",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "agent", "type": "agent"}
                ],
                "edges": [
                    {"source": "trigger", "target": "agent"}
                ]
            }
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/versions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let workflow_version_id = payload["id"].as_str().unwrap();

        assert_ne!(workflow_version_id, "client-supplied-id");
        assert_eq!(payload["workflow_id"], "workflow-1");
        assert_eq!(payload["version"], 2);
        assert_eq!(payload["status"], "draft");
        assert_eq!(payload["graph"]["workflow_version_id"], workflow_version_id);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/workflow-versions/{workflow_version_id}?tenant_id=tenant-1"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn lists_workflow_versions_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1",
            "graph": {
                "workflow_version_id": "client-supplied-id",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "agent", "type": "agent"}
                ],
                "edges": [
                    {"source": "trigger", "target": "agent"}
                ]
            }
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/versions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows/workflow-1/versions?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload.as_array().unwrap().len(), 2);
        assert_eq!(payload[0]["version"], 2);
        assert_eq!(payload[1]["version"], 1);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows/workflow-1/versions?tenant_id=tenant-1&status=draft")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.as_array().unwrap().len(), 1);
        assert_eq!(payload[0]["version"], 2);
        assert_eq!(payload[0]["status"], "draft");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows/workflow-1/versions?tenant_id=tenant-1&status=archived")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn publishes_workflow_version_over_http() {
        let app = router();
        let workflow_body = json!({
            "tenant_id": "tenant-1",
            "workspace_id": "workspace-1",
            "project_id": "project-1",
            "name": "New Workflow"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(workflow_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let workflow: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let workflow_id = workflow["id"].as_str().unwrap();

        let version_body = json!({
            "tenant_id": "tenant-1",
            "graph": {
                "workflow_version_id": "client-supplied-id",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "agent", "type": "agent"}
                ],
                "edges": [
                    {"source": "trigger", "target": "agent"}
                ]
            }
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_id}/versions"))
                    .header("content-type", "application/json")
                    .body(Body::from(version_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let version: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let workflow_version_id = version["id"].as_str().unwrap();

        let publish_body = json!({
            "tenant_id": "tenant-1"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/workflow-versions/{workflow_version_id}/publish"
                    ))
                    .header("content-type", "application/json")
                    .body(Body::from(publish_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["id"], workflow_version_id);
        assert_eq!(payload["status"], "published");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows?tenant_id=tenant-1&project_id=project-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let workflow = payload
            .as_array()
            .unwrap()
            .iter()
            .find(|workflow| workflow["id"] == workflow_id)
            .unwrap();

        assert_eq!(workflow["status"], "published");
        assert_eq!(workflow["latest_version_id"], workflow_version_id);
    }

    #[tokio::test]
    async fn approval_node_waits_and_resumes_on_approve() {
        let app = router();

        let request_body = json!({
            "tenant_id": "tenant-1",
            "workflow_id": "workflow-1",
            "workflow_version_id": "version-a",
            "graph": {
                "workflow_version_id": "version-a",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "approve", "type": "approval"}
                ],
                "edges": [{"source": "trigger", "target": "approve"}]
            },
            "input_json": "{}"
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "running");
        let execution_id = payload["id"].as_str().unwrap().to_string();

        // Give the executor time to reach the approval node
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Status should be waiting_approval
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "waiting_approval");

        // Approve
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/executions/{execution_id}/approve"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id": "tenant-1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Wait for the execution to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "succeeded");
    }

    #[tokio::test]
    async fn approval_node_fails_on_reject() {
        let app = router();

        let request_body = json!({
            "tenant_id": "tenant-1",
            "workflow_id": "workflow-1",
            "workflow_version_id": "version-b",
            "graph": {
                "workflow_version_id": "version-b",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "approve", "type": "approval"}
                ],
                "edges": [{"source": "trigger", "target": "approve"}]
            },
            "input_json": "{}"
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let execution_id = payload["id"].as_str().unwrap().to_string();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Reject
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/executions/{execution_id}/reject"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "failed");
    }

    #[tokio::test]
    async fn approve_returns_404_when_no_pending_approval() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions/no-such-execution/approve")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn creates_and_triggers_webhook_over_http() {
        let app = router();

        // Create webhook for the dev-seeded version-1
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflow-versions/version-1/webhook")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id": "tenant-1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let webhook: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let token = webhook["token"].as_str().unwrap();
        assert!(!token.is_empty());
        assert_eq!(webhook["url"], format!("/v1/webhooks/{token}"));

        // Idempotent: same call returns the same token
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflow-versions/version-1/webhook")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id": "tenant-1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let webhook2: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(webhook2["token"], webhook["token"]);

        // Trigger the webhook with a JSON body
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/webhooks/{token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"source": "crm", "lead_id": "lead-99"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let execution: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(execution["tenant_id"], "tenant-1");
        assert_eq!(execution["workflow_id"], "workflow-1");
        assert_eq!(execution["status"], "running");

        // Unknown token → 404
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/not-a-real-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_invalid_graph_over_http() {
        let app = router();
        let request_body = json!({
            "tenant_id": "tenant-1",
            "workflow_id": "workflow-1",
            "workflow_version_id": "version-1",
            "graph": {
                "workflow_version_id": "version-1",
                "nodes": [
                    {"id": "a", "type": "http"},
                    {"id": "b", "type": "agent"}
                ],
                "edges": [
                    {"source": "a", "target": "b"},
                    {"source": "b", "target": "a"}
                ]
            },
            "input_json": "{}"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn creates_lists_and_deletes_credentials_over_http() {
        let app = router();

        // List empty
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/credentials?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 0);

        // Create
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/credentials")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id": "tenant-1", "name": "my-api-key", "value": "sk-secret"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let cred: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(cred["name"], "my-api-key");
        assert!(cred.get("value").is_none(), "value must not be returned");
        let cred_id = cred["id"].as_str().unwrap().to_string();

        // List shows one
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/credentials?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);

        // Delete
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/credentials/{cred_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // List empty again
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/credentials?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn credential_reference_resolved_before_execution() {
        let cred_store = PlatformCredentialStore::default();
        cred_store
            .create("tenant-1", "my-token", "Bearer secret-abc")
            .await
            .unwrap();

        let store = PlatformExecutionStore::memory();
        let gate = Arc::new(ApprovalGate::default());
        let service = ExecutionService::new(
            store.clone(),
            PlatformExecutorClient::inline_with_gate(store, Arc::clone(&gate)),
        );
        let workflow_service =
            WorkflowService::new(PlatformWorkflowVersionStore::memory_with_dev_seed());
        let app = router_with_services(
            service,
            workflow_service,
            PlatformWebhookStore::default(),
            gate,
            cred_store,
        );

        // Start execution with a graph whose node config has a credential reference.
        let request_body = json!({
            "tenant_id": "tenant-1",
            "workflow_id": "workflow-1",
            "workflow_version_id": "version-cred",
            "graph": {
                "workflow_version_id": "version-cred",
                "nodes": [{
                    "id": "trigger", "type": "trigger",
                    "config": {"auth": "{{credential.my-token}}"}
                }],
                "edges": []
            },
            "input_json": "{}"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // The graph stored in the execution record should have the resolved value.
        assert_eq!(payload["graph"]["nodes"][0]["config"]["auth"], "Bearer secret-abc");
    }

    #[tokio::test]
    async fn publishing_with_schedule_trigger_registers_schedule() {
        let app = router();

        // Create a new workflow
        let wf_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id":"tenant-1","workspace_id":"ws-1","project_id":"proj-1","name":"Scheduled WF"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(wf_response.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let wf_id = wf["id"].as_str().unwrap();

        // Create a version with interval_secs on the trigger node
        let ver_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/versions"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "tenant-1",
                            "graph": {
                                "workflow_version_id": "temp",
                                "nodes": [{"id":"trigger","type":"trigger","config":{"interval_secs":3600}}],
                                "edges": []
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(ver_response.into_body(), usize::MAX).await.unwrap();
        let ver: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let ver_id = ver["id"].as_str().unwrap();

        // Initially no schedules
        let sched_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/schedules?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(sched_response.into_body(), usize::MAX).await.unwrap();
        let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(schedules.as_array().unwrap().len(), 0);

        // Publish the version
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflow-versions/{ver_id}/publish"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id":"tenant-1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Schedule should now be registered
        let sched_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/schedules?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(sched_response.into_body(), usize::MAX).await.unwrap();
        let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let list = schedules.as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["workflow_id"], wf_id);
        assert_eq!(list[0]["interval_secs"], 3600);
    }

    #[tokio::test]
    async fn audit_log_records_execution_started() {
        let app = router();

        let request_body = json!({
            "tenant_id": "tenant-audit",
            "workflow_id": "workflow-1",
            "workflow_version_id": "version-1",
            "graph": {
                "workflow_version_id": "version-1",
                "nodes": [{"id": "trigger", "type": "trigger"}],
                "edges": []
            },
            "input_json": "{}"
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/audit-log?tenant_id=tenant-audit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let events: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let list = events.as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["action"], "execution.started");
        assert_eq!(list[0]["tenant_id"], "tenant-audit");
        assert_eq!(list[0]["resource_type"], "execution");
    }

    #[tokio::test]
    async fn exports_workflow_graph_over_http() {
        let app = router();

        // Export the dev-seeded workflow-1 (has published version-1)
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows/workflow-1/export?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let export: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(export["name"], "Dev Lead Workflow");
        assert!(export["graph"]["nodes"].as_array().unwrap().len() >= 1);
        assert!(export["exported_at"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn imports_workflow_from_json_over_http() {
        let app = router();

        let body = json!({
            "tenant_id": "tenant-1",
            "workspace_id": "workspace-1",
            "project_id": "project-1",
            "name": "Imported Copy",
            "graph": {
                "workflow_version_id": "ignored-id",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "agent",   "type": "agent"}
                ],
                "edges": [{"source": "trigger", "target": "agent"}]
            }
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/import")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let workflow: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(workflow["name"], "Imported Copy");
        assert_eq!(workflow["status"], "draft");
        assert!(workflow["latest_version_id"].is_null());

        // A draft version should have been created with the imported graph
        let wf_id = workflow["id"].as_str().unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/workflows/{wf_id}/versions?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let versions: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let list = versions.as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["status"], "draft");
        assert_eq!(list[0]["graph"]["nodes"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn archiving_workflow_removes_schedule() {
        let app = router();

        // Use the dev-seeded workflow-1 / version-1 — first add a schedule manually via publish.
        // Create version with schedule
        let ver_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/versions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "tenant-1",
                            "graph": {
                                "workflow_version_id": "temp",
                                "nodes": [{"id":"trigger","type":"trigger","config":{"interval_secs":60}}],
                                "edges": []
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(ver_response.into_body(), usize::MAX).await.unwrap();
        let ver: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let ver_id = ver["id"].as_str().unwrap();

        // Publish to register schedule
        app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflow-versions/{ver_id}/publish"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id":"tenant-1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Verify schedule registered
        let sched_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/schedules?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(sched_response.into_body(), usize::MAX).await.unwrap();
        let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(schedules.as_array().unwrap().len(), 1);

        // Archive the workflow
        app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/archive")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id":"tenant-1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Schedule should be gone
        let sched_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/schedules?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(sched_response.into_body(), usize::MAX).await.unwrap();
        let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(schedules.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn retry_execution_creates_new_execution() {
        let store = PlatformExecutionStore::memory();
        let service = ExecutionService::new(store.clone(), PlatformExecutorClient::noop());
        let workflow_service =
            WorkflowService::new(PlatformWorkflowVersionStore::memory_with_dev_seed());
        let gate = Arc::new(ApprovalGate::default());
        let app = router_with_services(
            service,
            workflow_service,
            PlatformWebhookStore::default(),
            gate,
            PlatformCredentialStore::default(),
        );

        // Start original execution
        let exec_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "tenant_id": "tenant-1", "input_json": "{\"key\":\"val\"}" })
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(exec_response.into_body(), usize::MAX).await.unwrap();
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let original_id = original["id"].as_str().unwrap().to_string();

        // Retry it
        let retry_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/executions/{original_id}/retry"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "tenant_id": "tenant-1" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(retry_response.status(), StatusCode::CREATED);
        let bytes = to_bytes(retry_response.into_body(), usize::MAX).await.unwrap();
        let retried: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        // New execution has a different ID
        assert_ne!(retried["id"], original["id"]);
        // Same workflow and input
        assert_eq!(retried["workflow_id"], original["workflow_id"]);
        assert_eq!(retried["input_json"], original["input_json"]);
        assert_eq!(retried["status"], "running");
    }

    #[tokio::test]
    async fn cancel_execution_over_http() {
        // Noop executor leaves execution in Running state so we can cancel it.
        let store = PlatformExecutionStore::memory();
        let service = ExecutionService::new(store.clone(), PlatformExecutorClient::noop());
        let workflow_service =
            WorkflowService::new(PlatformWorkflowVersionStore::memory_with_dev_seed());
        let gate = Arc::new(ApprovalGate::default());
        let app = router_with_services(
            service,
            workflow_service,
            PlatformWebhookStore::default(),
            gate,
            PlatformCredentialStore::default(),
        );

        // Start an execution
        let exec_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "tenant_id": "tenant-1", "input_json": "{}" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(exec_response.status().is_success());
        let bytes = to_bytes(exec_response.into_body(), usize::MAX).await.unwrap();
        let exec: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let exec_id = exec["id"].as_str().unwrap().to_string();

        // Cancel it
        let cancel_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/executions/{exec_id}/cancel"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "tenant_id": "tenant-1" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cancel_response.status(), StatusCode::OK);

        // Verify status is cancelled
        let get_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/executions/{exec_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(get_response.into_body(), usize::MAX).await.unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(updated["status"], "cancelled");
        assert!(updated["finished_at"].is_number());
    }

    #[tokio::test]
    async fn duplicate_workflow_creates_copy() {
        let app = router();

        // Create a workflow to duplicate
        let create_body = json!({
            "tenant_id": "tenant-1",
            "workspace_id": "workspace-1",
            "project_id": "project-1",
            "name": "Original Workflow"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(create_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let original: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let original_id = original["id"].as_str().unwrap().to_string();

        // Duplicate it
        let dup_body = json!({ "tenant_id": "tenant-1" });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{original_id}/duplicate"))
                    .header("content-type", "application/json")
                    .body(Body::from(dup_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let copy: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_ne!(copy["id"], original["id"]);
        assert_eq!(copy["name"], "Original Workflow (copy)");
        assert_eq!(copy["status"], "draft");
        assert_eq!(copy["workspace_id"], original["workspace_id"]);
        assert_eq!(copy["project_id"], original["project_id"]);
    }

    #[tokio::test]
    async fn create_token_returns_jwt_for_valid_key() {
        std::env::set_var("DEV_API_KEY", "test-key-29");
        let app = router();
        let body = serde_json::json!({ "api_key": "test-key-29" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/token")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(resp["token"].as_str().map(|t| t.len() > 20).unwrap_or(false));
        assert_eq!(resp["tenant_id"], "tenant-1");
        assert_eq!(resp["workspace_id"], "workspace-1");
    }

    #[tokio::test]
    async fn create_token_rejects_wrong_key() {
        std::env::set_var("DEV_API_KEY", "correct-key");
        let app = router();
        let body = serde_json::json!({ "api_key": "wrong-key" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/token")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
