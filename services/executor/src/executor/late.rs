// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── Slice 274: Twitch ─────────────────────────────────────────────────────────

pub(super) async fn execute_twitch(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Twitch node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let client_id = match cfg.get("client_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Twitch node missing 'client_id'"),
    };
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Twitch node missing 'access_token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Twitch node missing 'endpoint' (e.g. /helix/streams)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://api.twitch.tv{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Client-ID", &client_id)
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
        Err(e) => NodeExecutionResult::failed(format!("Twitch request error: {e}")),
    }
}

// ── Slice 275: Figma ──────────────────────────────────────────────────────────

pub(super) async fn execute_figma(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Figma node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Figma node missing 'token' (personal access token)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Figma node missing 'endpoint' (e.g. /v1/files/KEY)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://api.figma.com{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("X-Figma-Token", &token)
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
        Err(e) => NodeExecutionResult::failed(format!("Figma request error: {e}")),
    }
}

// ── Slice 276: Dropbox ────────────────────────────────────────────────────────

pub(super) async fn execute_dropbox(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Dropbox node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Dropbox node missing 'token' (OAuth2 access token)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_folder");

    // Different Dropbox API endpoints per operation
    let (host, path, arg): (&str, &str, serde_json::Value) = match operation {
        "list_folder" => {
            let folder = cfg.get("path").and_then(|v| v.as_str()).unwrap_or("");
            (
                "api.dropboxapi.com",
                "/2/files/list_folder",
                serde_json::json!({ "path": folder, "recursive": false }),
            )
        }
        "get_metadata" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox get_metadata requires 'path'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/get_metadata",
                serde_json::json!({ "path": p }),
            )
        }
        "delete" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox delete requires 'path'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/delete_v2",
                serde_json::json!({ "path": p }),
            )
        }
        "create_folder" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox create_folder requires 'path'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/create_folder_v2",
                serde_json::json!({ "path": p, "autorename": false }),
            )
        }
        "search" => {
            let q = match cfg.get("query").and_then(|v| v.as_str()) {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox search requires 'query'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/search_v2",
                serde_json::json!({ "query": q }),
            )
        }
        _ => return NodeExecutionResult::failed(format!("Unknown Dropbox operation: {operation}")),
    };

    let url = format!("https://{host}{path}");

    match client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&arg)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body, "operation": operation })
                    .to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Dropbox request error: {e}")),
    }
}

// ── Slice 277: Cloudflare ─────────────────────────────────────────────────────

pub(super) async fn execute_cloudflare(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Cloudflare node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Cloudflare node missing 'api_token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Cloudflare node missing 'endpoint' (e.g. /zones/ZONE_ID/dns_records)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://api.cloudflare.com/client/v4{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {api_token}"))
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
            // Cloudflare wraps responses in {success, errors, messages, result}
            let success = body
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body, "success": success })
                    .to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Cloudflare request error: {e}")),
    }
}

#[cfg(test)]
mod tests_274_277 {
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

    // ── Twitch ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn twitch_fails_without_client_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tw1".into(),
            node_type: NodeType::Twitch,
            config: Some(
                serde_json::json!({ "access_token": "tok", "endpoint": "/helix/streams" }),
            ),
        };
        let r = execute_twitch(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_id"));
    }

    #[tokio::test]
    async fn twitch_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tw2".into(),
            node_type: NodeType::Twitch,
            config: Some(serde_json::json!({ "client_id": "cid", "access_token": "tok" })),
        };
        let r = execute_twitch(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Figma ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn figma_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fg1".into(),
            node_type: NodeType::Figma,
            config: Some(serde_json::json!({ "endpoint": "/v1/files/KEY" })),
        };
        let r = execute_figma(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn figma_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fg2".into(),
            node_type: NodeType::Figma,
            config: Some(serde_json::json!({ "token": "figd_abc" })),
        };
        let r = execute_figma(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Dropbox ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn dropbox_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "db1".into(),
            node_type: NodeType::Dropbox,
            config: Some(serde_json::json!({ "operation": "list_folder" })),
        };
        let r = execute_dropbox(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn dropbox_rejects_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "db2".into(),
            node_type: NodeType::Dropbox,
            config: Some(serde_json::json!({ "token": "sl.abc", "operation": "unknown_op" })),
        };
        let r = execute_dropbox(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }

    // ── Cloudflare ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cloudflare_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cf1".into(),
            node_type: NodeType::Cloudflare,
            config: Some(serde_json::json!({ "endpoint": "/zones" })),
        };
        let r = execute_cloudflare(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn cloudflare_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cf2".into(),
            node_type: NodeType::Cloudflare,
            config: Some(serde_json::json!({ "api_token": "abc123" })),
        };
        let r = execute_cloudflare(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 278: Box ────────────────────────────────────────────────────────────

pub(super) async fn execute_box(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Box node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Box node missing 'token' (OAuth2 access token)"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Box node missing 'endpoint' (e.g. /folders/0/items)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://api.box.com/2.0{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Box request error: {e}")),
    }
}

