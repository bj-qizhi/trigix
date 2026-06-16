// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn healthz() -> &'static str {
    "ok"
}

async fn openapi_json() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&oa::spec()).unwrap_or_default(),
    )
}

async fn openapi_docs() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        oa::swagger_ui_html(),
    )
}

async fn healthz_detail(State(state): State<AppState>) -> Json<HealthDetail> {
    let database = state
        .workflow_service
        .list_workflows("__health__", None, None, None)
        .await
        .is_ok();
    let cache = state.cache.is_available();
    Json(HealthDetail {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        database,
        cache,
    })
}

/// Reads the SPA-facing captcha config: returns `(provider, site_key)` only when
/// a valid provider, a public site key, and the server-side secret are all set —
/// i.e. only when the SPA's token will actually be verified.
fn captcha_public_config() -> (Option<String>, Option<String>) {
    let provider = std::env::var("CAPTCHA_PROVIDER")
        .ok()
        .and_then(|p| crate::captcha::CaptchaProvider::parse(&p).map(|_| p.trim().to_lowercase()));
    let site_key = std::env::var("CAPTCHA_SITE_KEY")
        .ok()
        .filter(|s| !s.is_empty());
    let secret_set = std::env::var("CAPTCHA_SECRET")
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    match (provider, site_key, secret_set) {
        (Some(p), Some(k), true) => (Some(p), Some(k)),
        _ => (None, None),
    }
}

async fn system_info() -> Json<SystemInfo> {
    let (captcha_provider, captcha_site_key) = captcha_public_config();
    Json(SystemInfo {
        version: env!("CARGO_PKG_VERSION"),
        node_types: 138,
        auth_required: std::env::var("AUTH_REQUIRED").as_deref() == Ok("true"),
        captcha_provider,
        captcha_site_key,
        max_concurrent_executions: max_concurrent_executions(),
        max_executions_per_tenant: max_executions_per_tenant(),
        running_executions: METRIC_EXEC_RUNNING.load(Ordering::Relaxed),
        rust_edition: "2021",
        features: &[
            "jwt-auth",
            "webhook-signature",
            "sse-streaming",
            "parallel-fanout",
            "postgres-persistence",
            "cron-scheduling",
            "input-schema-validation",
            "token-usage-tracking",
            "named-env-sets",
            "per-tenant-quota",
            "distributed-scheduler-lock",
            "webhook-retry",
            "workflow-locking",
            "event-subscriptions",
            "rbac",
            "openapi-docs",
            "redis-streams-queue",
            "user-management",
            "stripe-billing",
        ],
    })
}

async fn queue_depth_handler(State(state): State<AppState>) -> Json<QueueDepthResponse> {
    let stream = crate::cache::keys::exec_queue_stream();
    let dead_stream = crate::cache::keys::exec_queue_dead_stream();
    let queue_depth = state.cache.xlen(stream).await;
    let dead_letter_depth = state.cache.xlen(dead_stream).await;
    Json(QueueDepthResponse {
        queue_depth,
        stream,
        dead_letter_depth,
        dead_letter_stream: dead_stream,
    })
}

async fn mcp_manifest() -> impl IntoResponse {
    let manifest = serde_json::json!({
        "schema_version": "v1",
        "name": "trigix",
        "description": "AI Agent Workflow Platform — run and manage workflows via MCP",
        "tools": [
            {
                "name": "list_workflows",
                "description": "List all published workflows available to the caller",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "tenant_id": { "type": "string", "description": "Tenant scope (optional)" }
                    }
                }
            },
            {
                "name": "execute_workflow",
                "description": "Execute a workflow by ID or name and return the execution record",
                "input_schema": {
                    "type": "object",
                    "required": ["workflow_id"],
                    "properties": {
                        "workflow_id": { "type": "string", "description": "Workflow ID or exact name" },
                        "input":       { "type": "object", "description": "Input data for the workflow" },
                        "tenant_id":   { "type": "string" }
                    }
                }
            }
        ]
    });
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&manifest).unwrap_or_default(),
    )
}

