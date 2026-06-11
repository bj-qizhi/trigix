// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn start_execution_from_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Json(mut request): Json<StartWorkflowVersionExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    state
        .billing_store
        .check_execution_quota(&request.tenant_id)
        .map_err(|e| ApiError {
            status: StatusCode::PAYMENT_REQUIRED,
            message: e,
        })?;
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
    if workflow_version.status == "draft" {
        return Err(WorkflowError::DraftVersion.into());
    }
    let env_set = request.env_set.as_deref().unwrap_or(DEFAULT_SET);
    let graph = resolve_graph_credentials(
        workflow_version.graph,
        &state.credential_store,
        &state.env_store,
        &request.tenant_id,
        env_set,
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
            label: request.label,
            callback_url: request.callback_url,
            trigger_type: Some("manual".to_string()),
            dry_run: request.dry_run.unwrap_or(false),
            retried_from: None,
        })
        .await?;
    let prev_used = state
        .billing_store
        .billing_status(&record.tenant_id)
        .usage
        .executions_used;
    state.billing_store.increment_execution(&record.tenant_id);
    spawn_quota_alert(&state, &record.tenant_id, prev_used);
    state.audit_store.record(
        &record.tenant_id,
        audit_action::EXECUTION_STARTED,
        "execution",
        &record.id,
        None,
    );
    // Fire user notification on failure/success based on workflow creator's prefs
    spawn_execution_notification(&state, &record, &workflow);
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn start_execution_from_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<StartWorkflowExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    state
        .billing_store
        .check_execution_quota(&request.tenant_id)
        .map_err(|e| ApiError {
            status: StatusCode::PAYMENT_REQUIRED,
            message: e,
        })?;
    let workflow = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_id)
        .await?;
    if workflow.status == "archived" {
        return Err(WorkflowError::ArchivedWorkflow.into());
    }
    let workflow_for_notif = workflow.clone();
    let workflow_version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let workflow_version = state
        .workflow_service
        .get_version(&request.tenant_id, &workflow_version_id)
        .await?;
    let env_set = request.env_set.as_deref().unwrap_or(DEFAULT_SET);
    let graph = resolve_graph_credentials(
        workflow_version.graph,
        &state.credential_store,
        &state.env_store,
        &request.tenant_id,
        env_set,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let tenant_id = request.tenant_id.clone();
    let tenant_running = state
        .execution_service
        .count_running_by_tenant(&tenant_id)
        .await
        .unwrap_or(0);
    let per_tenant_max = max_executions_per_tenant();
    if tenant_running >= per_tenant_max {
        return Err(ApiError::bad_request(&format!(
            "Tenant execution limit reached ({tenant_running}/{per_tenant_max} active). Try again when a run completes."
        )));
    }
    // Per-workflow hourly rate limit
    if let Some(max_per_hour) = workflow.max_runs_per_hour {
        let hour_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(3600);
        let recent = state
            .execution_service
            .list(&tenant_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.workflow_id == workflow_id && e.started_at >= hour_ago)
            .count() as u32;
        if recent >= max_per_hour {
            return Err(ApiError {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: format!(
                    "Workflow rate limit reached ({recent}/{max_per_hour} runs in the last hour)"
                ),
            });
        }
    }
    // Per-workflow concurrent execution limit
    if let Some(max_concurrent) = workflow.max_concurrent_runs {
        let wf_running = state
            .execution_service
            .count_running_by_workflow(&tenant_id, &workflow_id)
            .await
            .unwrap_or(0);
        if wf_running >= max_concurrent as u64 {
            return Err(ApiError { status: StatusCode::TOO_MANY_REQUESTS, message: format!("Workflow concurrent execution limit reached ({wf_running}/{max_concurrent} active runs)") });
        }
    }
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: request.tenant_id,
            workflow_id: workflow.id,
            workflow_version_id: workflow_version.id,
            graph,
            input_json: request.input_json,
            label: request.label,
            callback_url: request.callback_url,
            trigger_type: Some("manual".to_string()),
            dry_run: request.dry_run.unwrap_or(false),
            retried_from: None,
        })
        .await?;
    let prev_used = state
        .billing_store
        .billing_status(&record.tenant_id)
        .usage
        .executions_used;
    state.billing_store.increment_execution(&record.tenant_id);
    spawn_quota_alert(&state, &record.tenant_id, prev_used);
    state.audit_store.record(
        &record.tenant_id,
        audit_action::EXECUTION_STARTED,
        "execution",
        &record.id,
        None,
    );
    spawn_execution_notification(&state, &record, &workflow_for_notif);
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn get_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Query(query): Query<GetWorkflowVersionQuery>,
) -> Result<Json<WorkflowVersionRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let record = state
        .workflow_service
        .get_version(&tenant_id, &workflow_version_id)
        .await?;
    Ok(Json(record))
}

async fn rollback_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, version_id)): Path<(String, String)>,
    Query(query): Query<GetWorkflowQuery>,
) -> Result<(StatusCode, Json<WorkflowVersionRecord>), ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let source = state
        .workflow_service
        .get_version(&tenant_id, &version_id)
        .await?;
    if source.workflow_id != workflow_id {
        return Err(ApiError::not_found(
            "workflow version not found for this workflow",
        ));
    }
    let new_version = state
        .workflow_service
        .create_version(
            &workflow_id,
            crate::workflow::CreateWorkflowVersionRequest {
                tenant_id: tenant_id.clone(),
                graph: source.graph,
                status: Some("draft".to_string()),
                message: Some(format!("Rollback to v{}", source.version)),
            },
        )
        .await?;
    state.audit_store.record(
        &tenant_id,
        "workflow.version.rollback",
        "workflow_version",
        &new_version.id,
        None,
    );
    Ok((StatusCode::CREATED, Json(new_version)))
}

async fn create_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut request): Json<CreateWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    if request.created_by.is_none() {
        request.created_by = claims.as_ref().and_then(|c| c.user_id.clone());
    }
    let record = state.workflow_service.create_workflow(request).await?;
    state.audit_store.record(
        &record.tenant_id,
        audit_action::WORKFLOW_CREATED,
        "workflow",
        &record.id,
        None,
    );
    Ok((StatusCode::CREATED, Json(record)))
}

async fn list_workflows(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ListWorkflowsQuery>,
) -> Result<Json<Vec<WorkflowRecord>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let caller_user_id = claims.as_ref().and_then(|c| c.user_id.clone());
    let caller_role = claims.as_ref().map(|c| c.role.as_str()).unwrap_or("");
    let mut records = state
        .workflow_service
        .list_workflows(
            &tenant_id,
            query.project_id.as_deref(),
            query.status.as_deref(),
            query.limit,
        )
        .await?;
    records.retain(|r| {
        r.visibility != "private"
            || caller_role == "admin"
            || r.created_by.as_deref() == caller_user_id.as_deref()
    });
    if let Some(tag) = &query.tag {
        records.retain(|r| r.tags.iter().any(|t| t == tag));
    }
    if let Some(folder) = &query.folder {
        records.retain(|r| r.folder.as_deref() == Some(folder.as_str()));
    }
    Ok(Json(records))
}