// ── Slice 279: Okta ───────────────────────────────────────────────────────────

pub(super) async fn execute_okta(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Okta node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let domain = match cfg.get("domain").and_then(|v| v.as_str()) {
        Some(d) if !d.is_empty() => d.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed("Okta node missing 'domain' (e.g. myco.okta.com)"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Okta node missing 'token' (SSWS API token or Bearer OAuth)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed("Okta node missing 'endpoint' (e.g. /api/v1/users)")
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let token_type = cfg
        .get("token_type")
        .and_then(|v| v.as_str())
        .unwrap_or("SSWS");

    let auth_value = if token_type.to_uppercase() == "BEARER" {
        format!("Bearer {token}")
    } else {
        format!("SSWS {token}")
    };

    let url = format!("https://{domain}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", auth_value)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

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
        Err(e) => NodeExecutionResult::failed(format!("Okta request error: {e}")),
    }
}

// ── Slice 280: Zoom ───────────────────────────────────────────────────────────

pub(super) async fn execute_zoom(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Zoom node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Zoom node missing 'token' (OAuth2 access token or JWT)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Zoom node missing 'endpoint' (e.g. /users/me/meetings)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://api.zoom.us/v2{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Zoom request error: {e}")),
    }
}

// ── Slice 281: Spotify ────────────────────────────────────────────────────────

pub(super) async fn execute_spotify(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Spotify node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Spotify node missing 'token' (OAuth2 access token)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Spotify node missing 'endpoint' (e.g. /me/player/currently-playing)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://api.spotify.com/v1{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            // Spotify returns 204 No Content for some operations
            let body: serde_json::Value = if status == 204 {
                serde_json::Value::Null
            } else {
                resp.json().await.unwrap_or(serde_json::Value::Null)
            };
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Spotify request error: {e}")),
    }
}

#[cfg(test)]
mod tests_278_281 {
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

    // ── Box ───────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn box_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bx1".into(),
            node_type: NodeType::Box,
            config: Some(serde_json::json!({ "endpoint": "/folders/0/items" })),
        };
        let r = execute_box(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn box_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bx2".into(),
            node_type: NodeType::Box,
            config: Some(serde_json::json!({ "token": "abc" })),
        };
        let r = execute_box(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Okta ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn okta_fails_without_domain() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ok1".into(),
            node_type: NodeType::Okta,
            config: Some(serde_json::json!({ "token": "tok", "endpoint": "/api/v1/users" })),
        };
        let r = execute_okta(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("domain"));
    }

    #[tokio::test]
    async fn okta_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ok2".into(),
            node_type: NodeType::Okta,
            config: Some(
                serde_json::json!({ "domain": "myco.okta.com", "endpoint": "/api/v1/users" }),
            ),
        };
        let r = execute_okta(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    // ── Zoom ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn zoom_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "zm1".into(),
            node_type: NodeType::Zoom,
            config: Some(serde_json::json!({ "endpoint": "/users/me/meetings" })),
        };
        let r = execute_zoom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn zoom_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "zm2".into(),
            node_type: NodeType::Zoom,
            config: Some(serde_json::json!({ "token": "tok" })),
        };
        let r = execute_zoom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Spotify ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn spotify_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sp1".into(),
            node_type: NodeType::Spotify,
            config: Some(serde_json::json!({ "endpoint": "/me/player/currently-playing" })),
        };
        let r = execute_spotify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn spotify_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sp2".into(),
            node_type: NodeType::Spotify,
            config: Some(serde_json::json!({ "token": "BQD…" })),
        };
        let r = execute_spotify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 282: Typeform ────────────────────────────────────────────────────────

