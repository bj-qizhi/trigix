// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Late-stage integration nodes (continued).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── Slice 298: Mailchimp ───────────────────────────────────────────────────────

pub(super) async fn execute_mailchimp(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Mailchimp requires 'api_key'"),
    };
    let server = match cfg.get("server").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        // Try to extract server prefix from api_key (format: key-us1)
        _ => match api_key.split('-').last() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => return NodeExecutionResult::failed("Mailchimp requires 'server' (e.g. us1)"),
        },
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Mailchimp requires 'endpoint'"),
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
    let url = format!("https://{server}.api.mailchimp.com/3.0{ep}");

    use base64::Engine as _;
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("anystring:{api_key}").as_bytes());

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {encoded}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Mailchimp request error: {e}")),
    }
}

// ── Slice 299: ActiveCampaign ──────────────────────────────────────────────────

pub(super) async fn execute_activecampaign(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("ActiveCampaign requires 'api_key'"),
    };
    let base_url = match cfg.get("base_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "ActiveCampaign requires 'base_url' (e.g. https://ACCOUNT.api-us1.com)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("ActiveCampaign requires 'endpoint'"),
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
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/3{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Api-Token", &api_key)
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT") {
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
        Err(e) => NodeExecutionResult::failed(format!("ActiveCampaign request error: {e}")),
    }
}

// ── Slice 300: Klaviyo ─────────────────────────────────────────────────────────

pub(super) async fn execute_klaviyo(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Klaviyo requires 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Klaviyo requires 'endpoint'"),
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
    let url = format!("https://a.klaviyo.com/api{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Klaviyo-API-Key {api_key}"))
        .header("revision", "2024-02-15")
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
        Err(e) => NodeExecutionResult::failed(format!("Klaviyo request error: {e}")),
    }
}

// ── Slice 301: Resend ──────────────────────────────────────────────────────────

pub(super) async fn execute_resend(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Resend requires 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Resend requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("POST")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("https://api.resend.com{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::POST),
            &url,
        )
        .header("Authorization", format!("Bearer {api_key}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Resend request error: {e}")),
    }
}

#[cfg(test)]
mod tests_298_301 {
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

    // ── Mailchimp ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn mailchimp_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "mc1".into(),
            node_type: NodeType::Mailchimp,
            config: Some(serde_json::json!({ "server": "us1", "endpoint": "/lists" })),
        };
        let r = execute_mailchimp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn mailchimp_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "mc2".into(),
            node_type: NodeType::Mailchimp,
            config: Some(serde_json::json!({ "api_key": "abc-us1", "server": "us1" })),
        };
        let r = execute_mailchimp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[tokio::test]
    async fn mailchimp_extracts_server_from_key() {
        // api_key format key-us1 → server "us1" extracted automatically
        let c = reqwest::Client::new();
        let n = Node {
            id: "mc3".into(),
            node_type: NodeType::Mailchimp,
            config: Some(serde_json::json!({ "api_key": "abc123-us7", "endpoint": "/lists" })),
        };
        // No server provided — should not fail with "server" error (will fail at network)
        let r = execute_mailchimp(&n, &ctx(), &c).await;
        assert!(!r.error.as_deref().unwrap_or("x").contains("server"));
    }

    // ── ActiveCampaign ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn activecampaign_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ac1".into(),
            node_type: NodeType::Activecampaign,
            config: Some(
                serde_json::json!({ "base_url": "https://acct.api-us1.com", "endpoint": "/contacts" }),
            ),
        };
        let r = execute_activecampaign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn activecampaign_fails_without_base_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ac2".into(),
            node_type: NodeType::Activecampaign,
            config: Some(serde_json::json!({ "api_key": "abc", "endpoint": "/contacts" })),
        };
        let r = execute_activecampaign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("base_url"));
    }

    #[tokio::test]
    async fn activecampaign_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ac3".into(),
            node_type: NodeType::Activecampaign,
            config: Some(
                serde_json::json!({ "api_key": "abc", "base_url": "https://acct.api-us1.com" }),
            ),
        };
        let r = execute_activecampaign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Klaviyo ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn klaviyo_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "kv1".into(),
            node_type: NodeType::Klaviyo,
            config: Some(serde_json::json!({ "endpoint": "/profiles" })),
        };
        let r = execute_klaviyo(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn klaviyo_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "kv2".into(),
            node_type: NodeType::Klaviyo,
            config: Some(serde_json::json!({ "api_key": "pk_abc" })),
        };
        let r = execute_klaviyo(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Resend ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn resend_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rs1".into(),
            node_type: NodeType::Resend,
            config: Some(serde_json::json!({ "endpoint": "/emails" })),
        };
        let r = execute_resend(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn resend_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rs2".into(),
            node_type: NodeType::Resend,
            config: Some(serde_json::json!({ "api_key": "re_abc" })),
        };
        let r = execute_resend(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 302: Contentful ──────────────────────────────────────────────────────

pub(super) async fn execute_contentful(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Contentful requires 'access_token'"),
    };
    let space_id = match cfg.get("space_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Contentful requires 'space_id'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Contentful requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    // Management API uses api.contentful.com; Delivery/Preview use cdn.contentful.com
    let api_type = cfg
        .get("api_type")
        .and_then(|v| v.as_str())
        .unwrap_or("delivery");
    let base = match api_type {
        "management" => format!("https://api.contentful.com/spaces/{space_id}"),
        "preview" => format!("https://preview.contentful.com/spaces/{space_id}"),
        _ => format!("https://cdn.contentful.com/spaces/{space_id}"),
    };
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("{base}{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {access_token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Contentful request error: {e}")),
    }
}

// ── Slice 303: Algolia ─────────────────────────────────────────────────────────

pub(super) async fn execute_algolia(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let app_id = match cfg.get("app_id").and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return NodeExecutionResult::failed("Algolia requires 'app_id'"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Algolia requires 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Algolia requires 'endpoint'"),
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
    let url = format!("https://{app_id}-dsn.algolia.net{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("X-Algolia-Application-Id", &app_id)
        .header("X-Algolia-API-Key", &api_key)
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT") {
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
        Err(e) => NodeExecutionResult::failed(format!("Algolia request error: {e}")),
    }
}

// ── Slice 304: Postmark ────────────────────────────────────────────────────────

pub(super) async fn execute_postmark(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let server_token = match cfg.get("server_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Postmark requires 'server_token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Postmark requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("POST")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("https://api.postmarkapp.com{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::POST),
            &url,
        )
        .header("X-Postmark-Server-Token", &server_token)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT") {
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
        Err(e) => NodeExecutionResult::failed(format!("Postmark request error: {e}")),
    }
}