async fn get_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<GetWorkflowQuery>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let record = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await?;
    if record.visibility == "private" {
        let caller_user_id = claims.as_ref().and_then(|c| c.user_id.clone());
        let caller_role = claims.as_ref().map(|c| c.role.as_str()).unwrap_or("");
        if caller_role != "admin" && record.created_by.as_deref() != caller_user_id.as_deref() {
            return Err(ApiError::not_found("workflow not found"));
        }
    }
    Ok(Json(record))
}

async fn update_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<UpdateWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let tags_changed = request.tags.is_some();
    let record = state
        .workflow_service
        .update_workflow(&workflow_id, request)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_UPDATED,
        "workflow",
        &workflow_id,
        None,
    );
    if tags_changed && !record.tags.is_empty() {
        let tag_detail = serde_json::Value::String(record.tags.join(","));
        state.audit_store.record(
            &tenant_id,
            audit_action::WORKFLOW_TAGGED,
            "workflow",
            &workflow_id,
            Some(tag_detail),
        );
    }
    Ok(Json(record))
}

async fn archive_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<ArchiveWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let record = state
        .workflow_service
        .archive_workflow(&workflow_id, request)
        .await?;
    // Remove any schedule for this workflow's latest version.
    if let Some(version_id) = &record.latest_version_id {
        if state.schedule_store.unregister(version_id) {
            state.audit_store.record(
                &tenant_id,
                audit_action::SCHEDULE_REMOVED,
                "workflow_version",
                version_id,
                None,
            );
        }
    }
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_ARCHIVED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn restore_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<RestoreWorkflowRequest>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let record = state
        .workflow_service
        .restore_workflow(&workflow_id, request)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_RESTORED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn pin_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .pin_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_PINNED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn unpin_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .unpin_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_UNPINNED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn lock_workflow_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .lock_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_LOCKED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn unlock_workflow_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(
        &claims,
        body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let record = state
        .workflow_service
        .unlock_workflow(&tenant_id, &workflow_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::WORKFLOW_UNLOCKED,
        "workflow",
        &workflow_id,
        None,
    );
    Ok(Json(record))
}

async fn set_workflow_visibility_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut body): Json<SetVisibilityBody>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let existing = state
        .workflow_service
        .get_workflow(&body.tenant_id, &workflow_id)
        .await?;
    let caller_user_id = claims.as_ref().and_then(|c| c.user_id.clone());
    let caller_role = claims.as_ref().map(|c| c.role.as_str()).unwrap_or("");
    if caller_role != "admin" && existing.created_by.as_deref() != caller_user_id.as_deref() {
        return Err(ApiError::forbidden(
            "Only the creator or an admin can change workflow visibility",
        ));
    }
    let record = state
        .workflow_service
        .set_workflow_visibility(&body.tenant_id, &workflow_id, &body.visibility)
        .await?;
    Ok(Json(record))
}

async fn move_workflow_folder(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut body): Json<MoveWorkflowBody>,
) -> Result<Json<WorkflowRecord>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let current = state
        .workflow_service
        .get_workflow(&body.tenant_id, &workflow_id)
        .await?;
    let record = state
        .workflow_service
        .update_workflow(
            &workflow_id,
            UpdateWorkflowRequest {
                tenant_id: body.tenant_id.clone(),
                name: current.name,
                tags: None,
                description: None,
                readme: None,
                folder: Some(body.folder.unwrap_or_default()).filter(|s| !s.is_empty()),
                sla_seconds: None,
                max_runs_per_hour: None,
                max_concurrent_runs: None,
                budget_usd: None,
            },
        )
        .await?;
    Ok(Json(record))
}

async fn workflow_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<WorkflowStats>, ApiError> {
    let tenant_id = effective_tenant_id(
        &claims,
        query.get("tenant_id").map(|s| s.as_str()).unwrap_or(""),
    );
    let records = state.execution_service.list(&tenant_id).await?;
    let wf_execs: Vec<_> = records
        .into_iter()
        .filter(|r| r.workflow_id == workflow_id)
        .collect();
    let total = wf_execs.len();
    let succeeded = wf_execs
        .iter()
        .filter(|r| matches!(r.status, execution_core::ExecutionStatus::Succeeded))
        .count();
    let failed = wf_execs
        .iter()
        .filter(|r| matches!(r.status, execution_core::ExecutionStatus::Failed))
        .count();
    let running = wf_execs
        .iter()
        .filter(|r| {
            matches!(
                r.status,
                execution_core::ExecutionStatus::Running
                    | execution_core::ExecutionStatus::WaitingApproval
            )
        })
        .count();
    let durations: Vec<f64> = wf_execs
        .iter()
        .filter_map(|r| r.finished_at.map(|f| (f as f64) - (r.started_at as f64)))
        .filter(|&d| d >= 0.0)
        .collect();
    let avg_duration_secs = if durations.is_empty() {
        None
    } else {
        Some(durations.iter().sum::<f64>() / durations.len() as f64)
    };
    Ok(Json(WorkflowStats {
        total,
        succeeded,
        failed,
        running,
        avg_duration_secs,
    }))
}

async fn workflow_estimate_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<WorkflowEstimate> {
    let tenant_id = effective_tenant_id(
        &claims,
        query.get("tenant_id").map(|s| s.as_str()).unwrap_or(""),
    );
    let records = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let mut durations: Vec<f64> = records
        .into_iter()
        .filter(|r| {
            r.workflow_id == workflow_id
                && matches!(r.status, execution_core::ExecutionStatus::Succeeded)
        })
        .filter_map(|r| r.finished_at.map(|f| (f as f64) - (r.started_at as f64)))
        .filter(|&d| d >= 0.0)
        .collect();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = durations.len();
    let percentile = |p: f64| -> Option<f64> {
        if n == 0 {
            return None;
        }
        let idx = ((n as f64) * p).floor() as usize;
        Some(durations[idx.min(n - 1)])
    };
    Json(WorkflowEstimate {
        sample_count: n,
        p50_secs: percentile(0.5),
        p95_secs: percentile(0.95),
        min_secs: durations.first().copied(),
        max_secs: durations.last().copied(),
    })
}

async fn workflow_node_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<Vec<NodeStat>> {
    let tenant_id = effective_tenant_id(
        &claims,
        query.get("tenant_id").map(|s| s.as_str()).unwrap_or(""),
    );
    let records = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let wf_execs: Vec<_> = records
        .into_iter()
        .filter(|r| r.workflow_id == workflow_id)
        .collect();

    let mut map: HashMap<String, NodeStat> = HashMap::new();
    let mut durations: HashMap<String, Vec<f64>> = HashMap::new();

    for exec in &wf_execs {
        if let Ok(full) = state.execution_service.get(&tenant_id, &exec.id).await {
            for nr in &full.node_results {
                let st = map.entry(nr.node_id.clone()).or_insert(NodeStat {
                    node_id: nr.node_id.clone(),
                    node_type: nr.node_type.clone(),
                    total: 0,
                    succeeded: 0,
                    failed: 0,
                    skipped: 0,
                    avg_duration_ms: None,
                });
                st.total += 1;
                match nr.status {
                    execution_core::NodeStatus::Succeeded => st.succeeded += 1,
                    execution_core::NodeStatus::Failed => st.failed += 1,
                    execution_core::NodeStatus::Skipped => st.skipped += 1,
                    _ => {}
                }
                if nr.duration_ms > 0 {
                    durations
                        .entry(nr.node_id.clone())
                        .or_default()
                        .push(nr.duration_ms as f64);
                }
            }
        }
    }

    for (node_id, durs) in &durations {
        if let Some(st) = map.get_mut(node_id) {
            st.avg_duration_ms = Some(durs.iter().sum::<f64>() / durs.len() as f64);
        }
    }

    let mut stats: Vec<_> = map.into_values().collect();
    stats.sort_by(|a, b| b.total.cmp(&a.total));
    Json(stats)
}

