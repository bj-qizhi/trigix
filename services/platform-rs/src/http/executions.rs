// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn start_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut request): Json<StartExecutionRequest>,
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
    request.graph = resolve_graph_credentials(
        request.graph,
        &state.credential_store,
        &state.env_store,
        &request.tenant_id,
        DEFAULT_SET,
    )
    .await;
    request.graph = inject_sub_workflow_graphs(
        request.graph,
        &state.workflow_service,
        &state.credential_store,
        &request.tenant_id,
    )
    .await;
    let running = state
        .execution_service
        .count_all_running()
        .await
        .unwrap_or(0);
    if running >= max_concurrent_executions() {
        return Err(ApiError::bad_request(&format!(
            "Too many concurrent executions ({running}/{max}). Try again shortly.",
            max = max_concurrent_executions()
        )));
    }
    let tenant_running = state
        .execution_service
        .count_running_by_tenant(&request.tenant_id)
        .await
        .unwrap_or(0);
    let per_tenant_max = max_executions_per_tenant();
    if tenant_running >= per_tenant_max {
        return Err(ApiError::bad_request(&format!(
            "Tenant execution limit reached ({tenant_running}/{per_tenant_max} active). Try again when a run completes."
        )));
    }
    let prev_used = state
        .billing_store
        .billing_status(&request.tenant_id)
        .usage
        .executions_used;
    let record = state.execution_service.start(request).await?;
    info!(execution_id = %record.id, tenant_id = %record.tenant_id, "execution started");
    METRIC_EXEC_STARTED.fetch_add(1, Ordering::Relaxed);
    METRIC_EXEC_RUNNING.fetch_add(1, Ordering::Relaxed);
    state.billing_store.increment_execution(&record.tenant_id);
    spawn_quota_alert(&state, &record.tenant_id, prev_used);
    state.audit_store.record(
        &record.tenant_id,
        audit_action::EXECUTION_STARTED,
        "execution",
        &record.id,
        None,
    );
    fire_event(
        Arc::clone(&state.subscription_store),
        record.tenant_id.clone(),
        EVENT_EXECUTION_STARTED,
        serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "running"}),
    );
    match record.status {
        ExecutionStatus::Succeeded => fire_event(
            Arc::clone(&state.subscription_store),
            record.tenant_id.clone(),
            EVENT_EXECUTION_COMPLETED,
            serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "succeeded"}),
        ),
        ExecutionStatus::Failed => fire_event(
            Arc::clone(&state.subscription_store),
            record.tenant_id.clone(),
            EVENT_EXECUTION_FAILED,
            serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "failed"}),
        ),
        ExecutionStatus::Cancelled => fire_event(
            Arc::clone(&state.subscription_store),
            record.tenant_id.clone(),
            EVENT_EXECUTION_CANCELLED,
            serde_json::json!({"execution_id": &record.id, "workflow_id": &record.workflow_id, "status": "cancelled"}),
        ),
        _ => {}
    }
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn start_execution_batch(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<BatchStartBody>,
) -> Result<(StatusCode, Json<Vec<ExecutionRecord>>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    if body.requests.len() > 20 {
        return Err(ApiError::bad_request(
            "Batch size limited to 20 executions.",
        ));
    }
    let mut results = Vec::with_capacity(body.requests.len());
    for mut req in body.requests {
        req.tenant_id = effective_tenant_id(&claims, &req.tenant_id);
        req.graph = resolve_graph_credentials(
            req.graph,
            &state.credential_store,
            &state.env_store,
            &req.tenant_id,
            DEFAULT_SET,
        )
        .await;
        req.graph = inject_sub_workflow_graphs(
            req.graph,
            &state.workflow_service,
            &state.credential_store,
            &req.tenant_id,
        )
        .await;
        let record = state.execution_service.start(req).await?;
        METRIC_EXEC_STARTED.fetch_add(1, Ordering::Relaxed);
        state.audit_store.record(
            &record.tenant_id,
            audit_action::EXECUTION_STARTED,
            "execution",
            &record.id,
            None,
        );
        results.push(record);
    }
    Ok((StatusCode::ACCEPTED, Json(results)))
}