// ── Slice 305: Vonage ──────────────────────────────────────────────────────────

pub(super) async fn execute_vonage(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Vonage requires 'api_key'"),
    };
    let api_secret = match cfg.get("api_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Vonage requires 'api_secret'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("sms")
        .to_string();

    match operation.as_str() {
        "sms" => {
            let to = cfg
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let from = cfg
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or("Vonage")
                .to_string();
            let text = cfg
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let body = serde_json::json!({
                "from": from, "to": to, "text": text,
                "api_key": api_key, "api_secret": api_secret
            });
            match http_client
                .post("https://rest.nexmo.com/sms/json")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let resp_body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": resp_body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Vonage SMS error: {e}")),
            }
        }
        "voice" | "verify" => {
            let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
                Some(e) if !e.is_empty() => e.to_string(),
                _ => return NodeExecutionResult::failed("Vonage voice/verify requires 'endpoint'"),
            };
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{api_key}:{api_secret}").as_bytes());
            let ep = if endpoint.starts_with('/') {
                endpoint.clone()
            } else {
                format!("/{endpoint}")
            };
            let url = format!("https://api.nexmo.com{ep}");
            let body = cfg.get("body").cloned().unwrap_or(serde_json::Value::Null);
            match http_client
                .post(&url)
                .header("Authorization", format!("Basic {encoded}"))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let resp_body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": resp_body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Vonage {operation} error: {e}")),
            }
        }
        _ => NodeExecutionResult::failed(format!("Vonage unknown operation '{operation}'")),
    }
}

#[cfg(test)]
mod tests_302_305 {
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

    // ── Contentful ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn contentful_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cf1".into(),
            node_type: NodeType::Contentful,
            config: Some(serde_json::json!({ "space_id": "sp1", "endpoint": "/entries" })),
        };
        let r = execute_contentful(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn contentful_fails_without_space_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cf2".into(),
            node_type: NodeType::Contentful,
            config: Some(serde_json::json!({ "access_token": "tok", "endpoint": "/entries" })),
        };
        let r = execute_contentful(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("space_id"));
    }

    #[tokio::test]
    async fn contentful_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cf3".into(),
            node_type: NodeType::Contentful,
            config: Some(serde_json::json!({ "access_token": "tok", "space_id": "sp1" })),
        };
        let r = execute_contentful(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Algolia ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn algolia_fails_without_app_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "al1".into(),
            node_type: NodeType::Algolia,
            config: Some(
                serde_json::json!({ "api_key": "key", "endpoint": "/1/indexes/myindex/query" }),
            ),
        };
        let r = execute_algolia(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("app_id"));
    }

    #[tokio::test]
    async fn algolia_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "al2".into(),
            node_type: NodeType::Algolia,
            config: Some(
                serde_json::json!({ "app_id": "ABC123", "endpoint": "/1/indexes/myindex/query" }),
            ),
        };
        let r = execute_algolia(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn algolia_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "al3".into(),
            node_type: NodeType::Algolia,
            config: Some(serde_json::json!({ "app_id": "ABC123", "api_key": "key" })),
        };
        let r = execute_algolia(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Postmark ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn postmark_fails_without_server_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pm1".into(),
            node_type: NodeType::Postmark,
            config: Some(serde_json::json!({ "endpoint": "/email" })),
        };
        let r = execute_postmark(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("server_token"));
    }

    #[tokio::test]
    async fn postmark_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pm2".into(),
            node_type: NodeType::Postmark,
            config: Some(serde_json::json!({ "server_token": "tok-abc" })),
        };
        let r = execute_postmark(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Vonage ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn vonage_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "vn1".into(),
            node_type: NodeType::Vonage,
            config: Some(
                serde_json::json!({ "api_secret": "sec", "operation": "sms", "to": "1234", "text": "hi" }),
            ),
        };
        let r = execute_vonage(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn vonage_fails_without_api_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "vn2".into(),
            node_type: NodeType::Vonage,
            config: Some(serde_json::json!({ "api_key": "key", "operation": "sms" })),
        };
        let r = execute_vonage(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_secret"));
    }

    #[tokio::test]
    async fn vonage_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "vn3".into(),
            node_type: NodeType::Vonage,
            config: Some(
                serde_json::json!({ "api_key": "key", "api_secret": "sec", "operation": "bogus" }),
            ),
        };
        let r = execute_vonage(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }
}
// ── Slice 308: Telegram ────────────────────────────────────────────────────────
pub(super) async fn execute_telegram(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let bot_token = match cfg.get("bot_token").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        _ => return NodeExecutionResult::failed("Telegram requires 'bot_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("sendMessage")
        .to_string();
    let base = format!("https://api.telegram.org/bot{}/{}", bot_token, operation);
    let mut payload = serde_json::json!({});
    if let Some(chat_id) = cfg.get("chat_id").and_then(|v| v.as_str()) {
        payload["chat_id"] = serde_json::json!(chat_id);
    }
    if let Some(text) = cfg.get("text").and_then(|v| v.as_str()) {
        payload["text"] = serde_json::json!(text);
    }
    if let Some(parse_mode) = cfg.get("parse_mode").and_then(|v| v.as_str()) {
        payload["parse_mode"] = serde_json::json!(parse_mode);
    }
    if let Some(extra) = cfg.get("extra") {
        if let Some(obj) = extra.as_object() {
            if let Some(map) = payload.as_object_mut() {
                for (k, v) in obj {
                    map.insert(k.clone(), v.clone());
                }
            }
        }
    }
    match http_client.post(&base).json(&payload).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Telegram error: {e}")),
    }
}

#[cfg(test)]
mod tests_306_309 {
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