async fn create_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<CreateWorkflowVersionRequest>,
) -> Result<(StatusCode, Json<WorkflowVersionRecord>), ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let wf = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_id)
        .await?;
    if wf.locked {
        return Err(WorkflowError::LockedWorkflow.into());
    }
    let record = state
        .workflow_service
        .create_version(&workflow_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn publish_workflow_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Json(mut request): Json<PublishWorkflowVersionRequest>,
) -> Result<Json<WorkflowVersionRecord>, ApiError> {
    require_write(&claims)?;
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let record = state
        .workflow_service
        .publish_version(&request.tenant_id, &workflow_version_id)
        .await?;

    // If the trigger node has an interval_secs or cron_expression config, register with the scheduler.
    let _ = state.schedule_store.unregister(&workflow_version_id);
    let cron_expr = extract_trigger_cron(&record.graph);
    let interval_secs = extract_trigger_interval(&record.graph).unwrap_or(0);
    if cron_expr.is_some() || interval_secs >= 60 {
        let next_run_at = cron_expr
            .as_deref()
            .and_then(crate::scheduler::cron_next_instant)
            .unwrap_or_else(|| {
                std::time::Instant::now() + std::time::Duration::from_secs(interval_secs)
            });
        state.schedule_store.register(ScheduleEntry {
            workflow_id: record.workflow_id.clone(),
            workflow_version_id: record.id.clone(),
            tenant_id: request.tenant_id.clone(),
            interval_secs,
            cron_expression: cron_expr,
            next_run_at,
            paused: false,
        });
        state.audit_store.record(
            &request.tenant_id,
            audit_action::SCHEDULE_REGISTERED,
            "workflow_version",
            &record.id,
            None,
        );
    }
    state.audit_store.record(
        &request.tenant_id,
        audit_action::WORKFLOW_PUBLISHED,
        "workflow_version",
        &record.id,
        None,
    );

    Ok(Json(record))
}

async fn list_workflow_versions(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<ListWorkflowVersionsQuery>,
) -> Result<Json<Vec<WorkflowVersionRecord>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let records = state
        .workflow_service
        .list_versions(
            &tenant_id,
            &workflow_id,
            query.status.as_deref(),
            query.limit,
        )
        .await?;
    Ok(Json(records))
}

async fn create_webhook(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_version_id): Path<String>,
    Json(mut body): Json<CreateWebhookBody>,
) -> Result<Json<WebhookResponse>, ApiError> {
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    match state
        .webhook_store
        .get_by_version(&workflow_version_id)
        .await
    {
        Ok(Some(existing)) => {
            return Ok(Json(WebhookResponse {
                url: format!("/v1/webhooks/{}", existing.token),
                token: existing.token,
                secret: existing.secret,
            }));
        }
        Ok(None) => {}
        Err(e) => return Err(e.into()),
    }

    let version = state
        .workflow_service
        .get_version(&body.tenant_id, &workflow_version_id)
        .await?;

    let secret = extract_trigger_webhook_secret(&version.graph);
    let token = uuid::Uuid::new_v4().to_string().replace('-', "");
    let record = state
        .webhook_store
        .upsert(WebhookRecord {
            token,
            tenant_id: body.tenant_id,
            workflow_id: version.workflow_id,
            workflow_version_id,
            secret: secret.clone(),
            condition_expr: None,
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .map_err(ApiError::from)?;

    Ok(Json(WebhookResponse {
        url: format!("/v1/webhooks/{}", record.token),
        token: record.token,
        secret,
    }))
}

async fn export_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<ExportWorkflowQuery>,
) -> Result<Json<WorkflowExport>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let workflow = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await?;
    let version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let version = state
        .workflow_service
        .get_version(&tenant_id, &version_id)
        .await?;
    let exported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(Json(WorkflowExport {
        name: workflow.name.clone(),
        description: workflow.description.clone(),
        readme: workflow.readme.clone(),
        tags: workflow.tags.clone(),
        graph: version.graph,
        exported_at,
    }))
}

async fn import_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<ImportWorkflowBody>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let name = body
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Imported Workflow".to_string());
    let mut workflow = state
        .workflow_service
        .create_workflow(CreateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            workspace_id: body.workspace_id,
            project_id: body.project_id,
            name,
            description: body.description.clone(),
            folder: None,
            created_by: claims.as_ref().and_then(|c| c.user_id.clone()),
        })
        .await?;
    // Restore readme and tags from export if provided
    if body.readme.is_some() || !body.tags.is_empty() {
        let update = UpdateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            name: workflow.name.clone(),
            description: body.description.clone(),
            tags: if body.tags.is_empty() {
                None
            } else {
                Some(body.tags.clone())
            },
            readme: body.readme.clone(),
            folder: None,
            sla_seconds: None,
            max_runs_per_hour: None,
            max_concurrent_runs: None,
            budget_usd: None,
        };
        if let Ok(updated) = state
            .workflow_service
            .update_workflow(&workflow.id, update)
            .await
        {
            workflow = updated;
        }
    }
    let version = state
        .workflow_service
        .create_version(
            &workflow.id,
            CreateWorkflowVersionRequest {
                tenant_id: body.tenant_id.clone(),
                graph: body.graph,
                status: None,
                message: None,
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
    workflow.latest_version_id = Some(version.id);
    Ok((StatusCode::CREATED, Json(workflow)))
}

// ── AI Workflow Generation ──────────────────────────────────────────────────

async fn generate_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<GenerateWorkflowRequest>,
) -> Result<(StatusCode, Json<GenerateWorkflowResponse>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);

    let api_key = body
        .api_key
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .ok_or_else(|| {
            ApiError::bad_request(
                "No Claude API key: provide api_key in request or set ANTHROPIC_API_KEY env var",
            )
        })?;
    let model = body
        .model
        .as_deref()
        .unwrap_or("claude-sonnet-4-6")
        .to_string();

    let system_prompt = r#"You are an expert workflow designer for Trigix, an AI-powered automation platform.

Generate a workflow graph JSON based on the user's description. Respond with ONLY valid JSON in this exact structure:
{
  "name": "Workflow name (concise)",
  "description": "One sentence description",
  "graph": {
    "workflow_version_id": "draft",
    "nodes": [
      { "id": "node_1", "type": "trigger", "config": {} },
      { "id": "node_2", "type": "...", "config": { ... } }
    ],
    "edges": [
      { "source": "node_1", "target": "node_2" }
    ]
  }
}

