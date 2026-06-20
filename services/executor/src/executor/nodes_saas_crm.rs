// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! CRM / project-management integration nodes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

pub(super) async fn execute_intercom(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Intercom requires 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Intercom requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("https://api.intercom.io{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Intercom request error: {e}")),
    }
}

// ── Slice 285: Pipedrive ───────────────────────────────────────────────────────

pub(super) async fn execute_pipedrive(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Pipedrive requires 'api_token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Pipedrive requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    // Pipedrive uses api_token as query parameter
    let url = format!("https://api.pipedrive.com/v1{ep}?api_token={api_token}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Pipedrive request error: {e}")),
    }
}

pub(super) async fn execute_trello(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Trello requires 'api_key'"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Trello requires 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Trello requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    // Trello auth is via query params key= and token=
    let url = format!("https://api.trello.com/1{ep}?key={api_key}&token={token}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Trello request error: {e}")),
    }
}

// ── Slice 287: Monday ──────────────────────────────────────────────────────────

pub(super) async fn execute_monday(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Monday requires 'token'"),
    };
    let query = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => return NodeExecutionResult::failed("Monday requires 'query' (GraphQL)"),
    };
    let variables = cfg
        .get("variables")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let body = serde_json::json!({ "query": query, "variables": variables });

    match http_client
        .post("https://api.monday.com/v2")
        .header("Authorization", &token)
        .header("API-Version", "2024-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Monday request error: {e}")),
    }
}

// ── Slice 288: ClickUp ─────────────────────────────────────────────────────────

pub(super) async fn execute_clickup(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("ClickUp requires 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("ClickUp requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("https://api.clickup.com/api/v2{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", &token)
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("ClickUp request error: {e}")),
    }
}

// ── Slice 289: Amplitude ───────────────────────────────────────────────────────

pub(super) async fn execute_calendly(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Calendly requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("get_current_user")
        .to_string();
    let auth = format!("Bearer {api_key}");
    let base = "https://api.calendly.com";

    match operation.as_str() {
        "get_current_user" => {
            match http_client
                .get(format!("{base}/users/me"))
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Calendly get_current_user error: {e}"))
                }
            }
        }
        "list_event_types" => {
            let user_uri = match cfg.get("user_uri").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly list_event_types requires 'user_uri'",
                    )
                }
            };
            let url = format!("{base}/event_types?user={}", urlencoding_simple(&user_uri));
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Calendly list_event_types error: {e}"))
                }
            }
        }
        "list_scheduled_events" => {
            let user_uri = match cfg.get("user_uri").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly list_scheduled_events requires 'user_uri'",
                    )
                }
            };
            let status_filter = cfg
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("active");
            let url = format!(
                "{base}/scheduled_events?user={}&status={}",
                urlencoding_simple(&user_uri),
                status_filter
            );
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!(
                    "Calendly list_scheduled_events error: {e}"
                )),
            }
        }
        "get_scheduled_event" => {
            let event_uuid = match cfg.get("event_uuid").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly get_scheduled_event requires 'event_uuid'",
                    )
                }
            };
            let url = format!("{base}/scheduled_events/{event_uuid}");
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Calendly get_scheduled_event error: {e}"))
                }
            }
        }
        "cancel_event" => {
            let event_uuid = match cfg.get("event_uuid").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly cancel_event requires 'event_uuid'",
                    )
                }
            };
            let reason = cfg.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            let body = serde_json::json!({ "reason": reason });
            let url = format!("{base}/scheduled_events/{event_uuid}/cancellation");
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Calendly cancel_event error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Calendly unknown operation '{other}'")),
    }
}