async fn list_executions(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<(axum::http::HeaderMap, Json<Vec<ExecutionSummary>>), ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let records = state.execution_service.list(&tenant_id).await?;
    let filtered: Vec<_> = records
        .into_iter()
        .filter(|r| {
            query
                .workflow_id
                .as_ref()
                .map_or(true, |id| &r.workflow_id == id)
        })
        .filter(|r| {
            query
                .label
                .as_ref()
                .map_or(true, |l| r.label.as_deref() == Some(l.as_str()))
        })
        .filter(|r| {
            query.status.as_ref().map_or(true, |s| {
                format!("{:?}", r.status).to_lowercase() == s.to_lowercase() ||
            // match the canonical string forms too
            matches!((&r.status, s.as_str()),
                (execution_core::ExecutionStatus::Running, "running") |
                (execution_core::ExecutionStatus::WaitingApproval, "waiting_approval") |
                (execution_core::ExecutionStatus::Succeeded, "succeeded") |
                (execution_core::ExecutionStatus::Failed, "failed") |
                (execution_core::ExecutionStatus::Cancelled, "cancelled")
            )
            })
        })
        .filter(|r| {
            query.search.as_ref().map_or(true, |s| {
                let s = s.to_lowercase();
                r.id.to_lowercase().starts_with(&s)
                    || r.label
                        .as_deref()
                        .map_or(false, |l| l.to_lowercase().contains(&s))
            })
        })
        .collect();
    let total = filtered.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100).min(500);
    let page = filtered.into_iter().skip(offset).take(limit).collect();
    let mut headers = axum::http::HeaderMap::new();
    if let Ok(v) = total.to_string().parse() {
        headers.insert("X-Total-Count", v);
    }
    Ok((headers, Json(page)))
}

/// SSE stream that pushes the full execution list every 2 seconds.
/// Clients connect once and receive `update` events with the serialised list.
/// The stream runs until the client disconnects; fixed-segment `/stream` takes
/// priority over the `/:execution_id` param route in Axum.
async fn list_executions_stream(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ListExecutionsQuery>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);

    tokio::spawn(async move {
        loop {
            let records = match state.execution_service.list(&tenant_id).await {
                Ok(r) => r,
                Err(_) => break,
            };
            let filtered: Vec<_> = records
                .into_iter()
                .filter(|r| {
                    query
                        .workflow_id
                        .as_ref()
                        .map_or(true, |id| &r.workflow_id == id)
                })
                .filter(|r| {
                    query
                        .label
                        .as_ref()
                        .map_or(true, |l| r.label.as_deref() == Some(l.as_str()))
                })
                .collect();

            if let Ok(data) = serde_json::to_string(&filtered) {
                if tx
                    .send(Ok(Event::default().event("update").data(data)))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

async fn get_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<Json<ExecutionRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let mut record = state
        .execution_service
        .get(&tenant_id, &execution_id)
        .await?;
    if record.status == ExecutionStatus::Running
        && state.approval_gate.is_waiting(&execution_id).await
    {
        record.status = ExecutionStatus::WaitingApproval;
    }
    Ok(Json(record))
}

async fn delete_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    // Only allow deleting terminal-state executions
    let record = state
        .execution_service
        .get(&tenant_id, &execution_id)
        .await?;
    if matches!(
        record.status,
        ExecutionStatus::Running | ExecutionStatus::WaitingApproval
    ) {
        return Err(ApiError::bad_request(
            "Cannot delete a running execution. Cancel it first.",
        ));
    }
    state
        .execution_service
        .delete(&tenant_id, &execution_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        "EXECUTION_DELETED",
        "execution",
        &execution_id,
        None,
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn patch_execution_label(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<PatchExecutionBody>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    state
        .execution_service
        .set_label(&tenant_id, &execution_id, body.label)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_execution_note_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<SetExecutionNoteBody>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    state
        .execution_service
        .set_note(&tenant_id, &execution_id, body.note.clone())
        .await?;
    Ok(Json(serde_json::json!({ "ok": true, "note": body.note })))
}

async fn star_execution_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .execution_service
        .set_starred(&tenant_id, &execution_id, true)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true, "starred": true })))
}

