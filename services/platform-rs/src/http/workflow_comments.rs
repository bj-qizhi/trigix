// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Workflow comment handlers.

use super::*;

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

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/workflows/:workflow_id/comments",
            get(list_workflow_comments_handler).post(create_workflow_comment_handler),
        )
        .route(
            "/v1/comments/:comment_id",
            patch(edit_workflow_comment_handler).delete(delete_workflow_comment_handler),
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
}