Available node types and their required config fields:
- trigger: {} (always start here)
- http: { url, method (GET/POST), headers?, body? }
- claude: { api_key, model (claude-sonnet-4-6), prompt_template, system_prompt?, max_tokens? }
- openai: { api_key, model (gpt-4o-mini), prompt_template, system_prompt?, max_tokens? }
- condition: { field (dot-path), operator (equals/not_equals/contains/gt/lt/exists/not_exists), value? }
- transform: { template (JSON with {{node_id.field}} placeholders) }
- filter: { items (expr), field, operator, value? }
- aggregate: { items (expr), operation (count/sum/avg/min/max/join/first/last), field? }
- delay: { delay_secs }
- slack: { webhook_url, message_template }
- github: { token, endpoint, method }
- jira: { base_url, email, token, endpoint, method, body? }
- notion: { token, endpoint, method, body? }
- database: { url, query }
- code: { code (Rhai script) }
- sub_workflow: { workflow_id }
- fan_out: {} (parallel split)
- fan_in: {} (wait for all parallel branches)
- assert: { condition, message? }
- validate: { source, schema }
- loop: { items, template? }
- extract: { source, path }
- merge: { fields: [{source, key?}] }
- catch: {} (error handler — connect with error edge from failing node)
- note: { text } (documentation only)

Template variables: {{input.field}}, {{node_id.field}}, {{credential.name}}, {{env.KEY}}
Edges: source → target. For condition nodes add condition_label: "true" or "false" on edges.

Rules:
- Always start with a trigger node as node_1
- Use descriptive node IDs like "fetch_data", "parse_response", "send_slack"
- Keep graphs focused — 3-8 nodes is ideal
- Use {{credential.name}} for sensitive values (API keys, tokens)
- Return ONLY the JSON, no explanation"#;

    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "system": system_prompt,
        "messages": [{ "role": "user", "content": body.prompt }],
    });

    let http_client = reqwest::Client::new();
    let resp = http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude request failed: {e}")))?;

    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(&format!("Claude API {code}: {text}")));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude response parse error: {e}")))?;

    let raw_content = resp_json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    // Extract JSON from the response (may be wrapped in markdown code blocks)
    let json_str = if raw_content.contains("```") {
        raw_content
            .split("```")
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.trim_start_matches("json").trim())
            .next()
            .unwrap_or(&raw_content)
            .to_string()
    } else {
        raw_content.clone()
    };

    let generated: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|_| ApiError::bad_request(&format!("Claude returned invalid JSON: {json_str}")))?;

    let name = generated["name"]
        .as_str()
        .unwrap_or("Generated Workflow")
        .to_string();
    let description = generated["description"].as_str().unwrap_or("").to_string();
    let graph = generated["graph"].clone();

    if graph.is_null() {
        return Err(ApiError::bad_request(
            "Claude response missing 'graph' field",
        ));
    }

    let mut workflow_record: Option<crate::workflow::WorkflowRecord> = None;

    if body.create {
        let wf = state
            .workflow_service
            .create_workflow(crate::workflow::CreateWorkflowRequest {
                tenant_id: body.tenant_id.clone(),
                workspace_id: body.workspace_id.unwrap_or_default(),
                project_id: body.project_id.unwrap_or_default(),
                name: name.clone(),
                description: Some(description.clone()),
                folder: None,
                created_by: claims.as_ref().and_then(|c| c.user_id.clone()),
            })
            .await?;

        // Deserialize the graph JSON into WorkflowGraph so create_version can store it
        let mut graph_val = graph.clone();
        if let Some(obj) = graph_val.as_object_mut() {
            obj.insert(
                "workflow_version_id".to_string(),
                serde_json::Value::String("draft".to_string()),
            );
        }
        let workflow_graph: workflow_core::WorkflowGraph = serde_json::from_value(graph_val)
            .map_err(|e| ApiError::bad_request(&format!("Invalid graph structure: {e}")))?;

        state
            .workflow_service
            .create_version(
                &wf.id,
                crate::workflow::CreateWorkflowVersionRequest {
                    tenant_id: body.tenant_id.clone(),
                    graph: workflow_graph,
                    status: None,
                    message: Some("Generated by AI".to_string()),
                },
            )
            .await?;

        state.audit_store.record(
            &body.tenant_id,
            audit_action::WORKFLOW_CREATED,
            "workflow",
            &wf.id,
            Some(serde_json::Value::String("ai_generated".to_string())),
        );

        workflow_record = Some(wf);
    }

    Ok((
        StatusCode::CREATED,
        Json(GenerateWorkflowResponse {
            graph,
            name,
            description,
            workflow: workflow_record,
        }),
    ))
}

async fn copilot_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CopilotRequest>,
) -> Result<Json<CopilotResponse>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);

    let api_key = body
        .api_key
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .ok_or_else(|| {
            ApiError::bad_request("No Claude API key: provide api_key or set ANTHROPIC_API_KEY")
        })?;

    let graph_context = if let Some(g) = &body.graph_json {
        format!("\n\nCurrent workflow graph (JSON):\n```json\n{}\n```", g)
    } else {
        String::new()
    };

    let system = format!(
        "You are an expert assistant for Trigix, an AI-powered workflow automation platform.\
\n\nYou help users understand, debug, and improve their workflows. You have deep knowledge of:\
\n- All 136 node types (trigger, http, claude, openai, gemini, slack, github, database, code, condition, loop, etc.)\
\n- Template variables: {{{{input.field}}}}, {{{{node_id.field}}}}, {{{{credential.name}}}}, {{{{env.KEY}}}}\
\n- Best practices for workflow design (error handling with catch nodes, validation, retry logic)\
\n- Integration patterns (webhooks, scheduled triggers, fan-out/fan-in parallelism)\
\n\nWhen asked to suggest changes, provide concrete, actionable advice with example node configs in JSON.\
\nKeep replies concise and practical — 2-5 sentences for simple questions, structured lists for complex ones.{}",
        graph_context
    );

    let payload = serde_json::json!({
        "model": body.model,
        "max_tokens": 1024,
        "system": system,
        "messages": [{ "role": "user", "content": body.message }],
    });

    let http_client = reqwest::Client::new();
    let resp = http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude request failed: {e}")))?;

    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(&format!("Claude API {code}: {text}")));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude response parse: {e}")))?;

    let reply = resp_json["content"][0]["text"]
        .as_str()
        .unwrap_or("(no response)")
        .trim()
        .to_string();

    let _ = state.audit_store.record(
        &body.tenant_id,
        "copilot.query",
        "copilot",
        &body.tenant_id,
        None,
    );

    Ok(Json(CopilotResponse { reply }))
}