async fn unstar_execution_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<TenantQuery>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .execution_service
        .set_starred(&tenant_id, &execution_id, false)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true, "starred": false })))
}

async fn execution_events(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);

    tokio::spawn(async move {
        loop {
            let record = match state.execution_service.get(&tenant_id, &execution_id).await {
                Ok(mut r) => {
                    if r.status == ExecutionStatus::Running
                        && state.approval_gate.is_waiting(&execution_id).await
                    {
                        r.status = ExecutionStatus::WaitingApproval;
                    }
                    r
                }
                Err(_) => break,
            };

            let terminal = matches!(
                record.status,
                ExecutionStatus::Succeeded | ExecutionStatus::Failed | ExecutionStatus::Cancelled
            );

            if let Ok(data) = serde_json::to_string(&record) {
                if tx
                    .send(Ok(Event::default().event("update").data(data)))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            if terminal {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
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
    let comment_meta = body
        .comment
        .as_ref()
        .map(|c| serde_json::json!({ "comment": c }));
    state.audit_store.record(
        body.tenant_id.as_deref().unwrap_or(""),
        audit_action::EXECUTION_APPROVED,
        "execution",
        &execution_id,
        comment_meta,
    );
    Ok(Json(
        serde_json::json!({ "ok": true, "comment": body.comment }),
    ))
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
    let comment_meta = body
        .comment
        .as_ref()
        .map(|c| serde_json::json!({ "comment": c }));
    state.audit_store.record(
        body.tenant_id.as_deref().unwrap_or(""),
        audit_action::EXECUTION_REJECTED,
        "execution",
        &execution_id,
        comment_meta,
    );
    Ok(Json(
        serde_json::json!({ "ok": true, "comment": body.comment }),
    ))
}

async fn cancel_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<CancelExecutionBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    state
        .execution_service
        .cancel(&tenant_id, &execution_id)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "ExecutionNotFound".to_string(),
        })?;
    state.audit_store.record(
        &tenant_id,
        audit_action::EXECUTION_CANCELLED,
        "execution",
        &execution_id,
        None,
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn cancel_all_running_executions(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CancelExecutionBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let count = state
        .execution_service
        .cancel_all_running(&tenant_id)
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::EXECUTION_CANCELLED,
        "execution",
        "*",
        Some(serde_json::json!({ "bulk_cancel": count })),
    );
    Ok(Json(serde_json::json!({ "cancelled": count })))
}

async fn retry_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(execution_id): Path<String>,
    Json(body): Json<RetryExecutionBody>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let original = state
        .execution_service
        .get(&tenant_id, &execution_id)
        .await?;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: original.tenant_id.clone(),
            workflow_id: original.workflow_id.clone(),
            workflow_version_id: original.workflow_version_id.clone(),
            graph: original.graph.clone(),
            input_json: body
                .input_json
                .clone()
                .unwrap_or_else(|| original.input_json.clone()),
            label: body.label.clone().or_else(|| original.label.clone()),
            callback_url: None,
            trigger_type: Some("retry".to_string()),
            dry_run: false,
            retried_from: Some(execution_id.clone()),
        })
        .await?;
    state.audit_store.record(
        &tenant_id,
        audit_action::EXECUTION_RETRIED,
        "execution",
        &record.id,
        Some(serde_json::json!({ "retried_from": execution_id })),
    );
    Ok((StatusCode::CREATED, Json(record)))
}

async fn list_schedules(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<CredentialQuery>,
) -> Json<Vec<crate::scheduler::ScheduleSummary>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.schedule_store.list(&tenant_id))
}

