// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use workflow_core::{GraphError, WorkflowGraph};

use crate::executor::DispatchingNodeExecutor;
use crate::runtime::{run_workflow, ExecutionReport, RuntimeError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunExecutionRequest {
    pub execution_id: String,
    pub graph: WorkflowGraph,
    pub input_json: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone)]
struct AppState {
    executor: Arc<DispatchingNodeExecutor>,
}

pub fn router() -> Router {
    let ai_runtime_base_url = std::env::var("AI_RUNTIME_BASE_URL").ok();
    router_with_config(ai_runtime_base_url)
}

pub fn router_with_config(ai_runtime_base_url: Option<String>) -> Router {
    let state = AppState {
        executor: Arc::new(DispatchingNodeExecutor::new(ai_runtime_base_url)),
    };

    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/executions:run", post(run_execution))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn run_execution(
    State(state): State<AppState>,
    Json(request): Json<RunExecutionRequest>,
) -> Result<Json<ExecutionReport>, ApiError> {
    validate_request(&request)?;

    let mut node_executor = (*state.executor).clone();
    let report = run_workflow(
        request.execution_id,
        &request.graph,
        request.input_json,
        &mut node_executor,
        request.dry_run,
    )
    .await?;

    Ok(Json(report))
}

fn validate_request(request: &RunExecutionRequest) -> Result<(), ApiError> {
    if request.execution_id.is_empty() {
        return Err(ApiError::bad_request("MissingExecution"));
    }
    if request.input_json.is_empty() {
        return Err(ApiError::bad_request("MissingInput"));
    }
    serde_json::from_str::<serde_json::Value>(&request.input_json)
        .map_err(|_| ApiError::bad_request("InvalidInput"))?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl From<RuntimeError> for ApiError {
    fn from(error: RuntimeError) -> Self {
        let message = match error {
            RuntimeError::InvalidGraph(graph_error) => graph_error_message(graph_error),
            RuntimeError::MissingNode(node_id) => format!("MissingNode({node_id})"),
        };

        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

fn graph_error_message(error: GraphError) -> String {
    match error {
        GraphError::MissingWorkflowVersion => "MissingWorkflowVersion".to_string(),
        GraphError::EmptyGraph => "EmptyGraph".to_string(),
        GraphError::EmptyNodeId => "EmptyNodeId".to_string(),
        GraphError::DuplicateNode(node_id) => format!("DuplicateNode({node_id})"),
        GraphError::UnknownEdgeSource(node_id) => format!("UnknownEdgeSource({node_id})"),
        GraphError::UnknownEdgeTarget(node_id) => format!("UnknownEdgeTarget({node_id})"),
        GraphError::CycleDetected => "CycleDetected".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use execution_core::ExecutionStatus;
    use serde_json::json;
    use tower::ServiceExt;

    fn test_router() -> Router {
        router_with_config(None)
    }

    #[tokio::test]
    async fn runs_trigger_only_execution_over_http() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions:run")
                    .header("content-type", "application/json")
                    .body(Body::from(trigger_only_request().to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: ExecutionReport = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload.execution_id, "execution-1");
        assert_eq!(payload.status, ExecutionStatus::Succeeded);
        assert_eq!(payload.node_results.len(), 1);
        assert_eq!(payload.node_results[0].node_id, "trigger");
        // trigger node returns workflow input
        assert_eq!(
            payload.node_results[0].output_json.as_deref(),
            Some(r#"{"lead_id":"lead-1"}"#)
        );
    }

    #[tokio::test]
    async fn agent_node_fails_without_ai_runtime_over_http() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions:run")
                    .header("content-type", "application/json")
                    .body(Body::from(valid_request().to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: ExecutionReport = serde_json::from_slice(&body).unwrap();

        // execution fails at agent node because AI Runtime URL is not configured
        assert_eq!(payload.status, ExecutionStatus::Failed);
        assert_eq!(payload.node_results[0].node_id, "trigger");
        assert_eq!(payload.node_results[1].node_id, "agent");
        assert!(payload.node_results[1]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("AI_RUNTIME_BASE_URL"));
    }

    #[tokio::test]
    async fn rejects_invalid_graph_over_http() {
        let request = json!({
            "execution_id": "execution-1",
            "graph": {
                "workflow_version_id": "version-1",
                "nodes": [
                    {"id": "a", "type": "http"},
                    {"id": "b", "type": "agent"}
                ],
                "edges": [
                    {"source": "a", "target": "b"},
                    {"source": "b", "target": "a"}
                ]
            },
            "input_json": "{}"
        });

        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/executions:run")
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    fn trigger_only_request() -> serde_json::Value {
        json!({
            "execution_id": "execution-1",
            "graph": {
                "workflow_version_id": "version-1",
                "nodes": [{"id": "trigger", "type": "trigger"}],
                "edges": []
            },
            "input_json": "{\"lead_id\":\"lead-1\"}"
        })
    }

    fn valid_request() -> serde_json::Value {
        json!({
            "execution_id": "execution-1",
            "graph": {
                "workflow_version_id": "version-1",
                "nodes": [
                    {"id": "trigger", "type": "trigger"},
                    {"id": "agent", "type": "agent"}
                ],
                "edges": [
                    {"source": "trigger", "target": "agent"}
                ]
            },
            "input_json": "{\"lead_id\":\"lead-1\"}"
        })
    }
}