async fn duplicate_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut body): Json<DuplicateWorkflowBody>,
) -> Result<(StatusCode, Json<WorkflowRecord>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let original = state
        .workflow_service
        .get_workflow(&body.tenant_id, &workflow_id)
        .await?;
    let new_name = format!("{} (copy)", original.name);
    let mut new_workflow = state
        .workflow_service
        .create_workflow(CreateWorkflowRequest {
            tenant_id: body.tenant_id.clone(),
            workspace_id: original.workspace_id,
            project_id: original.project_id,
            name: new_name,
            description: original.description,
            folder: original.folder,
            created_by: claims.as_ref().and_then(|c| c.user_id.clone()),
        })
        .await?;
    if let Some(version_id) = &original.latest_version_id {
        let version = state
            .workflow_service
            .get_version(&body.tenant_id, version_id)
            .await?;
        let dup_version = state
            .workflow_service
            .create_version(
                &new_workflow.id,
                CreateWorkflowVersionRequest {
                    tenant_id: body.tenant_id.clone(),
                    graph: version.graph,
                    status: None,
                    message: None,
                },
            )
            .await?;
        new_workflow.latest_version_id = Some(dup_version.id);
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

async fn workflow_json_schema_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<TenantQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let workflow = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await?;
    let schema = if let Some(version_id) = &workflow.latest_version_id {
        state
            .workflow_service
            .get_version(&tenant_id, version_id)
            .await
            .ok()
            .map(|v| v.graph.input_schema)
            .unwrap_or_default()
    } else {
        vec![]
    };

    let mut properties = serde_json::Map::new();
    let mut required_fields: Vec<serde_json::Value> = vec![];

    for field in &schema {
        let json_type = match field.field_type.as_str() {
            "number" => "number",
            "boolean" => "boolean",
            "json" => "object",
            _ => "string",
        };
        let mut prop = serde_json::json!({ "type": json_type });
        if !field.description.is_empty() {
            prop["description"] = serde_json::Value::String(field.description.clone());
        }
        if let Some(ref default) = field.default_value {
            prop["default"] = serde_json::Value::String(default.clone());
        }
        properties.insert(field.key.clone(), prop);
        if field.required {
            required_fields.push(serde_json::Value::String(field.key.clone()));
        }
    }

    let mut result = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": workflow.name,
        "type": "object",
        "properties": properties,
    });
    if !required_fields.is_empty() {
        result["required"] = serde_json::Value::Array(required_fields);
    }
    Ok(Json(result))
}

async fn workflow_health_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<WorkflowHealthReport>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let mut issues: Vec<WorkflowHealthIssue> = Vec::new();

    // Get the workflow
    let wf = state
        .workflow_service
        .get_workflow(&tenant_id, &workflow_id)
        .await
        .map_err(|_| ApiError::not_found("Workflow not found"))?;

    // Check published version exists
    let published_version_id: Option<String> = {
        let versions = state
            .workflow_service
            .list_versions(&tenant_id, &workflow_id, None, None)
            .await
            .unwrap_or_default();
        versions
            .into_iter()
            .find(|v| v.status == "published")
            .map(|v| v.id)
    };

    if published_version_id.is_none() {
        issues.push(WorkflowHealthIssue {
            severity: "warning".into(),
            message: "No published version. Workflow cannot be triggered.".into(),
        });
    }

    // Check lock status
    if wf.locked {
        issues.push(WorkflowHealthIssue {
            severity: "warning".into(),
            message: "Workflow is locked. Edits are prevented.".into(),
        });
    }

    // Check credentials referenced in the published version's graph
    if let Some(ref vid) = published_version_id {
        if let Ok(ver) = state.workflow_service.get_version(&tenant_id, vid).await {
            let cred_names: Vec<String> = {
                let graph_str = serde_json::to_string(&ver.graph).unwrap_or_default();
                let mut names = Vec::new();
                let mut remaining = graph_str.as_str();
                while let Some(start) = remaining.find("{{credential.") {
                    remaining = &remaining[start + 13..];
                    if let Some(end) = remaining.find("}}") {
                        names.push(remaining[..end].to_string());
                        remaining = &remaining[end + 2..];
                    }
                }
                names.sort();
                names.dedup();
                names
            };
            for name in &cred_names {
                match state.credential_store.get_by_name(&tenant_id, name).await {
                    Ok(None) => issues.push(WorkflowHealthIssue {
                        severity: "error".into(),
                        message: format!(
                            "Credential '{}' referenced in graph but not found in store.",
                            name
                        ),
                    }),
                    Err(_) => {}
                    Ok(Some(_)) => {
                        // Credential found — check if expiring
                        let creds = state
                            .credential_store
                            .list(&tenant_id)
                            .await
                            .unwrap_or_default();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        for cred in &creds {
                            if cred.name == *name {
                                if let Some(exp) = cred.expires_at {
                                    let days = (exp.saturating_sub(now)) / 86400;
                                    if exp <= now {
                                        issues.push(WorkflowHealthIssue {
                                            severity: "error".into(),
                                            message: format!("Credential '{}' has EXPIRED. Rotate it immediately.", name),
                                        });
                                    } else if days <= 7 {
                                        issues.push(WorkflowHealthIssue {
                                            severity: "warning".into(),
                                            message: format!(
                                                "Credential '{}' expires in {} day(s).",
                                                name, days
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Check last run status
    let all_execs = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let last_exec = all_execs
        .iter()
        .filter(|e| e.workflow_id == workflow_id)
        .max_by_key(|e| e.started_at);
    let last_run_status = last_exec.map(|e| format!("{:?}", e.status).to_lowercase());
    let last_run_at = last_exec.map(|e| e.started_at);

    if let Some(ref s) = last_run_status {
        if s == "failed" {
            issues.push(WorkflowHealthIssue {
                severity: "warning".into(),
                message: "Most recent execution failed.".into(),
            });
        }
    }

    let status = if issues.iter().any(|i| i.severity == "error") {
        "error"
    } else if !issues.is_empty() {
        "warning"
    } else {
        "healthy"
    };

    Ok(Json(WorkflowHealthReport {
        workflow_id,
        status: status.into(),
        issues,
        published_version_id,
        last_run_status,
        last_run_at,
    }))
}

async fn get_latest_execution_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<Option<ExecutionSummary>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let all = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let latest = all
        .into_iter()
        .filter(|e| e.workflow_id == workflow_id)
        .max_by_key(|e| e.started_at);
    Ok(Json(latest))
}

async fn list_variables_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(query): Query<VariableQuery>,
) -> Json<Vec<crate::variables::Variable>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.variable_store.list(&tenant_id, &workflow_id).await)
}

async fn get_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
) -> Result<Json<crate::variables::Variable>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .variable_store
        .get(&tenant_id, &workflow_id, &key)
        .await
        .map(Json)
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "VariableNotFound".to_string(),
        })
}

async fn set_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
    Json(body): Json<SetVariableBody>,
) -> Json<crate::variables::Variable> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(
        state
            .variable_store
            .set(&tenant_id, &workflow_id, &key, body.value)
            .await,
    )
}

async fn delete_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
) -> StatusCode {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state
        .variable_store
        .delete(&tenant_id, &workflow_id, &key)
        .await
    {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn increment_variable_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((workflow_id, key)): Path<(String, String)>,
    Query(query): Query<VariableQuery>,
    Json(body): Json<IncrementVariableBody>,
) -> Json<crate::variables::Variable> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(
        state
            .variable_store
            .increment(&tenant_id, &workflow_id, &key, body.by)
            .await,
    )
}

