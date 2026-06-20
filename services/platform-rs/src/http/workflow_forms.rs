// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Public form publishing handlers.

use super::*;

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
        .route(
            "/v1/workflows/:workflow_id/publish-form",
            get(method_not_allowed).post(publish_form_handler),
        )
        .route("/v1/workflows/:workflow_id/forms", get(list_forms_handler))
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
}