    // ── Shopify ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn shopify_fails_without_shop() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s1".into(),
            node_type: NodeType::Shopify,
            config: Some(serde_json::json!({"token":"shpat_test"})),
        };
        let r = execute_shopify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("shop"));
    }

    #[tokio::test]
    async fn shopify_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s2".into(),
            node_type: NodeType::Shopify,
            config: Some(serde_json::json!({"shop":"test.myshopify.com"})),
        };
        let r = execute_shopify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    // ── Discord ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn discord_fails_without_webhook_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d1".into(),
            node_type: NodeType::Discord,
            config: Some(serde_json::json!({"content":"hello"})),
        };
        let r = execute_discord(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("webhook_url"));
    }

    #[tokio::test]
    async fn discord_fails_without_content() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d2".into(),
            node_type: NodeType::Discord,
            config: Some(
                serde_json::json!({"webhook_url":"https://discord.com/api/webhooks/test"}),
            ),
        };
        let r = execute_discord(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("content"));
    }

    #[tokio::test]
    async fn discord_fails_without_config() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d3".into(),
            node_type: NodeType::Discord,
            config: None,
        };
        let r = execute_discord(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }

    // ── Telegram ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn telegram_fails_without_bot_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t1".into(),
            node_type: NodeType::Telegram,
            config: Some(serde_json::json!({"chat_id":"123","text":"hello"})),
        };
        let r = execute_telegram(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bot_token"));
    }

    #[tokio::test]
    async fn telegram_with_token_attempts_request() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t2".into(),
            node_type: NodeType::Telegram,
            config: Some(
                serde_json::json!({"bot_token":"123:test","chat_id":"456","text":"hello"}),
            ),
        };
        let r = execute_telegram(&n, &ctx(), &c).await;
        // Network will fail but config validation passes
        assert!(
            r.error.as_deref().unwrap_or("").contains("Telegram error") || r.output_json.is_some()
        );
    }

    // ── Notion ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn notion_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "n1".into(),
            node_type: NodeType::Notion,
            config: Some(serde_json::json!({"endpoint":"/v1/databases"})),
        };
        let r = execute_notion(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn notion_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "n2".into(),
            node_type: NodeType::Notion,
            config: Some(serde_json::json!({"token":"secret_test"})),
        };
        let r = execute_notion(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 310: Replicate ───────────────────────────────────────────────────────
pub(super) async fn execute_replicate(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Replicate requires 'api_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("run")
        .to_string();
    let auth = format!("Token {api_token}");

    match operation.as_str() {
        "run" | "create_prediction" => {
            let version = match cfg.get("version").and_then(|v| v.as_str()) {
                Some(v) if !v.is_empty() => v.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Replicate run requires 'version' (model version ID)",
                    )
                }
            };
            let input = cfg.get("input").cloned().unwrap_or(serde_json::json!({}));
            let body = serde_json::json!({ "version": version, "input": input });
            match http_client
                .post("https://api.replicate.com/v1/predictions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Replicate error: {e}")),
            }
        }
        "get_prediction" => {
            let prediction_id = match cfg.get("prediction_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Replicate get_prediction requires 'prediction_id'",
                    )
                }
            };
            let url = format!("https://api.replicate.com/v1/predictions/{prediction_id}");
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
                Err(e) => NodeExecutionResult::failed(format!("Replicate get error: {e}")),
            }
        }
        "list_models" => {
            match http_client
                .get("https://api.replicate.com/v1/models")
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
                Err(e) => NodeExecutionResult::failed(format!("Replicate list error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Replicate unknown operation '{other}'")),
    }
}

// ── Slice 311: Mistral ─────────────────────────────────────────────────────────
pub(super) async fn execute_mistral(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Mistral requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chat")
        .to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("mistral-small-latest")
                .to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed("Mistral chat requires 'messages' or 'prompt'");
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") {
                body["temperature"] = temp.clone();
            }
            if let Some(max_tokens) = cfg.get("max_tokens") {
                body["max_tokens"] = max_tokens.clone();
            }
            match http_client
                .post("https://api.mistral.ai/v1/chat/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Mistral error: {e}")),
            }
        }
        "embeddings" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("mistral-embed")
                .to_string();
            let input = match cfg.get("input") {
                Some(i) => i.clone(),
                None => return NodeExecutionResult::failed("Mistral embeddings requires 'input'"),
            };
            let body = serde_json::json!({ "model": model, "input": input });
            match http_client
                .post("https://api.mistral.ai/v1/embeddings")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Mistral embeddings error: {e}")),
            }
        }
        "list_models" => {
            match http_client
                .get("https://api.mistral.ai/v1/models")
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
                Err(e) => NodeExecutionResult::failed(format!("Mistral list models error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Mistral unknown operation '{other}'")),
    }
}

// ── Slice 312: WhatsApp Business ───────────────────────────────────────────────
pub(super) async fn execute_whatsapp(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("WhatsApp requires 'access_token'"),
    };
    let phone_number_id = match cfg.get("phone_number_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return NodeExecutionResult::failed("WhatsApp requires 'phone_number_id'"),
    };
    let to = match cfg.get("to").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("WhatsApp requires 'to' (recipient phone number)"),
    };
    let message_type = cfg
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .to_string();
    let api_version = cfg
        .get("api_version")
        .and_then(|v| v.as_str())
        .unwrap_or("v18.0");
    let url = format!("https://graph.facebook.com/{api_version}/{phone_number_id}/messages");

    let body = match message_type.as_str() {
        "text" => {
            let text = cfg
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            serde_json::json!({
                "messaging_product": "whatsapp",
                "to": to,
                "type": "text",
                "text": { "body": text }
            })
        }
        "template" => {
            let template_name = match cfg.get("template_name").and_then(|v| v.as_str()) {
                Some(n) => n.to_string(),
                None => {
                    return NodeExecutionResult::failed(
                        "WhatsApp template requires 'template_name'",
                    )
                }
            };
            let language_code = cfg
                .get("language_code")
                .and_then(|v| v.as_str())
                .unwrap_or("en_US");
            let components = cfg
                .get("components")
                .cloned()
                .unwrap_or(serde_json::json!([]));
            serde_json::json!({
                "messaging_product": "whatsapp",
                "to": to,
                "type": "template",
                "template": {
                    "name": template_name,
                    "language": { "code": language_code },
                    "components": components
                }
            })
        }
        "image" | "document" | "audio" | "video" => {
            let media_url = cfg.get("media_url").and_then(|v| v.as_str()).unwrap_or("");
            serde_json::json!({
                "messaging_product": "whatsapp",
                "to": to,
                "type": message_type,
                message_type.clone(): { "link": media_url }
            })
        }
        _ => {
            return NodeExecutionResult::failed(format!(
                "WhatsApp unknown message_type '{message_type}'"
            ))
        }
    };

    match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let resp_body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": resp_body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("WhatsApp error: {e}")),
    }
}