async fn pause_schedule_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(version_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_write(&claims)?;
    let found = state.schedule_store.set_paused(&version_id, true);
    if found {
        Ok(Json(
            serde_json::json!({"ok": true, "paused": true, "workflow_version_id": version_id}),
        ))
    } else {
        Err(ApiError::not_found("schedule"))
    }
}

async fn resume_schedule_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(version_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_write(&claims)?;
    let found = state.schedule_store.set_paused(&version_id, false);
    if found {
        Ok(Json(
            serde_json::json!({"ok": true, "paused": false, "workflow_version_id": version_id}),
        ))
    } else {
        Err(ApiError::not_found("schedule"))
    }
}

async fn execution_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<ExecutionStats>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let all = state.execution_service.list(&tenant_id).await?;
    let mut stats = ExecutionStats {
        total: all.len() as u64,
        running: 0,
        waiting_approval: 0,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        by_trigger: std::collections::HashMap::new(),
        avg_duration_secs: None,
    };
    let mut duration_sum = 0u64;
    let mut duration_count = 0u64;
    for e in &all {
        match e.status {
            execution_core::ExecutionStatus::Running => stats.running += 1,
            execution_core::ExecutionStatus::WaitingApproval => stats.waiting_approval += 1,
            execution_core::ExecutionStatus::Succeeded => stats.succeeded += 1,
            execution_core::ExecutionStatus::Failed => stats.failed += 1,
            execution_core::ExecutionStatus::Cancelled => stats.cancelled += 1,
        }
        let trigger = e.trigger_type.as_deref().unwrap_or("manual").to_string();
        *stats.by_trigger.entry(trigger).or_insert(0) += 1;
        if let Some(fin) = e.finished_at {
            duration_sum += fin.saturating_sub(e.started_at);
            duration_count += 1;
        }
    }
    if duration_count > 0 {
        stats.avg_duration_secs = Some(duration_sum as f64 / duration_count as f64);
    }
    Ok(Json(stats))
}

// ── Node-type analytics ───────────────────────────────────────────────────

async fn cron_preview_handler(Json(req): Json<CronPreviewRequest>) -> impl IntoResponse {
    use chrono::Utc;
    use cron::Schedule;
    use std::str::FromStr;

    let count = req.count.min(10);
    match Schedule::from_str(&req.expression) {
        Err(e) => Json(CronPreviewResponse {
            next_times: vec![],
            error: Some(format!("Invalid expression: {e}")),
        }),
        Ok(schedule) => {
            let times: Vec<String> = schedule
                .upcoming(Utc)
                .take(count)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .collect();
            Json(CronPreviewResponse {
                next_times: times,
                error: None,
            })
        }
    }
}

// ── Test Cases ──────────────────────────────────────────────────────────────

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/executions", get(list_executions).post(start_execution))
        .route("/v1/executions/batch", post(start_execution_batch))
        .route("/v1/executions/stats", get(execution_stats_handler))
        .route("/v1/executions/stream", get(list_executions_stream))
        .route(
            "/v1/executions/cancel-running",
            post(cancel_all_running_executions),
        )
        .route(
            "/v1/executions/:execution_id",
            get(get_execution)
                .delete(delete_execution)
                .patch(patch_execution_label),
        )
        .route(
            "/v1/executions/:execution_id/note",
            post(set_execution_note_handler),
        )
        .route(
            "/v1/executions/:execution_id/star",
            post(star_execution_handler),
        )
        .route(
            "/v1/executions/:execution_id/unstar",
            post(unstar_execution_handler),
        )
        .route("/v1/executions/:execution_id/events", get(execution_events))
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
        .route("/v1/schedules", get(list_schedules))
        .route(
            "/v1/schedules/:version_id/pause",
            post(pause_schedule_handler),
        )
        .route(
            "/v1/schedules/:version_id/resume",
            post(resume_schedule_handler),
        )
        .route("/v1/cron/preview", post(cron_preview_handler))
}
