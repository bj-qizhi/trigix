// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::sync::Arc;

use std::convert::Infallible;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use workflow_core::{GraphError, WorkflowGraph};

use crate::executor::DispatchingNodeExecutor;
use crate::runtime::{
    run_workflow, run_workflow_with_progress, ExecutionReport, NodeProgressCallback, NodeReport,
    RuntimeError, TokenSink, TOKEN_SINK,
};

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
        .route("/v1/run-stream", post(run_execution_stream))
        .with_state(state)
}

/// Progress callback that forwards each completed node onto the SSE channel so a
/// remote platform can observe the run live (mirrors the platform's inline
/// StoreProgressCallback, but over the wire).
struct ChannelProgress {
    tx: mpsc::Sender<Result<SseEvent, Infallible>>,
}

impl NodeProgressCallback for ChannelProgress {
    fn on_node_complete(&self, report: &NodeReport) {
        if let Ok(report_json) = serde_json::to_string(report) {
            let data = format!(r#"{{"kind":"node","report":{report_json}}}"#);
            let _ = self
                .tx
                .try_send(Ok(SseEvent::default().event("node").data(data)));
        }
    }
}

/// Streaming twin of `run_execution`: emits `data:{"kind":"node"|"token"|
/// "report"|"error", …}` SSE frames so a separate/queue-mode deployment gets the
/// same live node + token updates the inline executor already provides. The
/// buffered `:run` endpoint is unchanged; the platform falls back to it if this
/// stream can't be reached.
async fn run_execution_stream(
    State(state): State<AppState>,
    Json(request): Json<RunExecutionRequest>,
) -> Sse<ReceiverStream<Result<SseEvent, Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<SseEvent, Infallible>>(256);
    let node_executor = (*state.executor).clone();

    tokio::spawn(async move {
        if let Err(err) = validate_request(&request) {
            let data = serde_json::json!({ "kind": "error", "message": err.message }).to_string();
            let _ = tx.try_send(Ok(SseEvent::default().event("error").data(data)));
            return;
        }

        let progress = ChannelProgress { tx: tx.clone() };
        let sink_tx = tx.clone();
        let sink: TokenSink = std::sync::Arc::new(move |node_id: &str, delta: &str| {
            let data = serde_json::json!({ "kind": "token", "node_id": node_id, "delta": delta })
                .to_string();
            let _ = sink_tx.try_send(Ok(SseEvent::default().event("token").data(data)));
        });

        let result = TOKEN_SINK
            .scope(
                Some(sink),
                run_workflow_with_progress(
                    request.execution_id,
                    &request.graph,
                    request.input_json,
                    &node_executor,
                    &progress,
                    request.dry_run,
                ),
            )
            .await;

        let data = match result {
            Ok(report) => match serde_json::to_string(&report) {
                Ok(report_json) => format!(r#"{{"kind":"report","report":{report_json}}}"#),
                Err(_) => r#"{"kind":"error","message":"SerializeReport"}"#.to_string(),
            },
            Err(err) => {
                let message = ApiError::from(err).message;
                serde_json::json!({ "kind": "error", "message": message }).to_string()
            }
        };
        let event = if data.contains(r#""kind":"report""#) {
            "report"
        } else {
            "error"
        };
        let _ = tx.try_send(Ok(SseEvent::default().event(event).data(data)));
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn run_execution(
    State(state): State<AppState>,
    Json(request): Json<RunExecutionRequest>,
) -> Result<Json<ExecutionReport>, ApiError> {
    validate_request(&request)?;

    let node_executor = (*state.executor).clone();
    let report = run_workflow(
        request.execution_id,
        &request.graph,
        request.input_json,
        &node_executor,
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

    #[tokio::test]
    async fn streaming_endpoint_emits_node_then_report_events() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/run-stream")
                    .header("content-type", "application/json")
                    .body(Body::from(trigger_only_request().to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        // The trigger node's completion is streamed, then the final report.
        assert!(
            text.contains(r#""kind":"node""#),
            "missing node event: {text}"
        );
        assert!(
            text.contains(r#""node_id":"trigger""#),
            "missing trigger node: {text}"
        );
        assert!(
            text.contains(r#""kind":"report""#),
            "missing report event: {text}"
        );
        // ExecutionStatus serialises lowercase; the run should have succeeded.
        assert!(
            text.contains(r#""status":"succeeded""#),
            "report should be succeeded: {text}"
        );
    }

    #[tokio::test]
    async fn streaming_endpoint_reports_invalid_input() {
        let bad = json!({
            "execution_id": "execution-1",
            "graph": trigger_only_request()["graph"].clone(),
            "input_json": ""
        });
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/run-stream")
                    .header("content-type", "application/json")
                    .body(Body::from(bad.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // The stream itself opens (200) and carries an error frame.
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains(r#""kind":"error""#),
            "missing error event: {text}"
        );
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