// ── Slice 313: Google Docs ─────────────────────────────────────────────────────
pub(super) async fn execute_googledocs(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Docs requires 'access_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("get")
        .to_string();
    let auth = format!("Bearer {access_token}");

    match operation.as_str() {
        "get" => {
            let document_id = match cfg.get("document_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Docs get requires 'document_id'"),
            };
            let url = format!("https://docs.googleapis.com/v1/documents/{document_id}");
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
                Err(e) => NodeExecutionResult::failed(format!("Google Docs get error: {e}")),
            }
        }
        "create" => {
            let title = cfg
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled Document")
                .to_string();
            let body = serde_json::json!({ "title": title });
            match http_client
                .post("https://docs.googleapis.com/v1/documents")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Google Docs create error: {e}")),
            }
        }
        "batch_update" => {
            let document_id = match cfg.get("document_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Google Docs batch_update requires 'document_id'",
                    )
                }
            };
            let requests = match cfg.get("requests") {
                Some(r) => r.clone(),
                None => {
                    return NodeExecutionResult::failed(
                        "Google Docs batch_update requires 'requests'",
                    )
                }
            };
            let url = format!("https://docs.googleapis.com/v1/documents/{document_id}:batchUpdate");
            let body = serde_json::json!({ "requests": requests });
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => {
                    NodeExecutionResult::failed(format!("Google Docs batch_update error: {e}"))
                }
            }
        }
        other => NodeExecutionResult::failed(format!("Google Docs unknown operation '{other}'")),
    }
}

#[cfg(test)]
mod tests_310_313 {
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

    // ── Replicate ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn replicate_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r1".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"version":"abc123","input":{}})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn replicate_run_fails_without_version() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r2".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"run"})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("version"));
    }

    #[tokio::test]
    async fn replicate_get_prediction_fails_without_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r3".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"get_prediction"})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prediction_id"));
    }

    #[tokio::test]
    async fn replicate_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r4".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"invalid"})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Mistral ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn mistral_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m1".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"prompt":"hello"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn mistral_chat_fails_without_messages_or_prompt() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m2".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    #[tokio::test]
    async fn mistral_embeddings_fails_without_input() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m3".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"embeddings"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("input"));
    }

    #[tokio::test]
    async fn mistral_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m4".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad_op"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── WhatsApp ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn whatsapp_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w1".into(),
            node_type: NodeType::Whatsapp,
            config: Some(serde_json::json!({"phone_number_id":"123","to":"+1234567890"})),
        };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn whatsapp_fails_without_phone_number_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w2".into(),
            node_type: NodeType::Whatsapp,
            config: Some(serde_json::json!({"access_token":"test","to":"+1234567890"})),
        };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("phone_number_id"));
    }

    #[tokio::test]
    async fn whatsapp_fails_without_to() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w3".into(),
            node_type: NodeType::Whatsapp,
            config: Some(serde_json::json!({"access_token":"test","phone_number_id":"123"})),
        };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("'to'"));
    }

    #[tokio::test]
    async fn whatsapp_template_fails_without_template_name() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w4".into(),
            node_type: NodeType::Whatsapp,
            config: Some(
                serde_json::json!({"access_token":"t","phone_number_id":"123","to":"+1","message_type":"template"}),
            ),
        };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("template_name"));
    }

    // ── Google Docs ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn googledocs_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g1".into(),
            node_type: NodeType::Googledocs,
            config: Some(serde_json::json!({"document_id":"doc123"})),
        };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn googledocs_get_fails_without_document_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g2".into(),
            node_type: NodeType::Googledocs,
            config: Some(serde_json::json!({"access_token":"test","operation":"get"})),
        };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("document_id"));
    }

    #[tokio::test]
    async fn googledocs_batch_update_fails_without_requests() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g3".into(),
            node_type: NodeType::Googledocs,
            config: Some(
                serde_json::json!({"access_token":"test","operation":"batch_update","document_id":"doc123"}),
            ),
        };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("requests"));
    }

    #[tokio::test]
    async fn googledocs_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g4".into(),
            node_type: NodeType::Googledocs,
            config: Some(serde_json::json!({"access_token":"test","operation":"invalid"})),
        };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }
}

// ── Slice 314: Perplexity ──────────────────────────────────────────────────────
pub(super) async fn execute_perplexity(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Perplexity requires 'api_key'"),
    };
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("llama-3.1-sonar-small-128k-online")
        .to_string();
    let messages = if let Some(msgs) = cfg.get("messages") {
        msgs.clone()
    } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
        serde_json::json!([{"role": "user", "content": prompt}])
    } else {
        return NodeExecutionResult::failed("Perplexity requires 'messages' or 'prompt'");
    };
    let mut body = serde_json::json!({ "model": model, "messages": messages });
    if let Some(temp) = cfg.get("temperature") {
        body["temperature"] = temp.clone();
    }
    if let Some(max_tokens) = cfg.get("max_tokens") {
        body["max_tokens"] = max_tokens.clone();
    }
    if let Some(search_domain_filter) = cfg.get("search_domain_filter") {
        body["search_domain_filter"] = search_domain_filter.clone();
    }
    if let Some(return_citations) = cfg.get("return_citations") {
        body["return_citations"] = return_citations.clone();
    }
    match http_client
        .post("https://api.perplexity.ai/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Perplexity error: {e}")),
    }
}