async fn mcp_execute_tool(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(req): Json<McpToolRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, req.tenant_id.as_deref().unwrap_or(""));
    match req.tool.as_str() {
        "list_workflows" => {
            let workflows = state
                .workflow_service
                .list_workflows(&tenant_id, None, Some("published"), Some(100))
                .await?;
            let items: Vec<serde_json::Value> = workflows
                .iter()
                .map(|w| {
                    serde_json::json!({
                        "id": w.id,
                        "name": w.name,
                        "description": w.description,
                        "tags": w.tags,
                    })
                })
                .collect();
            Ok(Json(serde_json::json!({ "workflows": items })))
        }
        "execute_workflow" => {
            let input = req.input.as_ref();
            let workflow_id = input
                .and_then(|v| v.get("workflow_id"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::bad_request("input.workflow_id is required"))?;
            let input_json = input
                .and_then(|v| v.get("input"))
                .map(|v| v.to_string())
                .unwrap_or_else(|| "{}".to_string());
            state
                .billing_store
                .check_execution_quota(&tenant_id)
                .map_err(|e| ApiError {
                    status: StatusCode::PAYMENT_REQUIRED,
                    message: e,
                })?;
            let workflow = state
                .workflow_service
                .get_workflow(&tenant_id, workflow_id)
                .await
                .or_else(|_| {
                    // Fallback: search by name is not supported in one step; propagate original error
                    Err(ApiError::not_found(&format!(
                        "workflow not found: {workflow_id}"
                    )))
                })?;
            let version_id = workflow
                .latest_version_id
                .ok_or(WorkflowError::NoPublishedVersion)?;
            let version = state
                .workflow_service
                .get_version(&tenant_id, &version_id)
                .await?;
            let graph = resolve_graph_credentials(
                version.graph,
                &state.credential_store,
                &state.env_store,
                &tenant_id,
                DEFAULT_SET,
            )
            .await;
            let record = state
                .execution_service
                .start(StartExecutionRequest {
                    tenant_id: tenant_id.clone(),
                    workflow_id: workflow.id,
                    workflow_version_id: version_id,
                    graph,
                    input_json,
                    label: Some("mcp".to_string()),
                    callback_url: None,
                    trigger_type: Some("mcp".to_string()),
                    dry_run: false,
                    retried_from: None,
                })
                .await?;
            let prev_used = state
                .billing_store
                .billing_status(&tenant_id)
                .usage
                .executions_used;
            state.billing_store.increment_execution(&tenant_id);
            spawn_quota_alert(&state, &tenant_id, prev_used);
            state.audit_store.record(
                &tenant_id,
                "execution.started.mcp",
                "execution",
                &record.id,
                None,
            );
            Ok(Json(serde_json::json!({
                "execution_id": record.id,
                "status": format!("{:?}", record.status).to_lowercase(),
                "workflow_id": record.workflow_id,
            })))
        }
        other => Err(ApiError::bad_request(&format!(
            "Unknown tool: {other}. Available: list_workflows, execute_workflow"
        ))),
    }
}

// ── Global search ─────────────────────────────────────────────────────────────

