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

#[cfg(test)]
mod tests {
    use crate::http::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn workspace_crud_over_http() {
        let app = router();

        // Create workspace
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workspaces")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "tenant_id": "tenant-1", "name": "Engineering" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let ws: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let ws_id = ws["id"].as_str().unwrap().to_string();
        assert_eq!(ws["name"], "Engineering");

        // List workspaces
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workspaces?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(list.as_array().unwrap().iter().any(|w| w["id"] == ws_id));

        // Create project in workspace
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workspaces/{ws_id}/projects"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "tenant_id": "tenant-1", "name": "Backend" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let proj: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let proj_id = proj["id"].as_str().unwrap().to_string();
        assert_eq!(proj["name"], "Backend");

        // List projects
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/workspaces/{ws_id}/projects?tenant_id=tenant-1"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let projects: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(projects.as_array().unwrap().len(), 1);
        assert_eq!(projects[0]["id"], proj_id);

        // Delete project
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/projects/{proj_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Delete workspace
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/workspaces/{ws_id}?tenant_id=tenant-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Workspace gone
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/workspaces?tenant_id=tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(!list.as_array().unwrap().iter().any(|w| w["id"] == ws_id));
    }
}