// ── Slice 315: Cohere ──────────────────────────────────────────────────────────
pub(super) async fn execute_cohere(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Cohere requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chat")
        .to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let message = match cfg.get("message").and_then(|v| v.as_str()) {
                Some(m) if !m.is_empty() => m.to_string(),
                _ => return NodeExecutionResult::failed("Cohere chat requires 'message'"),
            };
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("command-r-plus")
                .to_string();
            let mut body = serde_json::json!({ "message": message, "model": model });
            if let Some(temperature) = cfg.get("temperature") {
                body["temperature"] = temperature.clone();
            }
            if let Some(chat_history) = cfg.get("chat_history") {
                body["chat_history"] = chat_history.clone();
            }
            match http_client
                .post("https://api.cohere.com/v1/chat")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Cohere chat error: {e}")),
            }
        }
        "embed" => {
            let texts = match cfg.get("texts") {
                Some(t) => t.clone(),
                None => {
                    return NodeExecutionResult::failed(
                        "Cohere embed requires 'texts' (array of strings)",
                    )
                }
            };
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("embed-english-v3.0")
                .to_string();
            let input_type = cfg
                .get("input_type")
                .and_then(|v| v.as_str())
                .unwrap_or("search_document")
                .to_string();
            let body =
                serde_json::json!({ "texts": texts, "model": model, "input_type": input_type });
            match http_client
                .post("https://api.cohere.com/v1/embed")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Cohere embed error: {e}")),
            }
        }
        "classify" => {
            let inputs = match cfg.get("inputs") {
                Some(i) => i.clone(),
                None => return NodeExecutionResult::failed("Cohere classify requires 'inputs'"),
            };
            let examples = match cfg.get("examples") {
                Some(e) => e.clone(),
                None => return NodeExecutionResult::failed("Cohere classify requires 'examples'"),
            };
            let body = serde_json::json!({ "inputs": inputs, "examples": examples });
            match http_client
                .post("https://api.cohere.com/v1/classify")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Cohere classify error: {e}")),
            }
        }
        "rerank" => {
            let query = match cfg.get("query").and_then(|v| v.as_str()) {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => return NodeExecutionResult::failed("Cohere rerank requires 'query'"),
            };
            let documents = match cfg.get("documents") {
                Some(d) => d.clone(),
                None => return NodeExecutionResult::failed("Cohere rerank requires 'documents'"),
            };
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("rerank-english-v3.0")
                .to_string();
            let body =
                serde_json::json!({ "query": query, "documents": documents, "model": model });
            match http_client
                .post("https://api.cohere.com/v1/rerank")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Cohere rerank error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Cohere unknown operation '{other}'")),
    }
}

// ── Slice 316: Google Drive ────────────────────────────────────────────────────
pub(super) async fn execute_googledrive(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Drive requires 'access_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();
    let auth = format!("Bearer {access_token}");

    match operation.as_str() {
        "list" => {
            let mut url = "https://www.googleapis.com/drive/v3/files?pageSize=100".to_string();
            if let Some(q) = cfg.get("query").and_then(|v| v.as_str()) {
                url.push_str(&format!("&q={}", urlencoding_simple(q)));
            }
            if let Some(fields) = cfg.get("fields").and_then(|v| v.as_str()) {
                url.push_str(&format!("&fields={}", urlencoding_simple(fields)));
            }
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
                Err(e) => NodeExecutionResult::failed(format!("Google Drive list error: {e}")),
            }
        }
        "get" => {
            let file_id = match cfg.get("file_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Drive get requires 'file_id'"),
            };
            let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}?fields=*");
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
                Err(e) => NodeExecutionResult::failed(format!("Google Drive get error: {e}")),
            }
        }
        "delete" => {
            let file_id = match cfg.get("file_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Drive delete requires 'file_id'"),
            };
            let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}");
            match http_client
                .delete(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Drive delete error: {e}")),
            }
        }
        "create_folder" => {
            let name = cfg
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("New Folder")
                .to_string();
            let parent_id = cfg.get("parent_id").and_then(|v| v.as_str());
            let mut metadata = serde_json::json!({
                "name": name,
                "mimeType": "application/vnd.google-apps.folder"
            });
            if let Some(pid) = parent_id {
                metadata["parents"] = serde_json::json!([pid]);
            }
            match http_client
                .post("https://www.googleapis.com/drive/v3/files")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&metadata)
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
                    NodeExecutionResult::failed(format!("Google Drive create_folder error: {e}"))
                }
            }
        }
        other => NodeExecutionResult::failed(format!("Google Drive unknown operation '{other}'")),
    }
}

// ── Slice 317: WooCommerce ─────────────────────────────────────────────────────
pub(super) async fn execute_woocommerce(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let consumer_key = match cfg.get("consumer_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("WooCommerce requires 'consumer_key'"),
    };
    let consumer_secret = match cfg.get("consumer_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("WooCommerce requires 'consumer_secret'"),
    };
    let site_url = match cfg.get("site_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed("WooCommerce requires 'site_url'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/wp-json/wc/v3/products")
        .to_string();
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("{}{}", site_url, endpoint);

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("{consumer_key}:{consumer_secret}").as_bytes());

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        "PATCH" => http_client.patch(&url),
        _ => http_client.get(&url),
    };
    req = req
        .header("Authorization", format!("Basic {encoded}"))
        .header("Content-Type", "application/json");
    if let Some(body) = cfg.get("body") {
        req = req.json(body);
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("WooCommerce error: {e}")),
    }
}

#[cfg(test)]
mod tests_314_317 {
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

    // ── Perplexity ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn perplexity_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p1".into(),
            node_type: NodeType::Perplexity,
            config: Some(serde_json::json!({"prompt":"hello"})),
        };
        let r = execute_perplexity(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn perplexity_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p2".into(),
            node_type: NodeType::Perplexity,
            config: Some(serde_json::json!({"api_key":"test"})),
        };
        let r = execute_perplexity(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    // ── Cohere ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cohere_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c1".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"message":"hello"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn cohere_chat_fails_without_message() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c2".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("message"));
    }

    #[tokio::test]
    async fn cohere_embed_fails_without_texts() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c3".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"embed"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("texts"));
    }

    #[tokio::test]
    async fn cohere_rerank_fails_without_query() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c4".into(),
            node_type: NodeType::Cohere,
            config: Some(
                serde_json::json!({"api_key":"test","operation":"rerank","documents":["doc1"]}),
            ),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn cohere_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c5".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"invalid"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Google Drive ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn googledrive_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g1".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"operation":"list"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn googledrive_get_fails_without_file_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g2".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"get"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("file_id"));
    }

    #[tokio::test]
    async fn googledrive_delete_fails_without_file_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g3".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"delete"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("file_id"));
    }

    #[tokio::test]
    async fn googledrive_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g4".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"bad"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── WooCommerce ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn woocommerce_fails_without_consumer_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w1".into(),
            node_type: NodeType::Woocommerce,
            config: Some(
                serde_json::json!({"consumer_secret":"sec","site_url":"https://shop.example.com"}),
            ),
        };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("consumer_key"));
    }

    #[tokio::test]
    async fn woocommerce_fails_without_consumer_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w2".into(),
            node_type: NodeType::Woocommerce,
            config: Some(
                serde_json::json!({"consumer_key":"ck_test","site_url":"https://shop.example.com"}),
            ),
        };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("consumer_secret"));
    }

    #[tokio::test]
    async fn woocommerce_fails_without_site_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w3".into(),
            node_type: NodeType::Woocommerce,
            config: Some(serde_json::json!({"consumer_key":"ck_test","consumer_secret":"cs_test"})),
        };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("site_url"));
    }
}

