// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Per-workflow persistent variable handlers.

use super::*;

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

pub(super) fn routes() -> Router<AppState> {
    Router::new()
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
}