pub(super) async fn execute_typeform(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Typeform requires 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Typeform requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://api.typeform.com{}", endpoint);

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Typeform request error: {e}")),
    }
}

// ── Slice 283: Webflow ─────────────────────────────────────────────────────────

pub(super) async fn execute_webflow(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Webflow requires 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Webflow requires 'endpoint'"),
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
    let url = format!("https://api.webflow.com/v2{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("accept-version", "1.0.0")
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
        Err(e) => NodeExecutionResult::failed(format!("Webflow request error: {e}")),
    }
}

// ── Slice 284: Intercom ────────────────────────────────────────────────────────

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

#[cfg(test)]
mod tests_282_285 {
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

    // ── Typeform ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn typeform_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tf1".into(),
            node_type: NodeType::Typeform,
            config: Some(serde_json::json!({ "endpoint": "/forms/FORM_ID/responses" })),
        };
        let r = execute_typeform(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn typeform_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tf2".into(),
            node_type: NodeType::Typeform,
            config: Some(serde_json::json!({ "token": "tfp_abc" })),
        };
        let r = execute_typeform(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Webflow ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn webflow_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "wf1".into(),
            node_type: NodeType::Webflow,
            config: Some(serde_json::json!({ "endpoint": "/sites" })),
        };
        let r = execute_webflow(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn webflow_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "wf2".into(),
            node_type: NodeType::Webflow,
            config: Some(serde_json::json!({ "token": "abcdef" })),
        };
        let r = execute_webflow(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Intercom ──────────────────────────────────────────────────────────────

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
}

// ── Slice 286: Trello ──────────────────────────────────────────────────────────

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

pub(super) async fn execute_amplitude(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Amplitude requires 'api_key'"),
    };
    let secret_key = match cfg.get("secret_key").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Amplitude requires 'secret_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("track")
        .to_string();

    let (url, body) = match operation.as_str() {
        "track" => {
            let events = cfg
                .get("events")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!({ "api_key": api_key, "events": events });
            ("https://api2.amplitude.com/2/httpapi".to_string(), b)
        }
        "identify" => {
            let identification = cfg
                .get("identification")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!({ "api_key": api_key, "identification": identification });
            ("https://api.amplitude.com/identify".to_string(), b)
        }
        "export" => {
            let start = cfg
                .get("start")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let end = cfg
                .get("end")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let credentials = format!("{api_key}:{secret_key}");
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
            let export_url = format!("https://amplitude.com/api/2/export?start={start}&end={end}");
            return match http_client
                .get(&export_url)
                .header("Authorization", format!("Basic {encoded}"))
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
                Err(e) => NodeExecutionResult::failed(format!("Amplitude export error: {e}")),
            };
        }
        _ => {
            return NodeExecutionResult::failed(format!(
                "Amplitude unknown operation '{operation}'"
            ))
        }
    };

    match http_client
        .post(&url)
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
        Err(e) => NodeExecutionResult::failed(format!("Amplitude request error: {e}")),
    }
}

#[cfg(test)]
mod tests_286_289 {
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

    // ── Trello ────────────────────────────────────────────────────────────────

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
    async fn amplitude_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "am1".into(),
            node_type: NodeType::Amplitude,
            config: Some(serde_json::json!({ "secret_key": "sec", "operation": "track" })),
        };
        let r = execute_amplitude(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn amplitude_fails_without_secret_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "am2".into(),
            node_type: NodeType::Amplitude,
            config: Some(serde_json::json!({ "api_key": "key", "operation": "track" })),
        };
        let r = execute_amplitude(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("secret_key"));
    }

    #[tokio::test]
    async fn amplitude_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "am3".into(),
            node_type: NodeType::Amplitude,
            config: Some(
                serde_json::json!({ "api_key": "key", "secret_key": "sec", "operation": "bogus" }),
            ),
        };
        let r = execute_amplitude(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }
}

// ── Slice 290: Mixpanel ────────────────────────────────────────────────────────