// ── Slice 318: Pinecone ────────────────────────────────────────────────────────
pub(super) async fn execute_pinecone(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Pinecone requires 'api_key'"),
    };
    let index_host = match cfg.get("index_host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Pinecone requires 'index_host' (e.g. https://my-index-abc.svc.pinecone.io)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("query")
        .to_string();

    match operation.as_str() {
        "query" => {
            let vector = match cfg.get("vector") {
                Some(v) => json_array_or_parse(v),
                None => {
                    return NodeExecutionResult::failed(
                        "Pinecone query requires 'vector' (float array)",
                    )
                }
            };
            let top_k = cfg
                .get("top_k")
                .or_else(|| cfg.get("top"))
                .and_then(|v| v.as_u64())
                .unwrap_or(10);
            let mut body = serde_json::json!({ "vector": vector, "topK": top_k });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                body["namespace"] = serde_json::json!(ns);
            }
            if let Some(filter) = cfg.get("filter") {
                body["filter"] = filter.clone();
            }
            if let Some(imd) = cfg.get("include_metadata").and_then(|v| v.as_bool()) {
                body["includeMetadata"] = serde_json::json!(imd);
            }
            let url = format!("{}/query", index_host);
            match http_client
                .post(&url)
                .header("Api-Key", &api_key)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone query error: {e}")),
            }
        }
        "upsert" => {
            let vectors = match cfg.get("vectors") {
                Some(v) => json_array_or_parse(v),
                None => {
                    return NodeExecutionResult::failed(
                        "Pinecone upsert requires 'vectors' (array of {id, values, metadata})",
                    )
                }
            };
            let mut body = serde_json::json!({ "vectors": vectors });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                body["namespace"] = serde_json::json!(ns);
            }
            let url = format!("{}/vectors/upsert", index_host);
            match http_client
                .post(&url)
                .header("Api-Key", &api_key)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone upsert error: {e}")),
            }
        }
        "delete" => {
            let ids = match cfg.get("ids") {
                Some(v) => json_array_or_parse(v),
                None => {
                    return NodeExecutionResult::failed(
                        "Pinecone delete requires 'ids' (array of vector IDs)",
                    )
                }
            };
            let mut body = serde_json::json!({ "ids": ids });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                body["namespace"] = serde_json::json!(ns);
            }
            let url = format!("{}/vectors/delete", index_host);
            match http_client
                .post(&url)
                .header("Api-Key", &api_key)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone delete error: {e}")),
            }
        }
        "fetch" => {
            let ids = match cfg.get("ids").and_then(|v| v.as_array()) {
                Some(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                None => {
                    return NodeExecutionResult::failed("Pinecone fetch requires 'ids' (array)")
                }
            };
            let mut url = format!("{}/vectors/fetch?ids={}", index_host, ids);
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                url.push_str(&format!("&namespace={ns}"));
            }
            match http_client
                .get(&url)
                .header("Api-Key", &api_key)
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone fetch error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Pinecone unknown operation '{other}'")),
    }
}

// ── Slice 319: Together AI ─────────────────────────────────────────────────────
pub(super) async fn execute_togetherai(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Together AI requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chat")
        .to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo")
                .to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed(
                    "Together AI chat requires 'messages' or 'prompt'",
                );
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") {
                body["temperature"] = temp.clone();
            }
            if let Some(max_tokens) = cfg.get("max_tokens") {
                body["max_tokens"] = max_tokens.clone();
            }
            match http_client
                .post("https://api.together.xyz/v1/chat/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Together AI chat error: {e}")),
            }
        }
        "completions" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("mistralai/Mixtral-8x7B-Instruct-v0.1")
                .to_string();
            let prompt = match cfg.get("prompt").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => {
                    return NodeExecutionResult::failed("Together AI completions requires 'prompt'")
                }
            };
            let mut body = serde_json::json!({ "model": model, "prompt": prompt });
            if let Some(temp) = cfg.get("temperature") {
                body["temperature"] = temp.clone();
            }
            if let Some(max_tokens) = cfg.get("max_tokens") {
                body["max_tokens"] = max_tokens.clone();
            }
            match http_client
                .post("https://api.together.xyz/v1/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => {
                    NodeExecutionResult::failed(format!("Together AI completions error: {e}"))
                }
            }
        }
        "embeddings" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("togethercomputer/m2-bert-80M-8k-retrieval")
                .to_string();
            let input = match cfg.get("input") {
                Some(i) => i.clone(),
                None => {
                    return NodeExecutionResult::failed("Together AI embeddings requires 'input'")
                }
            };
            let body = serde_json::json!({ "model": model, "input": input });
            match http_client
                .post("https://api.together.xyz/v1/embeddings")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Together AI embeddings error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Together AI unknown operation '{other}'")),
    }
}