// ── Workspace / Project ───────────────────────────────────────────────────

async fn list_test_cases_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<Vec<serde_json::Value>> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let cases = state.test_case_store.list(&tenant_id, &workflow_id).await;
    Json(
        cases
            .into_iter()
            .map(|tc| serde_json::to_value(&tc).unwrap_or_default())
            .collect(),
    )
}

async fn create_test_case_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<CreateTestCaseRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let tenant_id = request.tenant_id.clone();
    let tc = state
        .test_case_store
        .create(&tenant_id, &workflow_id, request)
        .await
        .map_err(|_| ApiError::internal("test_case_create_failed"))?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&tc).unwrap_or_default()),
    ))
}

async fn get_test_case_handler(
    State(state): State<AppState>,
    Path(test_case_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tc = state
        .test_case_store
        .get(&test_case_id)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    Ok(Json(serde_json::to_value(&tc).unwrap_or_default()))
}

async fn update_test_case_handler(
    State(state): State<AppState>,
    Path(test_case_id): Path<String>,
    Json(request): Json<UpdateTestCaseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tc = state
        .test_case_store
        .update(&test_case_id, request)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    Ok(Json(serde_json::to_value(&tc).unwrap_or_default()))
}

async fn delete_test_case_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(test_case_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    state
        .test_case_store
        .delete(&test_case_id)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn run_test_case_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(test_case_id): Path<String>,
) -> Result<Json<TestCaseRunResult>, ApiError> {
    let tc = state
        .test_case_store
        .get(&test_case_id)
        .await
        .map_err(|e| match e {
            TestCaseError::NotFound => ApiError::not_found("test_case"),
            _ => ApiError::internal("test_case_store"),
        })?;
    // Verify caller owns this test case's tenant (returns 404 to avoid leaking existence).
    let caller_tenant = effective_tenant_id(&claims, &tc.tenant_id);
    if caller_tenant != tc.tenant_id {
        return Err(ApiError::not_found("test_case"));
    }
    let workflow = state
        .workflow_service
        .get_workflow(&tc.tenant_id, &tc.workflow_id)
        .await?;
    let version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let version = state
        .workflow_service
        .get_version(&tc.tenant_id, &version_id)
        .await?;
    let graph = resolve_graph_credentials(
        version.graph,
        &state.credential_store,
        &state.env_store,
        &tc.tenant_id,
        DEFAULT_SET,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &tc.tenant_id,
    )
    .await;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: tc.tenant_id.clone(),
            workflow_id: tc.workflow_id.clone(),
            workflow_version_id: version_id,
            graph,
            input_json: tc.input_json.clone(),
            label: Some(format!("test:{}", &tc.name)),
            callback_url: None,
            trigger_type: Some("test".to_string()),
            dry_run: false,
            retried_from: None,
        })
        .await?;
    let passed = if let (Some(expected), Some(actual)) = (&tc.expected_output, &record.output_json)
    {
        let ev: serde_json::Value = serde_json::from_str(expected).unwrap_or_default();
        let av: serde_json::Value = serde_json::from_str(actual).unwrap_or_default();
        ev == av
    } else {
        tc.expected_output.is_none()
    };
    Ok(Json(TestCaseRunResult {
        test_case_id: tc.id,
        execution_id: record.id,
        status: format!("{:?}", record.status).to_lowercase(),
        passed,
        output_json: record.output_json,
        expected_output: tc.expected_output,
    }))
}

// ── Event Subscriptions ────────────────────────────────────────────────────

async fn list_event_subscriptions_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let subs = state
        .subscription_store
        .list(&tenant_id)
        .await
        .map_err(|_| ApiError::internal("subscription_store"))?;
    Ok(Json(
        subs.into_iter()
            .map(|s| serde_json::to_value(&s).unwrap_or_default())
            .collect(),
    ))
}

async fn create_event_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut req): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    req.tenant_id = effective_tenant_id(&claims, &req.tenant_id);
    let sub = state
        .subscription_store
        .create(req)
        .await
        .map_err(|e| match e {
            SubscriptionError::InvalidUrl => {
                ApiError::bad_request("url must start with http or https")
            }
            _ => ApiError::internal("subscription_store"),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&sub).unwrap_or_default()),
    ))
}

async fn delete_event_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(sub_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    state
        .subscription_store
        .delete(&tenant_id, &sub_id)
        .await
        .map_err(|e| match e {
            SubscriptionError::NotFound => ApiError::not_found("event_subscription"),
            _ => ApiError::internal("subscription_store"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Workflow Comments ──────────────────────────────────────────────────────

async fn list_workflow_comments_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let comments = state
        .comment_store
        .list(&tenant_id, &workflow_id)
        .await
        .map_err(|_| ApiError::internal("comment_store"))?;
    Ok(Json(
        comments
            .into_iter()
            .map(|c| serde_json::to_value(&c).unwrap_or_default())
            .collect(),
    ))
}

async fn create_workflow_comment_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(req): Json<CreateCommentBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let tenant_id = effective_tenant_id(&claims, &req.tenant_id);
    use crate::comments::CommentError;
    let comment = state
        .comment_store
        .create(CreateCommentRequest {
            tenant_id,
            workflow_id,
            author: req.author,
            body: req.body,
        })
        .await
        .map_err(|e| match e {
            CommentError::EmptyBody => ApiError::bad_request("comment body must not be empty"),
            _ => ApiError::internal("comment_store"),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&comment).unwrap_or_default()),
    ))
}

async fn edit_workflow_comment_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(comment_id): Path<String>,
    Json(req): Json<EditCommentBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &req.tenant_id);
    use crate::comments::CommentError;
    let comment = state
        .comment_store
        .edit(
            &tenant_id,
            &comment_id,
            EditCommentRequest {
                tenant_id: tenant_id.clone(),
                body: req.body,
            },
        )
        .await
        .map_err(|e| match e {
            CommentError::NotFound => ApiError::not_found("comment"),
            CommentError::EmptyBody => ApiError::bad_request("comment body must not be empty"),
            _ => ApiError::internal("comment_store"),
        })?;
    Ok(Json(serde_json::to_value(&comment).unwrap_or_default()))
}

