// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::time::Duration;

use execution_core::{current_tenant_id, ExecutionContext, NodeExecutionResult};
use serde::{Deserialize, Serialize};
use workflow_core::{Node, NodeType};

#[derive(Clone)]
pub struct BrowserRuntimeClient {
    base_url: String,
    auth_token: String,
    client: reqwest::Client,
    poll_interval: Duration,
}

#[derive(Debug, Serialize)]
struct CreateTaskRequest<'a> {
    tenant_id: &'a str,
    workflow_id: &'a str,
    execution_id: &'a str,
    node_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    timeout_ms: u64,
    actions: Vec<BrowserAction>,
}

#[derive(Debug, Serialize)]
struct CreateSessionRequest<'a> {
    tenant_id: &'a str,
    execution_id: &'a str,
}

#[derive(Debug, Serialize)]
struct BrowserAction {
    #[serde(rename = "type")]
    action_type: &'static str,
    params: serde_json::Value,
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
struct CreatedTask {
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct BrowserSession {
    id: String,
}

#[derive(Debug, Deserialize)]
struct BrowserTask {
    status: String,
    result: Option<serde_json::Value>,
    error: Option<BrowserTaskError>,
}

#[derive(Debug, Deserialize)]
struct BrowserTaskError {
    code: String,
    message: String,
}

impl BrowserRuntimeClient {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("BROWSER_RUNTIME_BASE_URL").ok()?;
        let auth_token = std::env::var("BROWSER_RUNTIME_AUTH_TOKEN").ok()?;
        if base_url.trim().is_empty() || auth_token.len() < 32 {
            return None;
        }
        let request_timeout = std::env::var("BROWSER_RUNTIME_REQUEST_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(65_000);
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_millis(request_timeout))
            .build()
            .ok()?;
        Some(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token,
            client,
            poll_interval: Duration::from_millis(100),
        })
    }

    async fn create_session(&self, tenant_id: &str, execution_id: &str) -> Result<String, String> {
        let response = self
            .request(reqwest::Method::POST, "/v1/sessions", tenant_id)
            .json(&CreateSessionRequest {
                tenant_id,
                execution_id,
            })
            .send()
            .await
            .map_err(runtime_unavailable)?;
        parse_success::<BrowserSession>(response)
            .await
            .map(|session| session.id)
    }

    async fn close_session(
        &self,
        tenant_id: &str,
        session_id: &str,
    ) -> Result<serde_json::Value, String> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("/v1/sessions/{session_id}"),
                tenant_id,
            )
            .send()
            .await
            .map_err(runtime_unavailable)?;
        parse_success(response).await
    }

    async fn create_task(&self, request: &CreateTaskRequest<'_>) -> Result<String, String> {
        let response = self
            .request(reqwest::Method::POST, "/v1/tasks", request.tenant_id)
            .json(request)
            .send()
            .await
            .map_err(runtime_unavailable)?;
        parse_success::<CreatedTask>(response)
            .await
            .map(|task| task.task_id)
    }

    async fn get_task(&self, tenant_id: &str, task_id: &str) -> Result<BrowserTask, String> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/v1/tasks/{task_id}"),
                tenant_id,
            )
            .send()
            .await
            .map_err(runtime_unavailable)?;
        parse_success(response).await
    }

    async fn cancel_task(&self, tenant_id: &str, task_id: &str) {
        let _ = self
            .request(
                reqwest::Method::DELETE,
                &format!("/v1/tasks/{task_id}"),
                tenant_id,
            )
            .send()
            .await;
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        tenant_id: &str,
    ) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(&self.auth_token)
            .header("x-trigix-tenant-id", tenant_id)
    }
}

struct CancelTaskOnDrop {
    client: BrowserRuntimeClient,
    tenant_id: String,
    task_id: Option<String>,
}

impl CancelTaskOnDrop {
    fn disarm(&mut self) {
        self.task_id = None;
    }
}

impl Drop for CancelTaskOnDrop {
    fn drop(&mut self) {
        let Some(task_id) = self.task_id.take() else {
            return;
        };
        let tenant_id = self.tenant_id.clone();
        let client = self.client.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                client.cancel_task(&tenant_id, &task_id).await;
            });
        }
    }
}