// ── Slice 320: AWS S3 ──────────────────────────────────────────────────────────
pub(super) async fn execute_awss3(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_key_id = match cfg.get("access_key_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("AWS S3 requires 'access_key_id'"),
    };
    let secret_access_key = match cfg.get("secret_access_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("AWS S3 requires 'secret_access_key'"),
    };
    let bucket = match cfg.get("bucket").and_then(|v| v.as_str()) {
        Some(b) if !b.is_empty() => b.to_string(),
        _ => return NodeExecutionResult::failed("AWS S3 requires 'bucket'"),
    };
    let region = cfg
        .get("region")
        .and_then(|v| v.as_str())
        .unwrap_or("us-east-1")
        .to_string();
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();

    let host = if region == "us-east-1" {
        format!("{}.s3.amazonaws.com", bucket)
    } else {
        format!("{}.s3.{}.amazonaws.com", bucket, region)
    };
    let base_url = format!("https://{}", host);

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp = now.as_secs();
    let date_str = {
        let (y, m, d) = epoch_to_ymd(timestamp);
        format!("{:04}{:02}{:02}", y, m, d)
    };
    let datetime_str = {
        let h = (timestamp % 86400) / 3600;
        let min = (timestamp % 3600) / 60;
        let sec = timestamp % 60;
        format!("{}T{:02}{:02}{:02}Z", date_str, h, min, sec)
    };
    const EMPTY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    match operation.as_str() {
        "list" => {
            let prefix = cfg
                .get("prefix")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let canonical_query = if prefix.is_empty() {
                "list-type=2".to_string()
            } else {
                format!("list-type=2&prefix={}", sigv4_uri_encode(&prefix))
            };
            let url = format!("{}/?{}", base_url, canonical_query);
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "GET",
                &host,
                "/",
                &canonical_query,
                EMPTY_HASH,
            );
            match http_client
                .get(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 list error: {e}")),
            }
        }
        "get_object" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return NodeExecutionResult::failed("S3 get_object requires 'key'"),
            };
            let key_path = format!("/{}", key.trim_start_matches('/'));
            let url = format!("{}{}", base_url, key_path);
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "GET",
                &host,
                &key_path,
                "",
                EMPTY_HASH,
            );
            match http_client
                .get(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 get_object error: {e}")),
            }
        }
        "delete_object" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return NodeExecutionResult::failed("S3 delete_object requires 'key'"),
            };
            let key_path = format!("/{}", key.trim_start_matches('/'));
            let url = format!("{}{}", base_url, key_path);
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "DELETE",
                &host,
                &key_path,
                "",
                EMPTY_HASH,
            );
            match http_client
                .delete(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 delete_object error: {e}")),
            }
        }
        "put_object" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return NodeExecutionResult::failed("S3 put_object requires 'key'"),
            };
            let body_content = cfg
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content_type = cfg
                .get("content_type")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string();
            let key_path = format!("/{}", key.trim_start_matches('/'));
            let url = format!("{}{}", base_url, key_path);
            let payload_hash = {
                use sha2::{Digest, Sha256};
                hex::encode(Sha256::digest(body_content.as_bytes()))
            };
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "PUT",
                &host,
                &key_path,
                "",
                &payload_hash,
            );
            match http_client
                .put(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", &payload_hash)
                .header("Content-Type", &content_type)
                .body(body_content)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 put_object error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("S3 unknown operation '{other}'")),
    }
}

fn sigv4_uri_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b => format!("%{:02X}", b),
        })
        .collect()
}

fn aws_sigv4_s3_auth(
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    date_str: &str,
    datetime_str: &str,
    method: &str,
    host: &str,
    canonical_uri: &str,
    canonical_query: &str,
    payload_hash: &str,
) -> String {
    use hmac::{Hmac, Mac};
    use sha2::{Digest, Sha256};
    type HmacSha256 = Hmac<Sha256>;

    let canonical_headers = format!(
        "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
        host, payload_hash, datetime_str
    );
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, canonical_uri, canonical_query, canonical_headers, signed_headers, payload_hash
    );

    let credential_scope = format!("{}/{}/s3/aws4_request", date_str, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        datetime_str,
        credential_scope,
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    let k_date = {
        let mut mac = HmacSha256::new_from_slice(format!("AWS4{}", secret_access_key).as_bytes())
            .expect("valid key");
        mac.update(date_str.as_bytes());
        mac.finalize().into_bytes()
    };
    let k_region = {
        let mut mac = HmacSha256::new_from_slice(&k_date).expect("valid key");
        mac.update(region.as_bytes());
        mac.finalize().into_bytes()
    };
    let k_service = {
        let mut mac = HmacSha256::new_from_slice(&k_region).expect("valid key");
        mac.update(b"s3");
        mac.finalize().into_bytes()
    };
    let k_signing = {
        let mut mac = HmacSha256::new_from_slice(&k_service).expect("valid key");
        mac.update(b"aws4_request");
        mac.finalize().into_bytes()
    };
    let signature = {
        let mut mac = HmacSha256::new_from_slice(&k_signing).expect("valid key");
        mac.update(string_to_sign.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    };

    format!(
        "AWS4-HMAC-SHA256 Credential={}/{},SignedHeaders={},Signature={}",
        access_key_id, credential_scope, signed_headers, signature
    )
}

fn epoch_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days = secs / 86400;
    let mut y = 1970u32;
    let mut d = days as u32;
    loop {
        let dy = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            366
        } else {
            365
        };
        if d < dy {
            break;
        }
        d -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days = [
        31u32,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0u32;
    for &md in &month_days {
        if d < md {
            break;
        }
        d -= md;
        m += 1;
    }
    (y, m + 1, d + 1)
}

// ── Slice 321: Hugging Face ────────────────────────────────────────────────────
pub(super) async fn execute_huggingface(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Hugging Face requires 'api_token'"),
    };
    let model = match cfg.get("model").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Hugging Face requires 'model' (e.g. gpt2 or facebook/bart-large-cnn)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("inference")
        .to_string();
    let auth = format!("Bearer {api_token}");

    match operation.as_str() {
        "inference" => {
            let inputs = match cfg.get("inputs") {
                Some(i) => i.clone(),
                None => {
                    return NodeExecutionResult::failed("Hugging Face inference requires 'inputs'")
                }
            };
            let mut body = serde_json::json!({ "inputs": inputs });
            if let Some(params) = cfg.get("parameters") {
                body["parameters"] = params.clone();
            }
            if let Some(options) = cfg.get("options") {
                body["options"] = options.clone();
            }
            let url = format!("https://api-inference.huggingface.co/models/{model}");
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Hugging Face inference error: {e}")),
            }
        }
        "model_info" => {
            let url = format!("https://huggingface.co/api/models/{model}");
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
                    NodeExecutionResult::failed(format!("Hugging Face model_info error: {e}"))
                }
            }
        }
        "list_models" => {
            let search = cfg.get("search").and_then(|v| v.as_str()).unwrap_or("");
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);
            let url = format!("https://huggingface.co/api/models?search={search}&limit={limit}");
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
                    NodeExecutionResult::failed(format!("Hugging Face list_models error: {e}"))
                }
            }
        }
        other => NodeExecutionResult::failed(format!("Hugging Face unknown operation '{other}'")),
    }
}

