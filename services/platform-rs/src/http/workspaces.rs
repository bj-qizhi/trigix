// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn list_workspaces_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<crate::workspace::WorkspaceRecord>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.workspace_store.list_workspaces(&tenant_id).await)
}

async fn create_workspace_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CreateWorkspaceBody>,
) -> (StatusCode, Json<crate::workspace::WorkspaceRecord>) {
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let record = state
        .workspace_store
        .create_workspace(&body.tenant_id, &body.name, body.description.as_deref())
        .await;
    (StatusCode::CREATED, Json(record))
}

async fn delete_workspace_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workspace_id): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> StatusCode {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state
        .workspace_store
        .delete_workspace(&tenant_id, &workspace_id)
        .await
    {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn list_projects_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workspace_id): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<crate::workspace::ProjectRecord>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(
        state
            .workspace_store
            .list_projects(&tenant_id, &workspace_id)
            .await,
    )
}

async fn create_project_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(workspace_id): Path<String>,
    Json(mut body): Json<CreateProjectBody>,
) -> Result<(StatusCode, Json<crate::workspace::ProjectRecord>), ApiError> {
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let record = state
        .workspace_store
        .create_project(
            &body.tenant_id,
            &workspace_id,
            &body.name,
            body.description.as_deref(),
        )
        .await
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "WorkspaceNotFound".to_string(),
        })?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn delete_project_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(project_id): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> StatusCode {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state
        .workspace_store
        .delete_project(&tenant_id, &project_id)
        .await
    {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/workspaces",
            get(list_workspaces_handler).post(create_workspace_handler),
        )
        .route(
            "/v1/workspaces/:workspace_id",
            get(method_not_allowed).delete(delete_workspace_handler),
        )
        .route(
            "/v1/workspaces/:workspace_id/projects",
            get(list_projects_handler).post(create_project_handler),
        )
        .route(
            "/v1/projects/:project_id",
            get(method_not_allowed).delete(delete_project_handler),
        )
}
