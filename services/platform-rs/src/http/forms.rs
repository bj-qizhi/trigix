// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn get_form_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let form = state.form_store.get(&token).await.map_err(|e| match e {
        FormError::NotFound => ApiError::not_found("form_token"),
        _ => ApiError::internal("form_store"),
    })?;
    Ok(Json(serde_json::json!({
        "token": form.token,
        "title": form.title,
        "description": form.description,
        "workflow_id": form.workflow_id,
        "input_schema": form.input_schema,
        "created_at": form.created_at,
    })))
}

async fn delete_form_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    state.form_store.delete(&token).await.map_err(|e| match e {
        FormError::NotFound => ApiError::not_found("form_token"),
        _ => ApiError::internal("form_store"),
    })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn submit_form_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(body): Json<FormSubmitBody>,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    let form = state.form_store.get(&token).await.map_err(|e| match e {
        FormError::NotFound => ApiError::not_found("form_token"),
        _ => ApiError::internal("form_store"),
    })?;
    let workflow = state
        .workflow_service
        .get_workflow(&form.tenant_id, &form.workflow_id)
        .await?;
    let version_id = workflow
        .latest_version_id
        .ok_or(WorkflowError::NoPublishedVersion)?;
    let version = state
        .workflow_service
        .get_version(&form.tenant_id, &version_id)
        .await?;
    let graph = resolve_graph_credentials(
        version.graph,
        &state.credential_store,
        &state.env_store,
        &form.tenant_id,
        DEFAULT_SET,
    )
    .await;
    let graph = inject_sub_workflow_graphs(
        graph,
        &state.workflow_service,
        &state.credential_store,
        &form.tenant_id,
    )
    .await;
    let record = state
        .execution_service
        .start(StartExecutionRequest {
            tenant_id: form.tenant_id,
            workflow_id: form.workflow_id,
            workflow_version_id: version_id,
            graph,
            input_json: body.input_json,
            label: Some(format!("form:{}", &token[..token.len().min(12)])),
            callback_url: None,
            trigger_type: Some("form".to_string()),
            dry_run: false,
            retried_from: None,
        })
        .await?;
    Ok((StatusCode::ACCEPTED, Json(record)))
}

// ── In-App Notification handlers ──────────────────────────────────────────────

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/forms/:token",
            get(get_form_handler).delete(delete_form_handler),
        )
        .route("/v1/forms/:token/submit", post(submit_form_handler))
}