#[cfg(test)]
mod tests_318_321 {
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

    // ── Pinecone ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn pinecone_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p1".into(),
            node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"index_host":"https://idx.svc.pinecone.io"})),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn pinecone_fails_without_index_host() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p2".into(),
            node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"api_key":"test"})),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("index_host"));
    }

    #[tokio::test]
    async fn pinecone_query_fails_without_vector() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p3".into(),
            node_type: NodeType::Pinecone,
            config: Some(
                serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"query"}),
            ),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vector"));
    }

    #[tokio::test]
    async fn pinecone_upsert_fails_without_vectors() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p4".into(),
            node_type: NodeType::Pinecone,
            config: Some(
                serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"upsert"}),
            ),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vectors"));
    }

    #[tokio::test]
    async fn pinecone_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p5".into(),
            node_type: NodeType::Pinecone,
            config: Some(
                serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"bad"}),
            ),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Together AI ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn togetherai_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t1".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"prompt":"hello"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn togetherai_chat_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t2".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    #[tokio::test]
    async fn togetherai_completions_fails_without_prompt() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t3".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"completions"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn togetherai_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t4".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── AWS S3 ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn awss3_fails_without_access_key_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a1".into(),
            node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"secret_access_key":"sec","bucket":"my-bucket"})),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_key_id"));
    }

    #[tokio::test]
    async fn awss3_fails_without_bucket() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a2".into(),
            node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"access_key_id":"key","secret_access_key":"sec"})),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bucket"));
    }

    #[tokio::test]
    async fn awss3_get_object_fails_without_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a3".into(),
            node_type: NodeType::Awss3,
            config: Some(
                serde_json::json!({"access_key_id":"k","secret_access_key":"s","bucket":"b","operation":"get_object"}),
            ),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key"));
    }

    #[tokio::test]
    async fn awss3_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a4".into(),
            node_type: NodeType::Awss3,
            config: Some(
                serde_json::json!({"access_key_id":"k","secret_access_key":"s","bucket":"b","operation":"bad"}),
            ),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── SigV4 signing ─────────────────────────────────────────────────────────

    #[test]
    fn sigv4_uri_encode_encodes_spaces_as_percent20() {
        assert_eq!(sigv4_uri_encode("hello world"), "hello%20world");
    }

    #[test]
    fn sigv4_uri_encode_encodes_slashes() {
        assert_eq!(sigv4_uri_encode("foo/bar"), "foo%2Fbar");
    }

    #[test]
    fn sigv4_uri_encode_passes_through_unreserved_chars() {
        assert_eq!(sigv4_uri_encode("abc-123.~_"), "abc-123.~_");
    }

    #[test]
    fn aws_sigv4_s3_auth_header_well_formed() {
        let auth = aws_sigv4_s3_auth(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "20130524",
            "20130524T000000Z",
            "GET",
            "examplebucket.s3.amazonaws.com",
            "/",
            "list-type=2",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        let expected_prefix = "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request,SignedHeaders=host;x-amz-content-sha256;x-amz-date,Signature=";
        assert!(
            auth.starts_with(expected_prefix),
            "bad header prefix: {auth}"
        );
        let sig = auth.split("Signature=").nth(1).unwrap_or("");
        assert_eq!(
            sig.len(),
            64,
            "signature must be 64 hex chars, got {}",
            sig.len()
        );
        assert!(
            sig.chars().all(|c| c.is_ascii_hexdigit()),
            "signature is not hex"
        );
        assert_ne!(sig, "placeholder");
    }

    #[test]
    fn aws_sigv4_s3_auth_is_deterministic() {
        let (aki, sak, reg, ds, dts, meth, host, uri, qry, ph) = (
            "AKID",
            "SECRET",
            "us-west-2",
            "20260101",
            "20260101T120000Z",
            "PUT",
            "mybucket.s3.us-west-2.amazonaws.com",
            "/mykey.txt",
            "",
            "abc123hash",
        );
        let a1 = aws_sigv4_s3_auth(aki, sak, reg, ds, dts, meth, host, uri, qry, ph);
        let a2 = aws_sigv4_s3_auth(aki, sak, reg, ds, dts, meth, host, uri, qry, ph);
        assert_eq!(a1, a2);
    }

    #[test]
    fn aws_sigv4_s3_auth_differs_by_secret() {
        let (aki, reg, ds, dts, meth, host, uri, qry, ph) = (
            "AKID",
            "us-east-1",
            "20260101",
            "20260101T000000Z",
            "GET",
            "b.s3.amazonaws.com",
            "/",
            "",
            "emptyhash",
        );
        let a1 = aws_sigv4_s3_auth(aki, "SECRET1", reg, ds, dts, meth, host, uri, qry, ph);
        let a2 = aws_sigv4_s3_auth(aki, "SECRET2", reg, ds, dts, meth, host, uri, qry, ph);
        assert_ne!(
            a1, a2,
            "different secrets must produce different signatures"
        );
    }

    // ── Hugging Face ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn huggingface_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h1".into(),
            node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"model":"gpt2","inputs":"hello"})),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn huggingface_fails_without_model() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h2".into(),
            node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"api_token":"hf_test","inputs":"hello"})),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("model"));
    }

    #[tokio::test]
    async fn huggingface_inference_fails_without_inputs() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h3".into(),
            node_type: NodeType::Huggingface,
            config: Some(
                serde_json::json!({"api_token":"hf_test","model":"gpt2","operation":"inference"}),
            ),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("inputs"));
    }

    #[tokio::test]
    async fn huggingface_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h4".into(),
            node_type: NodeType::Huggingface,
            config: Some(
                serde_json::json!({"api_token":"hf_test","model":"gpt2","operation":"bad"}),
            ),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }
}
