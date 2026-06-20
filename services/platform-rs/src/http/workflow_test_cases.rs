// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Workflow test-case handlers.

use super::*;

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

pub(super) fn routes() -> Router<AppState> {
    Router::new()
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