pub(super) async fn execute_copper(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Copper requires 'api_key'"),
    };
    let user_email = match cfg.get("user_email").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Copper requires 'user_email'"),
    };
    let resource = cfg
        .get("resource")
        .and_then(|v| v.as_str())
        .unwrap_or("people");
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();
    let base = "https://api.copper.com/developer_api/v1";

    let mut req_builder;
    match operation.as_str() {
        "list" => {
            let body = cfg.get("filter").cloned().unwrap_or(serde_json::json!({}));
            req_builder = http_client
                .post(format!("{base}/{resource}/search"))
                .json(&body);
        }
        "get" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper get requires 'record_id'"),
            };
            req_builder = http_client.get(format!("{base}/{resource}/{id}"));
        }
        "create" => {
            let body = cfg
                .get("body")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            req_builder = http_client.post(format!("{base}/{resource}")).json(&body);
        }
        "update" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper update requires 'record_id'"),
            };
            let body = cfg
                .get("body")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            req_builder = http_client
                .put(format!("{base}/{resource}/{id}"))
                .json(&body);
        }
        "delete" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper delete requires 'record_id'"),
            };
            req_builder = http_client.delete(format!("{base}/{resource}/{id}"));
        }
        other => return NodeExecutionResult::failed(format!("Copper unknown operation '{other}'")),
    };
    req_builder = req_builder
        .header("X-PW-AccessToken", &api_key)
        .header("X-PW-Application", "developer_api")
        .header("X-PW-UserEmail", &user_email)
        .header("Content-Type", "application/json");
    match req_builder.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Copper error: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow_core::{Node, NodeType};

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{}".into(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn intercom_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ic1".into(),
            node_type: NodeType::Intercom,
            config: Some(serde_json::json!({ "endpoint": "/contacts" })),
        };
        let r = execute_intercom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn intercom_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ic2".into(),
            node_type: NodeType::Intercom,
            config: Some(serde_json::json!({ "token": "dG9rZW4…" })),
        };
        let r = execute_intercom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Pipedrive ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn pipedrive_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pd1".into(),
            node_type: NodeType::Pipedrive,
            config: Some(serde_json::json!({ "endpoint": "/deals" })),
        };
        let r = execute_pipedrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn pipedrive_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pd2".into(),
            node_type: NodeType::Pipedrive,
            config: Some(serde_json::json!({ "api_token": "abc123" })),
        };
        let r = execute_pipedrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[tokio::test]
    async fn trello_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tr1".into(),
            node_type: NodeType::Trello,
            config: Some(serde_json::json!({ "token": "tok", "endpoint": "/boards" })),
        };
        let r = execute_trello(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn trello_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tr2".into(),
            node_type: NodeType::Trello,
            config: Some(serde_json::json!({ "api_key": "key", "endpoint": "/boards" })),
        };
        let r = execute_trello(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn trello_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tr3".into(),
            node_type: NodeType::Trello,
            config: Some(serde_json::json!({ "api_key": "key", "token": "tok" })),
        };
        let r = execute_trello(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Monday ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn monday_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "mn1".into(),
            node_type: NodeType::Monday,
            config: Some(serde_json::json!({ "query": "{ boards { id } }" })),
        };
        let r = execute_monday(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn monday_fails_without_query() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "mn2".into(),
            node_type: NodeType::Monday,
            config: Some(serde_json::json!({ "token": "eyJhbGci…" })),
        };
        let r = execute_monday(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    // ── ClickUp ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn clickup_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cu1".into(),
            node_type: NodeType::Clickup,
            config: Some(serde_json::json!({ "endpoint": "/team" })),
        };
        let r = execute_clickup(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn clickup_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cu2".into(),
            node_type: NodeType::Clickup,
            config: Some(serde_json::json!({ "token": "pk_abc" })),
        };
        let r = execute_clickup(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Amplitude ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn calendly_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca1".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"operation":"get_current_user"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn calendly_list_event_types_fails_without_user_uri() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca2".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"list_event_types"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("user_uri"));
    }

    #[tokio::test]
    async fn calendly_cancel_event_fails_without_uuid() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca3".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"cancel_event"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("event_uuid"));
    }

    #[tokio::test]
    async fn calendly_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca4".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"bad"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    #[tokio::test]
    async fn copper_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "co1".into(),
            node_type: NodeType::Copper,
            config: Some(serde_json::json!({"user_email":"a@b.com"})),
        };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn copper_fails_without_user_email() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "co2".into(),
            node_type: NodeType::Copper,
            config: Some(serde_json::json!({"api_key":"key"})),
        };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("user_email"));
    }

    #[tokio::test]
    async fn copper_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "co3".into(),
            node_type: NodeType::Copper,
            config: Some(
                serde_json::json!({"api_key":"key","user_email":"a@b.com","operation":"bad"}),
            ),
        };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }
}