pub(super) async fn execute_mixpanel(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let project_token = match cfg.get("project_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Mixpanel requires 'project_token'"),
    };
    let api_secret = match cfg.get("api_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Mixpanel requires 'api_secret'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("track")
        .to_string();

    use base64::Engine as _;
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("{api_secret}:").as_bytes());

    let (url, body) = match operation.as_str() {
        "track" => {
            let events = cfg
                .get("events")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!({ "project_token": project_token, "events": events });
            (
                "https://api.mixpanel.com/track#live-event-import".to_string(),
                b,
            )
        }
        "import" => {
            let events = cfg
                .get("events")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!(events);
            (
                format!("https://api.mixpanel.com/import?project_token={project_token}"),
                b,
            )
        }
        "query" => {
            let endpoint = cfg
                .get("endpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("/api/query")
                .to_string();
            let params = cfg
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let url = format!("https://mixpanel.com/api/2.0{endpoint}");
            return match http_client
                .post(&url)
                .header("Authorization", format!("Basic {encoded}"))
                .header("Content-Type", "application/json")
                .json(&params)
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
                Err(e) => NodeExecutionResult::failed(format!("Mixpanel query error: {e}")),
            };
        }
        _ => {
            return NodeExecutionResult::failed(format!("Mixpanel unknown operation '{operation}'"))
        }
    };

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
            let resp_body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": resp_body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Mixpanel request error: {e}")),
    }
}

// ── Slice 291: Segment ─────────────────────────────────────────────────────────

pub(super) async fn execute_segment(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let write_key = match cfg.get("write_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Segment requires 'write_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("track")
        .to_string();

    use base64::Engine as _;
    // Segment Basic auth: base64(write_key:)
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("{write_key}:").as_bytes());

    let endpoint = match operation.as_str() {
        "track" => "/v1/track",
        "identify" => "/v1/identify",
        "page" => "/v1/page",
        "group" => "/v1/group",
        "alias" => "/v1/alias",
        "batch" => "/v1/batch",
        _ => {
            return NodeExecutionResult::failed(format!("Segment unknown operation '{operation}'"))
        }
    };

    let body = cfg
        .get("body")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let url = format!("https://api.segment.io{endpoint}");

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
            let resp_body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": resp_body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Segment request error: {e}")),
    }
}

// ── Slice 292: SendGrid ────────────────────────────────────────────────────────

pub(super) async fn execute_sendgrid(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("SendGrid requires 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("SendGrid requires 'endpoint'"),
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
    let url = format!("https://api.sendgrid.com/v3{ep}");

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
            let body: serde_json::Value = if status == 202 || status == 204 {
                serde_json::Value::Null
            } else {
                resp.json().await.unwrap_or(serde_json::Value::Null)
            };
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("SendGrid request error: {e}")),
    }
}

// ── Slice 293: Braintree ───────────────────────────────────────────────────────

pub(super) async fn execute_braintree(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let merchant_id = match cfg.get("merchant_id").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'merchant_id'"),
    };
    let public_key = match cfg.get("public_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'public_key'"),
    };
    let private_key = match cfg.get("private_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'private_key'"),
    };
    let environment = cfg
        .get("environment")
        .and_then(|v| v.as_str())
        .unwrap_or("sandbox")
        .to_string();
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    use base64::Engine as _;
    let credentials = format!("{public_key}:{private_key}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());

    let base_url = if environment == "production" {
        format!("https://api.braintreegateway.com/merchants/{merchant_id}")
    } else {
        format!("https://api.sandbox.braintreegateway.com/merchants/{merchant_id}")
    };
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("{base_url}{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {encoded}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Braintree-Version", "2019-01-01");

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
        Err(e) => NodeExecutionResult::failed(format!("Braintree request error: {e}")),
    }
}

#[cfg(test)]
mod tests_290_293 {
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