pub async fn execute_browser_node(
    node: &Node,
    context: &ExecutionContext,
    client: Option<&BrowserRuntimeClient>,
    resolved_config: serde_json::Value,
) -> NodeExecutionResult {
    let Some(client) = client else {
        return NodeExecutionResult::failed(
            "BROWSER_RUNTIME_UNAVAILABLE: runtime is not configured",
        );
    };
    let Some(tenant_id) = current_tenant_id() else {
        return NodeExecutionResult::failed(
            "BROWSER_UNAUTHORIZED: execution has no authenticated Tenant context",
        );
    };
    if node.node_type == NodeType::BrowserStart {
        return match client
            .create_session(&tenant_id, &context.execution_id)
            .await
        {
            Ok(session_id) => NodeExecutionResult::succeeded(
                serde_json::json!({"browser": {"session_id": session_id}}).to_string(),
            ),
            Err(error) => NodeExecutionResult::failed(error),
        };
    }
    let session_id = resolved_config
        .get("session_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if session_id.is_empty() {
        return NodeExecutionResult::failed("BROWSER_INVALID_REQUEST: session_id is required");
    }
    if node.node_type == NodeType::BrowserClose {
        return match client.close_session(&tenant_id, session_id).await {
            Ok(_) => NodeExecutionResult::succeeded(
                serde_json::json!({"browser": {"session_id": session_id, "closed": true}})
                    .to_string(),
            ),
            Err(error) => NodeExecutionResult::failed(error),
        };
    }
    let Some(action_type) = action_type(&node.node_type) else {
        return NodeExecutionResult::failed("BROWSER_INVALID_REQUEST: unsupported Browser Node");
    };
    let timeout_ms = resolved_config
        .get("timeout_ms")
        .and_then(|value| value.as_u64())
        .unwrap_or(10_000)
        .clamp(1, 60_000);
    let mut params = resolved_config.as_object().cloned().unwrap_or_default();
    for key in [
        "session_id",
        "timeout_ms",
        "max_retries",
        "retry_delay_ms",
        "cache_ttl_secs",
        "node_label",
        "timeout_secs",
    ] {
        params.remove(key);
    }
    if action_type == "wait" {
        let mode = params
            .remove("wait_mode")
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "selector".to_string());
        params.retain(|key, _| match mode.as_str() {
            "milliseconds" => key == "milliseconds",
            "url" => key == "url",
            "load_state" => key == "load_state",
            _ => key == "selector" || key == "state",
        });
    }
    let request = CreateTaskRequest {
        tenant_id: &tenant_id,
        workflow_id: &context.workflow_version_id,
        execution_id: &context.execution_id,
        node_id: &node.id,
        session_id: Some(session_id),
        timeout_ms,
        actions: vec![BrowserAction {
            action_type,
            params: serde_json::Value::Object(params),
            timeout_ms,
        }],
    };
    let task_id = match client.create_task(&request).await {
        Ok(task_id) => task_id,
        Err(error) => return NodeExecutionResult::failed(error),
    };
    let mut guard = CancelTaskOnDrop {
        client: client.clone(),
        tenant_id: tenant_id.clone(),
        task_id: Some(task_id.clone()),
    };
    loop {
        match client.get_task(&tenant_id, &task_id).await {
            Ok(task) => match task.status.as_str() {
                "completed" => {
                    guard.disarm();
                    let result = task.result.unwrap_or_else(|| serde_json::json!({}));
                    let action_data = result
                        .get("actions")
                        .and_then(|actions| actions.as_array())
                        .and_then(|actions| actions.last())
                        .and_then(|action| action.get("data"))
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let artifact_url = action_data
                        .get("id")
                        .and_then(|value| value.as_str())
                        .map(|artifact_id| format!("/v1/browser/artifacts/{artifact_id}"));
                    return NodeExecutionResult::succeeded(serde_json::json!({"browser": {
                        "session_id": session_id,
                        "task_id": task_id,
                        "result": action_data,
                        "duration_ms": result.get("duration_ms").cloned().unwrap_or(serde_json::Value::Null),
                        "url": result.get("final_url").cloned().unwrap_or(serde_json::Value::Null),
                        "title": result.get("title").cloned().unwrap_or(serde_json::Value::Null),
                        "artifact_url": artifact_url,
                    }}).to_string());
                }
                "failed" | "timeout" | "cancelled" => {
                    guard.disarm();
                    let error = task
                        .error
                        .map(|error| format!("{}: {}", error.code, error.message))
                        .unwrap_or_else(|| {
                            format!("BROWSER_ACTION_FAILED: task {task_id} {}", task.status)
                        });
                    return NodeExecutionResult::failed(error);
                }
                _ => tokio::time::sleep(client.poll_interval).await,
            },
            Err(error) => return NodeExecutionResult::failed(error),
        }
    }
}

fn action_type(node_type: &NodeType) -> Option<&'static str> {
    match node_type {
        NodeType::BrowserNavigate => Some("navigate"),
        NodeType::BrowserClick => Some("click"),
        NodeType::BrowserInput => Some("input"),
        NodeType::BrowserWait => Some("wait"),
        NodeType::BrowserExtract => Some("extract"),
        NodeType::BrowserScreenshot => Some("screenshot"),
        _ => None,
    }
}

async fn parse_success<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, String> {
    let status = response.status();
    let body = response.text().await.map_err(runtime_unavailable)?;
    if !status.is_success() {
        let value: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let code = value
            .pointer("/error/code")
            .and_then(|value| value.as_str())
            .unwrap_or("BROWSER_INTERNAL_ERROR");
        let message = value
            .pointer("/error/message")
            .and_then(|value| value.as_str())
            .unwrap_or("Browser Runtime rejected the request");
        return Err(format!("{code}: {message}"));
    }
    serde_json::from_str(&body)
        .map_err(|_| "BROWSER_INTERNAL_ERROR: invalid runtime response".to_string())
}

fn runtime_unavailable(error: impl std::fmt::Display) -> String {
    format!("BROWSER_RUNTIME_UNAVAILABLE: {error}")
}