async fn delete_workflow_comment_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(comment_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    use crate::comments::CommentError;
    state
        .comment_store
        .delete(&tenant_id, &comment_id)
        .await
        .map_err(|e| match e {
            CommentError::NotFound => ApiError::not_found("comment"),
            _ => ApiError::internal("comment_store"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Form Publisher ─────────────────────────────────────────────────────────

async fn publish_form_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Json(mut request): Json<PublishFormRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    request.tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let workflow = state
        .workflow_service
        .get_workflow(&request.tenant_id, &workflow_id)
        .await?;
    let input_schema = if let Some(version_id) = &workflow.latest_version_id {
        let version = state
            .workflow_service
            .get_version(&request.tenant_id, version_id)
            .await
            .ok();
        version
            .and_then(|v| {
                v.graph
                    .nodes
                    .iter()
                    .find(|n| n.node_type == workflow_core::NodeType::Trigger)
                    .and_then(|n| n.config.clone())
                    .and_then(|c| c.get("input_schema").cloned())
            })
            .unwrap_or(serde_json::json!([]))
    } else {
        serde_json::json!([])
    };
    let tenant_id = request.tenant_id.clone();
    let record = state
        .form_store
        .publish_form(&tenant_id, &workflow_id, request, input_schema)
        .await
        .map_err(|_| ApiError::internal("form_publish_failed"))?;
    Ok(Json(serde_json::json!({
        "token": record.token,
        "title": record.title,
        "workflow_id": record.workflow_id,
    })))
}

async fn list_forms_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workflow_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<Vec<serde_json::Value>> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let forms = state
        .form_store
        .list_by_workflow(&tenant_id, &workflow_id)
        .await;
    Json(
        forms
            .into_iter()
            .map(|f| {
                serde_json::json!({
                    "token": f.token,
                    "title": f.title,
                    "description": f.description,
                    "created_at": f.created_at,
                })
            })
            .collect(),
    )
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
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
            "/v1/workflows/:workflow_id/pin",
            get(method_not_allowed).post(pin_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/unpin",
            get(method_not_allowed).post(unpin_workflow),
        )
        .route(
            "/v1/workflows/:workflow_id/lock",
            get(method_not_allowed).post(lock_workflow_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/unlock",
            get(method_not_allowed).post(unlock_workflow_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/visibility",
            get(method_not_allowed).patch(set_workflow_visibility_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/move",
            get(method_not_allowed).post(move_workflow_folder),
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
            "/v1/workflows/:workflow_id/latest-execution",
            get(get_latest_execution_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/variables",
            get(list_variables_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/variables/:key",
            get(get_variable_handler)
                .put(set_variable_handler)
                .delete(delete_variable_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/variables/:key/increment",
            post(increment_variable_handler),
        )
        .route("/v1/workflows/:workflow_id/export", get(export_workflow))
        .route(
            "/v1/workflows/:workflow_id/stats",
            get(workflow_stats_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/estimate",
            get(workflow_estimate_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/node-stats",
            get(workflow_node_stats_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/health",
            get(workflow_health_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/json-schema",
            get(workflow_json_schema_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/duplicate",
            get(method_not_allowed).post(duplicate_workflow),
        )
        .route(
            "/v1/workflows/import",
            get(method_not_allowed).post(import_workflow),
        )
        .route(
            "/v1/workflows/generate",
            get(method_not_allowed).post(generate_workflow),
        )
        .route("/v1/copilot", get(method_not_allowed).post(copilot_handler))
        .route(
            "/v1/workflows/:workflow_id/test-cases",
            get(list_test_cases_handler).post(create_test_case_handler),
        )
        .route(
            "/v1/test-cases/:test_case_id",
            get(get_test_case_handler)
                .patch(update_test_case_handler)
                .delete(delete_test_case_handler),
        )
        .route(
            "/v1/test-cases/:test_case_id/run",
            get(method_not_allowed).post(run_test_case_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/publish-form",
            get(method_not_allowed).post(publish_form_handler),
        )
        .route("/v1/workflows/:workflow_id/forms", get(list_forms_handler))
        .route(
            "/v1/workflows/:workflow_id/comments",
            get(list_workflow_comments_handler).post(create_workflow_comment_handler),
        )
        .route(
            "/v1/comments/:comment_id",
            patch(edit_workflow_comment_handler).delete(delete_workflow_comment_handler),
        )
        .route(
            "/v1/event-subscriptions",
            get(list_event_subscriptions_handler).post(create_event_subscription_handler),
        )
        .route(
            "/v1/event-subscriptions/:sub_id",
            delete(delete_event_subscription_handler),
        )
        .route(
            "/v1/workflows/:workflow_id/rollback/:version_id",
            post(rollback_workflow_version),
        )
}

#[cfg(test)]
mod tests {
    use crate::http::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::json;
    use tower::ServiceExt;

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
    async fn draft_version_execution_is_rejected() {
        let app = router();

        // Create a new (draft) version of workflow-1
        let create_body = json!({
            "tenant_id": "tenant-1",
            "graph": {
                "workflow_version_id": "draft-version-x",
                "nodes": [{"id": "trigger", "type": "trigger"}],
                "edges": []
            }
        });
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/versions")
                    .header("content-type", "application/json")
                    .body(Body::from(create_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let body = to_bytes(create_resp.into_body(), usize::MAX).await.unwrap();
        let version: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let draft_id = version["id"].as_str().unwrap().to_string();
        assert_eq!(version["status"], "draft");

        // Trying to run the draft version must be rejected
        let exec_body = json!({
            "tenant_id": "tenant-1",
            "input_json": "{}"
        });
        let exec_resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflow-versions/{draft_id}/executions"))
                    .header("content-type", "application/json")
                    .body(Body::from(exec_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(exec_resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn delete_execution_removes_terminal_execution() {
        let app = router();
        // Start execution on pre-seeded workflow
        let start_body = json!({ "tenant_id": "tenant-1", "input_json": "{}" });
        let start_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflow-versions/version-1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(start_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(start_resp.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(start_resp.into_body(), usize::MAX).await.unwrap();
        let exec: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let exec_id = exec["id"].as_str().unwrap().to_string();

        // Wait for the background executor task to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Delete the finished execution
        let del_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/executions/{exec_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

        // Verify it's gone
        let get_resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/executions/{exec_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
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
                    .body(Body::from(
                        json!({"source": "crm", "lead_id": "lead-99"}).to_string(),
                    ))
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
        let body = to_bytes(ver_response.into_body(), usize::MAX)
            .await
            .unwrap();
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
        let body = to_bytes(sched_response.into_body(), usize::MAX)
            .await
            .unwrap();
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
        let body = to_bytes(sched_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let list = schedules.as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["workflow_id"], wf_id);
        assert_eq!(list[0]["interval_secs"], 3600);
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
        // Import creates the workflow together with a draft version and points
        // latest_version_id at it (see import/duplicate latest_version_id fix).
        assert!(workflow["latest_version_id"].is_string());

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
        let body = to_bytes(ver_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let ver: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let ver_id = ver["id"].as_str().unwrap();

        // Publish to register schedule
        app.clone()
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
        let body = to_bytes(sched_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(schedules.as_array().unwrap().len(), 1);

        // Archive the workflow
        app.clone()
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
        let body = to_bytes(sched_response.into_body(), usize::MAX)
            .await
            .unwrap();
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
        let bytes = to_bytes(exec_response.into_body(), usize::MAX)
            .await
            .unwrap();
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
                    .body(Body::from(json!({ "tenant_id": "tenant-1" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(retry_response.status(), StatusCode::CREATED);
        let bytes = to_bytes(retry_response.into_body(), usize::MAX)
            .await
            .unwrap();
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
        let bytes = to_bytes(exec_response.into_body(), usize::MAX)
            .await
            .unwrap();
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
                    .body(Body::from(json!({ "tenant_id": "tenant-1" }).to_string()))
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
        let bytes = to_bytes(get_response.into_body(), usize::MAX)
            .await
            .unwrap();
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
    async fn search_returns_matching_workflows() {
        let app = router();
        // First create a workflow
        let body = serde_json::json!({ "name": "SearchTargetWorkflow", "workspace_id": "ws-1", "project_id": "project-1", "tenant_id": "t1" });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Search for it
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/search?q=SearchTarget&tenant_id=t1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(result["workflows"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn form_publish_and_get() {
        let app = router();

        // Create a workflow first
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"FormWf"}).to_string())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap().to_string();

        // Publish a form
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/publish-form"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id":"t1","title":"My Form"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let form: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let token = form["token"].as_str().unwrap().to_string();
        assert!(!token.is_empty());

        // Get the form by token
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/forms/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let got: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(got["title"].as_str().unwrap(), "My Form");
        assert_eq!(got["workflow_id"].as_str().unwrap(), wf_id);
    }

    #[tokio::test]
    async fn form_list_and_delete() {
        let app = router();

        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"FormWf2"}).to_string())).unwrap()
        ).await.unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap().to_string();

        // Publish two forms
        for title in ["Form A", "Form B"] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/v1/workflows/{wf_id}/publish-form"))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"tenant_id":"t1","title":title}).to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // List forms for the workflow
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/workflows/{wf_id}/forms?tenant_id=t1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let forms: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(forms.as_array().unwrap().len(), 2);

        // Get token of first form
        let token = forms[0]["token"].as_str().unwrap().to_string();

        // Delete that form
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/forms/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Confirm it's gone
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/forms/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── Test case HTTP tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_case_create_list_update_delete() {
        let app = router();

        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"TcWf"}).to_string())).unwrap()
        ).await.unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap().to_string();

        // Create test case
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/test-cases"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "t1",
                            "name": "TC1",
                            "input_json": r#"{"x":1}"#,
                            "expected_output": r#"{"result":2}"#
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let tc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let tc_id = tc["id"].as_str().unwrap().to_string();
        assert_eq!(tc["name"].as_str().unwrap(), "TC1");

        // List test cases
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/workflows/{wf_id}/test-cases?tenant_id=t1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let tcs: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(tcs.as_array().unwrap().len(), 1);

        // Get test case
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/test-cases/{tc_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Update test case
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/test-cases/{tc_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name": "TC1 Updated"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(updated["name"].as_str().unwrap(), "TC1 Updated");

        // Delete test case
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/test-cases/{tc_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Confirm deletion
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/test-cases/{tc_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── Comment HTTP tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn workflow_comments_crud() {
        let app = router();

        // Create a workflow first
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"CommentWf"}).to_string())).unwrap()
        ).await.unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap().to_string();

        // Create comment
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/comments"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id":"t1","author":"alice","body":"Hello world"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let comment: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let comment_id = comment["id"].as_str().unwrap().to_string();
        assert_eq!(comment["author"].as_str().unwrap(), "alice");
        assert_eq!(comment["body"].as_str().unwrap(), "Hello world");
        assert!(comment["edited_at"].is_null());

        // List comments
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/workflows/{wf_id}/comments?tenant_id=t1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let comments: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(comments.as_array().unwrap().len(), 1);

        // Edit comment
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/comments/{comment_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id":"t1","body":"Updated body"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let edited: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(edited["body"].as_str().unwrap(), "Updated body");
        assert!(!edited["edited_at"].is_null());

        // Delete comment
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/comments/{comment_id}?tenant_id=t1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // List should now be empty
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/workflows/{wf_id}/comments?tenant_id=t1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let comments: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(comments.as_array().unwrap().is_empty());
    }

    // ── Workflow locking HTTP tests ─────────────────────────────────────────

    #[tokio::test]
    async fn workflow_lock_blocks_version_save() {
        let app = router();

        // Create workflow
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"LockWf"}).to_string())).unwrap()
        ).await.unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap().to_string();
        assert!(!wf["locked"].as_bool().unwrap_or(false));

        // Lock it
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/lock"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id":"t1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let locked_wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(locked_wf["locked"].as_bool(), Some(true));

        let min_graph = json!({
            "workflow_version_id": "v1",
            "nodes": [{"id": "trigger-1", "type": "trigger"}],
            "edges": [],
            "input_schema": []
        });

        // Attempt to save a version — must be rejected (workflow is locked)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/versions"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id": "t1", "graph": min_graph}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Unlock it
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/unlock"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tenant_id":"t1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Now save should succeed
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/versions"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id": "t1", "graph": min_graph}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn workflow_visibility_set_and_filter() {
        let app = router();

        // Create a workflow via the API
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "vis-tenant",
                            "workspace_id": "ws-1",
                            "project_id": "proj-1",
                            "name": "Private WF"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap().to_string();
        assert_eq!(wf["visibility"], "tenant");

        // Set visibility to private
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/workflows/{wf_id}/visibility"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id": "vis-tenant", "visibility": "private"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(updated["visibility"], "private");

        // List should still include it (no auth in test mode — created_by is None, caller is None)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workflows?tenant_id=vis-tenant")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let found = list.as_array().unwrap().iter().any(|w| w["id"] == wf_id);
        assert!(
        found,
        "workflow with visibility=private still visible when created_by matches caller (both None)"
    );

        // Set back to tenant
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/workflows/{wf_id}/visibility"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id": "vis-tenant", "visibility": "tenant"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let restored: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored["visibility"], "tenant");
    }

    #[tokio::test]
    async fn set_visibility_rejects_invalid_value() {
        let app = router();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "vis2-tenant",
                            "workspace_id": "ws-1",
                            "project_id": "proj-1",
                            "name": "WF2"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/workflows/{wf_id}/visibility"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id": "vis2-tenant", "visibility": "public"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn rollback_creates_new_draft_version() {
        let app = router();
        // Create a workflow
        let create_body = json!({
            "tenant_id": "t-rollback",
            "workspace_id": "ws-1",
            "project_id": "proj-1",
            "name": "Rollback Test Workflow"
        });
        let resp = app
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
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wf_id = wf["id"].as_str().unwrap();

        // Create a version
        let version_body = json!({
            "tenant_id": "t-rollback",
            "graph": { "workflow_version_id": "v-rb", "nodes": [{"id": "n1", "type": "trigger"}], "edges": [] },
            "status": "draft",
            "message": "initial"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/versions"))
                    .header("content-type", "application/json")
                    .body(Body::from(version_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let version: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let version_id = version["id"].as_str().unwrap();

        // Rollback to that version
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/workflows/{wf_id}/rollback/{version_id}?tenant_id=t-rollback"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let rolled: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        // New version should be a draft with a rollback message
        assert_eq!(rolled["status"].as_str().unwrap(), "draft");
        let msg = rolled["message"].as_str().unwrap_or("");
        assert!(
            msg.contains("Rollback") || msg.contains("ollback"),
            "message should mention rollback: {msg}"
        );
    }

    // ── Slice 360: MCP server ─────────────────────────────────────────────────

    #[tokio::test]
    async fn run_test_case_returns_404_for_unknown_id() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/test-cases/nonexistent-tc-id/run")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── Slice 392: Notification center ────────────────────────────────────────
}