    // ── Mixpanel ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn mixpanel_fails_without_project_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "mp1".into(),
            node_type: NodeType::Mixpanel,
            config: Some(serde_json::json!({ "api_secret": "sec", "operation": "track" })),
        };
        let r = execute_mixpanel(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_token"));
    }

    #[tokio::test]
    async fn mixpanel_fails_without_api_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "mp2".into(),
            node_type: NodeType::Mixpanel,
            config: Some(serde_json::json!({ "project_token": "tok", "operation": "track" })),
        };
        let r = execute_mixpanel(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_secret"));
    }

    #[tokio::test]
    async fn mixpanel_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "mp3".into(),
            node_type: NodeType::Mixpanel,
            config: Some(
                serde_json::json!({ "project_token": "tok", "api_secret": "sec", "operation": "bogus" }),
            ),
        };
        let r = execute_mixpanel(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }

    // ── Segment ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn segment_fails_without_write_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sg1".into(),
            node_type: NodeType::Segment,
            config: Some(serde_json::json!({ "operation": "track" })),
        };
        let r = execute_segment(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("write_key"));
    }

    #[tokio::test]
    async fn segment_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sg2".into(),
            node_type: NodeType::Segment,
            config: Some(serde_json::json!({ "write_key": "wk_abc", "operation": "bogus" })),
        };
        let r = execute_segment(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }

    // ── SendGrid ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn sendgrid_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sg3".into(),
            node_type: NodeType::Sendgrid,
            config: Some(serde_json::json!({ "endpoint": "/mail/send" })),
        };
        let r = execute_sendgrid(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn sendgrid_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sg4".into(),
            node_type: NodeType::Sendgrid,
            config: Some(serde_json::json!({ "api_key": "SG.abc" })),
        };
        let r = execute_sendgrid(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Braintree ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn braintree_fails_without_merchant_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bt1".into(),
            node_type: NodeType::Braintree,
            config: Some(
                serde_json::json!({ "public_key": "pk", "private_key": "prk", "endpoint": "/transactions" }),
            ),
        };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("merchant_id"));
    }

    #[tokio::test]
    async fn braintree_fails_without_public_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bt2".into(),
            node_type: NodeType::Braintree,
            config: Some(
                serde_json::json!({ "merchant_id": "mid", "private_key": "prk", "endpoint": "/transactions" }),
            ),
        };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("public_key"));
    }

    #[tokio::test]
    async fn braintree_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bt3".into(),
            node_type: NodeType::Braintree,
            config: Some(
                serde_json::json!({ "merchant_id": "mid", "public_key": "pk", "private_key": "prk" }),
            ),
        };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 294: PayPal ──────────────────────────────────────────────────────────

pub(super) async fn execute_paypal(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let client_id = match cfg.get("client_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("PayPal requires 'client_id'"),
    };
    let client_secret = match cfg.get("client_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("PayPal requires 'client_secret'"),
    };
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            // Obtain token via client_credentials grant
            let environment = cfg
                .get("environment")
                .and_then(|v| v.as_str())
                .unwrap_or("sandbox");
            let token_url = if environment == "live" {
                "https://api-m.paypal.com/v1/oauth2/token"
            } else {
                "https://api-m.sandbox.paypal.com/v1/oauth2/token"
            };
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{client_id}:{client_secret}").as_bytes());
            match http_client
                .post(token_url)
                .header("Authorization", format!("Basic {encoded}"))
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body("grant_type=client_credentials")
                .send()
                .await
            {
                Ok(resp) => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    match json.get("access_token").and_then(|v| v.as_str()) {
                        Some(t) => t.to_string(),
                        None => return NodeExecutionResult::failed("PayPal token exchange failed"),
                    }
                }
                Err(e) => {
                    return NodeExecutionResult::failed(format!("PayPal token exchange error: {e}"))
                }
            }
        }
    };

    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("PayPal requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let environment = cfg
        .get("environment")
        .and_then(|v| v.as_str())
        .unwrap_or("sandbox");
    let base = if environment == "live" {
        "https://api-m.paypal.com"
    } else {
        "https://api-m.sandbox.paypal.com"
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
        Err(e) => NodeExecutionResult::failed(format!("PayPal request error: {e}")),
    }
}

// ── Slice 295: Razorpay ────────────────────────────────────────────────────────

pub(super) async fn execute_razorpay(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let key_id = match cfg.get("key_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Razorpay requires 'key_id'"),
    };
    let key_secret = match cfg.get("key_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Razorpay requires 'key_secret'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Razorpay requires 'endpoint'"),
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
    let url = format!("https://api.razorpay.com/v1{ep}");

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("{key_id}:{key_secret}").as_bytes());

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
        Err(e) => NodeExecutionResult::failed(format!("Razorpay request error: {e}")),
    }
}

// ── Slice 296: Firebase ────────────────────────────────────────────────────────