async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<SearchResult>, ApiError> {
    let supplied = params.tenant_id.unwrap_or_default();
    let tenant_id = effective_tenant_id(&claims, &supplied);
    let q = params.q.unwrap_or_default().to_lowercase();
    if q.is_empty() {
        return Ok(Json(SearchResult {
            workflows: vec![],
            executions: vec![],
        }));
    }

    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .map_err(|_| ApiError::bad_request("workflow list failed"))?;
    let wf_hits: Vec<WorkflowSearchHit> = workflows
        .into_iter()
        .filter(|w| {
            w.name.to_lowercase().contains(&q)
                || w.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
                || w.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .take(10)
        .map(|w| WorkflowSearchHit {
            id: w.id,
            name: w.name,
            status: w.status,
            description: w.description,
        })
        .collect();

    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .map_err(|_| ApiError::bad_request("execution list failed"))?;
    let exec_hits: Vec<ExecutionSearchHit> = executions
        .into_iter()
        .filter(|e| {
            e.id.starts_with(&q) || e.label.as_deref().unwrap_or("").to_lowercase().contains(&q)
        })
        .take(10)
        .map(|e| ExecutionSearchHit {
            id: e.id,
            workflow_id: e.workflow_id,
            status: format!("{:?}", e.status).to_lowercase(),
            label: e.label,
        })
        .collect();

    Ok(Json(SearchResult {
        workflows: wf_hits,
        executions: exec_hits,
    }))
}

async fn metrics_handler() -> axum::response::Response {
    let mut out = String::with_capacity(512);

    let started = METRIC_EXEC_STARTED.load(Ordering::Relaxed);
    let succeeded = METRIC_EXEC_SUCCEEDED.load(Ordering::Relaxed);
    let failed = METRIC_EXEC_FAILED.load(Ordering::Relaxed);
    let cancelled = METRIC_EXEC_CANCELLED.load(Ordering::Relaxed);
    let running = METRIC_EXEC_RUNNING.load(Ordering::Relaxed);
    let requests = METRIC_REQUESTS.load(Ordering::Relaxed);
    let max_concurrent = max_concurrent_executions();

    out.push_str("# HELP af_executions_started_total Executions started since server start\n");
    out.push_str("# TYPE af_executions_started_total counter\n");
    out.push_str(&format!("af_executions_started_total {started}\n"));
    out.push_str("# HELP af_executions_completed_total Executions completed by outcome\n");
    out.push_str("# TYPE af_executions_completed_total counter\n");
    out.push_str(&format!(
        "af_executions_completed_total{{outcome=\"succeeded\"}} {succeeded}\n"
    ));
    out.push_str(&format!(
        "af_executions_completed_total{{outcome=\"failed\"}} {failed}\n"
    ));
    out.push_str(&format!(
        "af_executions_completed_total{{outcome=\"cancelled\"}} {cancelled}\n"
    ));
    out.push_str("# HELP af_executions_running Current number of in-flight executions\n");
    out.push_str("# TYPE af_executions_running gauge\n");
    out.push_str(&format!("af_executions_running {running}\n"));
    out.push_str(
        "# HELP af_executions_max_concurrent Configured max concurrent executions limit\n",
    );
    out.push_str("# TYPE af_executions_max_concurrent gauge\n");
    out.push_str(&format!("af_executions_max_concurrent {max_concurrent}\n"));
    let per_tenant_max = max_executions_per_tenant();
    out.push_str("# HELP af_executions_max_per_tenant Per-tenant concurrent execution limit\n");
    out.push_str("# TYPE af_executions_max_per_tenant gauge\n");
    out.push_str(&format!("af_executions_max_per_tenant {per_tenant_max}\n"));
    out.push_str("# HELP af_http_requests_total HTTP requests handled since server start\n");
    out.push_str("# TYPE af_http_requests_total counter\n");
    out.push_str(&format!("af_http_requests_total {requests}\n"));
    let dlq = METRIC_DLQ_TOTAL.load(Ordering::Relaxed);
    out.push_str(
        "# HELP af_dlq_messages_total Jobs routed to the dead-letter stream since server start\n",
    );
    out.push_str("# TYPE af_dlq_messages_total counter\n");
    out.push_str(&format!("af_dlq_messages_total {dlq}\n"));

    axum::response::Response::builder()
        .status(200)
        .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
        .body(axum::body::Body::from(out))
        .unwrap()
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/healthz/detail", get(healthz_detail))
        .route("/openapi.json", get(openapi_json))
        .route("/docs", get(openapi_docs))
        .route("/v1/system/info", get(system_info))
        .route("/v1/system/queue-depth", get(queue_depth_handler))
        .route("/v1/search", get(search_handler))
        .route("/metrics", get(metrics_handler))
        .route("/.well-known/mcp.json", get(mcp_manifest))
        .route("/v1/mcp/tools", post(mcp_execute_tool))
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
    async fn system_info_returns_expected_fields() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/system/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let info: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(info["version"].is_string());
        assert!(info["node_types"].as_u64().unwrap() > 0);
        assert!(info["features"].is_array());
        assert!(info["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str() == Some("jwt-auth")));
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_prometheus_text() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(ct.contains("text/plain"));
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body = std::str::from_utf8(&bytes).unwrap();
        assert!(body.contains("af_executions_started_total"));
        assert!(body.contains("af_http_requests_total"));
    }

    #[tokio::test]
    async fn openapi_json_returns_valid_spec() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let spec: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(spec["openapi"], "3.0.3");
        assert!(spec["paths"].as_object().unwrap().len() > 20);
    }

    #[tokio::test]
    async fn openapi_docs_returns_html() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/docs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("swagger-ui"));
        assert!(body.contains("/openapi.json"));
    }

    // ── User auth HTTP tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn mcp_manifest_returns_json() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/.well-known/mcp.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v
            .get("tools")
            .and_then(|t| t.as_array())
            .map(|a| a.len() >= 2)
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn mcp_list_workflows_tool() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/mcp/tools")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tool": "list_workflows",
                            "tenant_id": "tenant-1"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.get("workflows").and_then(|w| w.as_array()).is_some());
    }

    #[tokio::test]
    async fn mcp_unknown_tool_returns_400() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/mcp/tools")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"tool": "nonexistent_tool"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn request_id_header_present_on_response() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            resp.headers().contains_key("x-request-id"),
            "x-request-id header missing from response"
        );
        let id = resp
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(id.len(), 36, "request ID should be a 36-char UUID");
    }

    #[tokio::test]
    async fn security_headers_present_on_response() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let headers = resp.headers();
        assert_eq!(
            headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
            Some("DENY")
        );
        assert_eq!(
            headers
                .get("x-content-type-options")
                .and_then(|v| v.to_str().ok()),
            Some("nosniff")
        );
        assert!(
            headers.contains_key("strict-transport-security"),
            "missing HSTS header"
        );
        assert!(
            headers.contains_key("referrer-policy"),
            "missing referrer-policy header"
        );
    }

    #[tokio::test]
    async fn request_ids_are_unique_across_requests() {
        let r1 = router()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let r2 = router()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let id1 = r1
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let id2 = r2
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_ne!(id1, id2, "each request should get a unique request ID");
    }
}