pub(super) async fn execute_firebase(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let project_id = match cfg.get("project_id").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("Firebase requires 'project_id'"),
    };
    let id_token = match cfg.get("id_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Firebase requires 'id_token'"),
    };
    let service = cfg
        .get("service")
        .and_then(|v| v.as_str())
        .unwrap_or("firestore")
        .to_string();
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Firebase requires 'endpoint'"),
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

    let url = match service.as_str() {
        "firestore" => format!("https://firestore.googleapis.com/v1/projects/{project_id}/databases/(default)/documents{ep}"),
        "rtdb" => {
            let db_url = cfg.get("database_url").and_then(|v| v.as_str())
                .unwrap_or("https://PROJECT.firebaseio.com");
            format!("{db_url}{ep}.json?auth={id_token}")
        }
        "storage" => format!("https://firebasestorage.googleapis.com/v0/b/{project_id}.appspot.com/o{ep}"),
        _ => return NodeExecutionResult::failed(format!("Firebase unknown service '{service}'")),
    };

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Content-Type", "application/json");

    // RTDB embeds auth in URL; Firestore/Storage use Bearer header
    if service != "rtdb" {
        req = req.header("Authorization", format!("Bearer {id_token}"));
    }

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
        Err(e) => NodeExecutionResult::failed(format!("Firebase request error: {e}")),
    }
}

// ── Slice 297: Supabase ────────────────────────────────────────────────────────

pub(super) async fn execute_supabase(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let project_url = match cfg.get("project_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("Supabase requires 'project_url'"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Supabase requires 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Supabase requires 'endpoint'"),
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
    let base = project_url.trim_end_matches('/');
    let url = format!("{base}{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("apikey", &api_key)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json");

    // PostgREST Prefer header for upsert/returning
    if let Some(prefer) = cfg.get("prefer").and_then(|v| v.as_str()) {
        req = req.header("Prefer", prefer);
    }

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
        Err(e) => NodeExecutionResult::failed(format!("Supabase request error: {e}")),
    }
}

#[cfg(test)]
mod tests_294_297 {
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

    // ── PayPal ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn paypal_fails_without_client_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pp1".into(),
            node_type: NodeType::Paypal,
            config: Some(
                serde_json::json!({ "client_secret": "sec", "endpoint": "/v2/checkout/orders" }),
            ),
        };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_id"));
    }

    #[tokio::test]
    async fn paypal_fails_without_client_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pp2".into(),
            node_type: NodeType::Paypal,
            config: Some(
                serde_json::json!({ "client_id": "cid", "endpoint": "/v2/checkout/orders" }),
            ),
        };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_secret"));
    }

    #[tokio::test]
    async fn paypal_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pp3".into(),
            node_type: NodeType::Paypal,
            config: Some(
                serde_json::json!({ "client_id": "cid", "client_secret": "sec", "access_token": "tok" }),
            ),
        };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Razorpay ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn razorpay_fails_without_key_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rp1".into(),
            node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_secret": "sec", "endpoint": "/orders" })),
        };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key_id"));
    }

    #[tokio::test]
    async fn razorpay_fails_without_key_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rp2".into(),
            node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_id": "rzp_test_abc", "endpoint": "/orders" })),
        };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key_secret"));
    }

    #[tokio::test]
    async fn razorpay_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rp3".into(),
            node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_id": "rzp_test_abc", "key_secret": "sec" })),
        };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Firebase ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn firebase_fails_without_project_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fb1".into(),
            node_type: NodeType::Firebase,
            config: Some(serde_json::json!({ "id_token": "tok", "endpoint": "/users" })),
        };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_id"));
    }

    #[tokio::test]
    async fn firebase_fails_without_id_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fb2".into(),
            node_type: NodeType::Firebase,
            config: Some(serde_json::json!({ "project_id": "my-proj", "endpoint": "/users" })),
        };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("id_token"));
    }

    #[tokio::test]
    async fn firebase_unknown_service() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fb3".into(),
            node_type: NodeType::Firebase,
            config: Some(
                serde_json::json!({ "project_id": "proj", "id_token": "tok", "endpoint": "/doc", "service": "bogus" }),
            ),
        };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }

    // ── Supabase ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn supabase_fails_without_project_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sb1".into(),
            node_type: NodeType::Supabase,
            config: Some(serde_json::json!({ "api_key": "eyJ…", "endpoint": "/rest/v1/users" })),
        };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_url"));
    }

    #[tokio::test]
    async fn supabase_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sb2".into(),
            node_type: NodeType::Supabase,
            config: Some(
                serde_json::json!({ "project_url": "https://xyz.supabase.co", "endpoint": "/rest/v1/users" }),
            ),
        };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn supabase_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sb3".into(),
            node_type: NodeType::Supabase,
            config: Some(
                serde_json::json!({ "project_url": "https://xyz.supabase.co", "api_key": "eyJ…" }),
            ),
        };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}
