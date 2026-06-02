// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

// ── Slice 274: Twitch ─────────────────────────────────────────────────────────

async fn execute_twitch(
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
        _ => return NodeExecutionResult::failed("Twitch node missing 'endpoint' (e.g. /helix/streams)"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Twitch request error: {e}")),
    }
}

// ── Slice 275: Figma ──────────────────────────────────────────────────────────

async fn execute_figma(
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
        _ => return NodeExecutionResult::failed("Figma node missing 'token' (personal access token)"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Figma node missing 'endpoint' (e.g. /v1/files/KEY)"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Figma request error: {e}")),
    }
}

// ── Slice 276: Dropbox ────────────────────────────────────────────────────────

async fn execute_dropbox(
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
        _ => return NodeExecutionResult::failed("Dropbox node missing 'token' (OAuth2 access token)"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list_folder");

    // Different Dropbox API endpoints per operation
    let (host, path, arg): (&str, &str, serde_json::Value) = match operation {
        "list_folder" => {
            let folder = cfg.get("path").and_then(|v| v.as_str()).unwrap_or("");
            ("api.dropboxapi.com", "/2/files/list_folder",
             serde_json::json!({ "path": folder, "recursive": false }))
        }
        "get_metadata" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox get_metadata requires 'path'"),
            };
            ("api.dropboxapi.com", "/2/files/get_metadata",
             serde_json::json!({ "path": p }))
        }
        "delete" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox delete requires 'path'"),
            };
            ("api.dropboxapi.com", "/2/files/delete_v2",
             serde_json::json!({ "path": p }))
        }
        "create_folder" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox create_folder requires 'path'"),
            };
            ("api.dropboxapi.com", "/2/files/create_folder_v2",
             serde_json::json!({ "path": p, "autorename": false }))
        }
        "search" => {
            let q = match cfg.get("query").and_then(|v| v.as_str()) {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox search requires 'query'"),
            };
            ("api.dropboxapi.com", "/2/files/search_v2",
             serde_json::json!({ "query": q }))
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
                serde_json::json!({ "status": status, "body": body, "operation": operation }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Dropbox request error: {e}")),
    }
}

// ── Slice 277: Cloudflare ─────────────────────────────────────────────────────

async fn execute_cloudflare(
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
        _ => return NodeExecutionResult::failed("Cloudflare node missing 'endpoint' (e.g. /zones/ZONE_ID/dns_records)"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();

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
            let success = body.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body, "success": success }).to_string()
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
        let n = Node { id: "tw1".into(), node_type: NodeType::Twitch,
            config: Some(serde_json::json!({ "access_token": "tok", "endpoint": "/helix/streams" })) };
        let r = execute_twitch(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_id"));
    }

    #[tokio::test]
    async fn twitch_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "tw2".into(), node_type: NodeType::Twitch,
            config: Some(serde_json::json!({ "client_id": "cid", "access_token": "tok" })) };
        let r = execute_twitch(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Figma ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn figma_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "fg1".into(), node_type: NodeType::Figma,
            config: Some(serde_json::json!({ "endpoint": "/v1/files/KEY" })) };
        let r = execute_figma(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn figma_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "fg2".into(), node_type: NodeType::Figma,
            config: Some(serde_json::json!({ "token": "figd_abc" })) };
        let r = execute_figma(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Dropbox ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn dropbox_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "db1".into(), node_type: NodeType::Dropbox,
            config: Some(serde_json::json!({ "operation": "list_folder" })) };
        let r = execute_dropbox(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn dropbox_rejects_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "db2".into(), node_type: NodeType::Dropbox,
            config: Some(serde_json::json!({ "token": "sl.abc", "operation": "unknown_op" })) };
        let r = execute_dropbox(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }

    // ── Cloudflare ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cloudflare_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "cf1".into(), node_type: NodeType::Cloudflare,
            config: Some(serde_json::json!({ "endpoint": "/zones" })) };
        let r = execute_cloudflare(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn cloudflare_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "cf2".into(), node_type: NodeType::Cloudflare,
            config: Some(serde_json::json!({ "api_token": "abc123" })) };
        let r = execute_cloudflare(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 278: Box ────────────────────────────────────────────────────────────

async fn execute_box(
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
        _ => return NodeExecutionResult::failed("Box node missing 'endpoint' (e.g. /folders/0/items)"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Box request error: {e}")),
    }
}

// ── Slice 279: Okta ───────────────────────────────────────────────────────────

async fn execute_okta(
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
        _ => return NodeExecutionResult::failed("Okta node missing 'token' (SSWS API token or Bearer OAuth)"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Okta node missing 'endpoint' (e.g. /api/v1/users)"),
    };
    let method    = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let token_type = cfg.get("token_type").and_then(|v| v.as_str()).unwrap_or("SSWS");

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Okta request error: {e}")),
    }
}

// ── Slice 280: Zoom ───────────────────────────────────────────────────────────

async fn execute_zoom(
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
        _ => return NodeExecutionResult::failed("Zoom node missing 'token' (OAuth2 access token or JWT)"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Zoom node missing 'endpoint' (e.g. /users/me/meetings)"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Zoom request error: {e}")),
    }
}

// ── Slice 281: Spotify ────────────────────────────────────────────────────────

async fn execute_spotify(
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
        _ => return NodeExecutionResult::failed("Spotify node missing 'token' (OAuth2 access token)"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Spotify node missing 'endpoint' (e.g. /me/player/currently-playing)"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();

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
                serde_json::json!({ "status": status, "body": body }).to_string()
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
        let n = Node { id: "bx1".into(), node_type: NodeType::Box,
            config: Some(serde_json::json!({ "endpoint": "/folders/0/items" })) };
        let r = execute_box(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn box_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "bx2".into(), node_type: NodeType::Box,
            config: Some(serde_json::json!({ "token": "abc" })) };
        let r = execute_box(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Okta ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn okta_fails_without_domain() {
        let c = reqwest::Client::new();
        let n = Node { id: "ok1".into(), node_type: NodeType::Okta,
            config: Some(serde_json::json!({ "token": "tok", "endpoint": "/api/v1/users" })) };
        let r = execute_okta(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("domain"));
    }

    #[tokio::test]
    async fn okta_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "ok2".into(), node_type: NodeType::Okta,
            config: Some(serde_json::json!({ "domain": "myco.okta.com", "endpoint": "/api/v1/users" })) };
        let r = execute_okta(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    // ── Zoom ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn zoom_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "zm1".into(), node_type: NodeType::Zoom,
            config: Some(serde_json::json!({ "endpoint": "/users/me/meetings" })) };
        let r = execute_zoom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn zoom_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "zm2".into(), node_type: NodeType::Zoom,
            config: Some(serde_json::json!({ "token": "tok" })) };
        let r = execute_zoom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Spotify ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn spotify_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "sp1".into(), node_type: NodeType::Spotify,
            config: Some(serde_json::json!({ "endpoint": "/me/player/currently-playing" })) };
        let r = execute_spotify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn spotify_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "sp2".into(), node_type: NodeType::Spotify,
            config: Some(serde_json::json!({ "token": "BQD…" })) };
        let r = execute_spotify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 282: Typeform ────────────────────────────────────────────────────────

async fn execute_typeform(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Typeform request error: {e}")),
    }
}

// ── Slice 283: Webflow ─────────────────────────────────────────────────────────

async fn execute_webflow(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Webflow request error: {e}")),
    }
}

// ── Slice 284: Intercom ────────────────────────────────────────────────────────

async fn execute_intercom(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Intercom request error: {e}")),
    }
}

// ── Slice 285: Pipedrive ───────────────────────────────────────────────────────

async fn execute_pipedrive(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
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
        let n = Node { id: "tf1".into(), node_type: NodeType::Typeform,
            config: Some(serde_json::json!({ "endpoint": "/forms/FORM_ID/responses" })) };
        let r = execute_typeform(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn typeform_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "tf2".into(), node_type: NodeType::Typeform,
            config: Some(serde_json::json!({ "token": "tfp_abc" })) };
        let r = execute_typeform(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Webflow ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn webflow_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "wf1".into(), node_type: NodeType::Webflow,
            config: Some(serde_json::json!({ "endpoint": "/sites" })) };
        let r = execute_webflow(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn webflow_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "wf2".into(), node_type: NodeType::Webflow,
            config: Some(serde_json::json!({ "token": "abcdef" })) };
        let r = execute_webflow(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Intercom ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn intercom_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "ic1".into(), node_type: NodeType::Intercom,
            config: Some(serde_json::json!({ "endpoint": "/contacts" })) };
        let r = execute_intercom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn intercom_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "ic2".into(), node_type: NodeType::Intercom,
            config: Some(serde_json::json!({ "token": "dG9rZW4…" })) };
        let r = execute_intercom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Pipedrive ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn pipedrive_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "pd1".into(), node_type: NodeType::Pipedrive,
            config: Some(serde_json::json!({ "endpoint": "/deals" })) };
        let r = execute_pipedrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn pipedrive_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "pd2".into(), node_type: NodeType::Pipedrive,
            config: Some(serde_json::json!({ "api_token": "abc123" })) };
        let r = execute_pipedrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 286: Trello ──────────────────────────────────────────────────────────

async fn execute_trello(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Trello request error: {e}")),
    }
}

// ── Slice 287: Monday ──────────────────────────────────────────────────────────

async fn execute_monday(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let variables = cfg.get("variables").cloned().unwrap_or(serde_json::Value::Null);

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Monday request error: {e}")),
    }
}

// ── Slice 288: ClickUp ─────────────────────────────────────────────────────────

async fn execute_clickup(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("ClickUp request error: {e}")),
    }
}

// ── Slice 289: Amplitude ───────────────────────────────────────────────────────

async fn execute_amplitude(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("track").to_string();

    let (url, body) = match operation.as_str() {
        "track" => {
            let events = cfg.get("events").cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!({ "api_key": api_key, "events": events });
            ("https://api2.amplitude.com/2/httpapi".to_string(), b)
        }
        "identify" => {
            let identification = cfg.get("identification").cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!({ "api_key": api_key, "identification": identification });
            ("https://api.amplitude.com/identify".to_string(), b)
        }
        "export" => {
            let start = cfg.get("start").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let end   = cfg.get("end").and_then(|v| v.as_str()).unwrap_or("").to_string();
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Amplitude export error: {e}")),
            };
        }
        _ => return NodeExecutionResult::failed(format!("Amplitude unknown operation '{operation}'")),
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
                serde_json::json!({ "status": status, "body": resp_body }).to_string()
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
        let n = Node { id: "tr1".into(), node_type: NodeType::Trello,
            config: Some(serde_json::json!({ "token": "tok", "endpoint": "/boards" })) };
        let r = execute_trello(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn trello_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "tr2".into(), node_type: NodeType::Trello,
            config: Some(serde_json::json!({ "api_key": "key", "endpoint": "/boards" })) };
        let r = execute_trello(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn trello_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "tr3".into(), node_type: NodeType::Trello,
            config: Some(serde_json::json!({ "api_key": "key", "token": "tok" })) };
        let r = execute_trello(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Monday ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn monday_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "mn1".into(), node_type: NodeType::Monday,
            config: Some(serde_json::json!({ "query": "{ boards { id } }" })) };
        let r = execute_monday(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn monday_fails_without_query() {
        let c = reqwest::Client::new();
        let n = Node { id: "mn2".into(), node_type: NodeType::Monday,
            config: Some(serde_json::json!({ "token": "eyJhbGci…" })) };
        let r = execute_monday(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    // ── ClickUp ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn clickup_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "cu1".into(), node_type: NodeType::Clickup,
            config: Some(serde_json::json!({ "endpoint": "/team" })) };
        let r = execute_clickup(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn clickup_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "cu2".into(), node_type: NodeType::Clickup,
            config: Some(serde_json::json!({ "token": "pk_abc" })) };
        let r = execute_clickup(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Amplitude ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn amplitude_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "am1".into(), node_type: NodeType::Amplitude,
            config: Some(serde_json::json!({ "secret_key": "sec", "operation": "track" })) };
        let r = execute_amplitude(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn amplitude_fails_without_secret_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "am2".into(), node_type: NodeType::Amplitude,
            config: Some(serde_json::json!({ "api_key": "key", "operation": "track" })) };
        let r = execute_amplitude(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("secret_key"));
    }

    #[tokio::test]
    async fn amplitude_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "am3".into(), node_type: NodeType::Amplitude,
            config: Some(serde_json::json!({ "api_key": "key", "secret_key": "sec", "operation": "bogus" })) };
        let r = execute_amplitude(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }
}

// ── Slice 290: Mixpanel ────────────────────────────────────────────────────────

async fn execute_mixpanel(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("track").to_string();

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{api_secret}:").as_bytes());

    let (url, body) = match operation.as_str() {
        "track" => {
            let events = cfg.get("events").cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!({ "project_token": project_token, "events": events });
            ("https://api.mixpanel.com/track#live-event-import".to_string(), b)
        }
        "import" => {
            let events = cfg.get("events").cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let b = serde_json::json!(events);
            (format!("https://api.mixpanel.com/import?project_token={project_token}"), b)
        }
        "query" => {
            let endpoint = cfg.get("endpoint").and_then(|v| v.as_str()).unwrap_or("/api/query").to_string();
            let params = cfg.get("params").cloned().unwrap_or(serde_json::Value::Null);
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Mixpanel query error: {e}")),
            };
        }
        _ => return NodeExecutionResult::failed(format!("Mixpanel unknown operation '{operation}'")),
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
                serde_json::json!({ "status": status, "body": resp_body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Mixpanel request error: {e}")),
    }
}

// ── Slice 291: Segment ─────────────────────────────────────────────────────────

async fn execute_segment(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let write_key = match cfg.get("write_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Segment requires 'write_key'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("track").to_string();

    use base64::Engine as _;
    // Segment Basic auth: base64(write_key:)
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{write_key}:").as_bytes());

    let endpoint = match operation.as_str() {
        "track"    => "/v1/track",
        "identify" => "/v1/identify",
        "page"     => "/v1/page",
        "group"    => "/v1/group",
        "alias"    => "/v1/alias",
        "batch"    => "/v1/batch",
        _ => return NodeExecutionResult::failed(format!("Segment unknown operation '{operation}'")),
    };

    let body = cfg.get("body").cloned()
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
                serde_json::json!({ "status": status, "body": resp_body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Segment request error: {e}")),
    }
}

// ── Slice 292: SendGrid ────────────────────────────────────────────────────────

async fn execute_sendgrid(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("POST").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("SendGrid request error: {e}")),
    }
}

// ── Slice 293: Braintree ───────────────────────────────────────────────────────

async fn execute_braintree(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let environment = cfg.get("environment").and_then(|v| v.as_str()).unwrap_or("sandbox").to_string();
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'endpoint'"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();

    use base64::Engine as _;
    let credentials = format!("{public_key}:{private_key}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());

    let base_url = if environment == "production" {
        format!("https://api.braintreegateway.com/merchants/{merchant_id}")
    } else {
        format!("https://api.sandbox.braintreegateway.com/merchants/{merchant_id}")
    };
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
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
        let n = Node { id: "mp1".into(), node_type: NodeType::Mixpanel,
            config: Some(serde_json::json!({ "api_secret": "sec", "operation": "track" })) };
        let r = execute_mixpanel(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_token"));
    }

    #[tokio::test]
    async fn mixpanel_fails_without_api_secret() {
        let c = reqwest::Client::new();
        let n = Node { id: "mp2".into(), node_type: NodeType::Mixpanel,
            config: Some(serde_json::json!({ "project_token": "tok", "operation": "track" })) };
        let r = execute_mixpanel(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_secret"));
    }

    #[tokio::test]
    async fn mixpanel_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "mp3".into(), node_type: NodeType::Mixpanel,
            config: Some(serde_json::json!({ "project_token": "tok", "api_secret": "sec", "operation": "bogus" })) };
        let r = execute_mixpanel(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }

    // ── Segment ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn segment_fails_without_write_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "sg1".into(), node_type: NodeType::Segment,
            config: Some(serde_json::json!({ "operation": "track" })) };
        let r = execute_segment(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("write_key"));
    }

    #[tokio::test]
    async fn segment_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "sg2".into(), node_type: NodeType::Segment,
            config: Some(serde_json::json!({ "write_key": "wk_abc", "operation": "bogus" })) };
        let r = execute_segment(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }

    // ── SendGrid ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn sendgrid_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "sg3".into(), node_type: NodeType::Sendgrid,
            config: Some(serde_json::json!({ "endpoint": "/mail/send" })) };
        let r = execute_sendgrid(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn sendgrid_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "sg4".into(), node_type: NodeType::Sendgrid,
            config: Some(serde_json::json!({ "api_key": "SG.abc" })) };
        let r = execute_sendgrid(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Braintree ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn braintree_fails_without_merchant_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "bt1".into(), node_type: NodeType::Braintree,
            config: Some(serde_json::json!({ "public_key": "pk", "private_key": "prk", "endpoint": "/transactions" })) };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("merchant_id"));
    }

    #[tokio::test]
    async fn braintree_fails_without_public_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "bt2".into(), node_type: NodeType::Braintree,
            config: Some(serde_json::json!({ "merchant_id": "mid", "private_key": "prk", "endpoint": "/transactions" })) };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("public_key"));
    }

    #[tokio::test]
    async fn braintree_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "bt3".into(), node_type: NodeType::Braintree,
            config: Some(serde_json::json!({ "merchant_id": "mid", "public_key": "pk", "private_key": "prk" })) };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 294: PayPal ──────────────────────────────────────────────────────────

async fn execute_paypal(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
            let environment = cfg.get("environment").and_then(|v| v.as_str()).unwrap_or("sandbox");
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
                Err(e) => return NodeExecutionResult::failed(format!("PayPal token exchange error: {e}")),
            }
        }
    };

    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("PayPal requires 'endpoint'"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let environment = cfg.get("environment").and_then(|v| v.as_str()).unwrap_or("sandbox");
    let base = if environment == "live" { "https://api-m.paypal.com" } else { "https://api-m.sandbox.paypal.com" };
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("PayPal request error: {e}")),
    }
}

// ── Slice 295: Razorpay ────────────────────────────────────────────────────────

async fn execute_razorpay(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Razorpay request error: {e}")),
    }
}

// ── Slice 296: Firebase ────────────────────────────────────────────────────────

async fn execute_firebase(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let service = cfg.get("service").and_then(|v| v.as_str()).unwrap_or("firestore").to_string();
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Firebase requires 'endpoint'"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Firebase request error: {e}")),
    }
}

// ── Slice 297: Supabase ────────────────────────────────────────────────────────

async fn execute_supabase(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
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
        let n = Node { id: "pp1".into(), node_type: NodeType::Paypal,
            config: Some(serde_json::json!({ "client_secret": "sec", "endpoint": "/v2/checkout/orders" })) };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_id"));
    }

    #[tokio::test]
    async fn paypal_fails_without_client_secret() {
        let c = reqwest::Client::new();
        let n = Node { id: "pp2".into(), node_type: NodeType::Paypal,
            config: Some(serde_json::json!({ "client_id": "cid", "endpoint": "/v2/checkout/orders" })) };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_secret"));
    }

    #[tokio::test]
    async fn paypal_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "pp3".into(), node_type: NodeType::Paypal,
            config: Some(serde_json::json!({ "client_id": "cid", "client_secret": "sec", "access_token": "tok" })) };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Razorpay ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn razorpay_fails_without_key_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "rp1".into(), node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_secret": "sec", "endpoint": "/orders" })) };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key_id"));
    }

    #[tokio::test]
    async fn razorpay_fails_without_key_secret() {
        let c = reqwest::Client::new();
        let n = Node { id: "rp2".into(), node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_id": "rzp_test_abc", "endpoint": "/orders" })) };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key_secret"));
    }

    #[tokio::test]
    async fn razorpay_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "rp3".into(), node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_id": "rzp_test_abc", "key_secret": "sec" })) };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Firebase ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn firebase_fails_without_project_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "fb1".into(), node_type: NodeType::Firebase,
            config: Some(serde_json::json!({ "id_token": "tok", "endpoint": "/users" })) };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_id"));
    }

    #[tokio::test]
    async fn firebase_fails_without_id_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "fb2".into(), node_type: NodeType::Firebase,
            config: Some(serde_json::json!({ "project_id": "my-proj", "endpoint": "/users" })) };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("id_token"));
    }

    #[tokio::test]
    async fn firebase_unknown_service() {
        let c = reqwest::Client::new();
        let n = Node { id: "fb3".into(), node_type: NodeType::Firebase,
            config: Some(serde_json::json!({ "project_id": "proj", "id_token": "tok", "endpoint": "/doc", "service": "bogus" })) };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }

    // ── Supabase ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn supabase_fails_without_project_url() {
        let c = reqwest::Client::new();
        let n = Node { id: "sb1".into(), node_type: NodeType::Supabase,
            config: Some(serde_json::json!({ "api_key": "eyJ…", "endpoint": "/rest/v1/users" })) };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_url"));
    }

    #[tokio::test]
    async fn supabase_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "sb2".into(), node_type: NodeType::Supabase,
            config: Some(serde_json::json!({ "project_url": "https://xyz.supabase.co", "endpoint": "/rest/v1/users" })) };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn supabase_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "sb3".into(), node_type: NodeType::Supabase,
            config: Some(serde_json::json!({ "project_url": "https://xyz.supabase.co", "api_key": "eyJ…" })) };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 298: Mailchimp ───────────────────────────────────────────────────────

async fn execute_mailchimp(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Mailchimp requires 'api_key'"),
    };
    let server = match cfg.get("server").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        // Try to extract server prefix from api_key (format: key-us1)
        _ => {
            match api_key.split('-').last() {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return NodeExecutionResult::failed("Mailchimp requires 'server' (e.g. us1)"),
            }
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Mailchimp requires 'endpoint'"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
    let url = format!("https://{server}.api.mailchimp.com/3.0{ep}");

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("anystring:{api_key}").as_bytes());

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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Mailchimp request error: {e}")),
    }
}

// ── Slice 299: ActiveCampaign ──────────────────────────────────────────────────

async fn execute_activecampaign(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("ActiveCampaign requires 'api_key'"),
    };
    let base_url = match cfg.get("base_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("ActiveCampaign requires 'base_url' (e.g. https://ACCOUNT.api-us1.com)"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("ActiveCampaign requires 'endpoint'"),
    };
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("ActiveCampaign request error: {e}")),
    }
}

// ── Slice 300: Klaviyo ─────────────────────────────────────────────────────────

async fn execute_klaviyo(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Klaviyo request error: {e}")),
    }
}

// ── Slice 301: Resend ──────────────────────────────────────────────────────────

async fn execute_resend(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("POST").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
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
        let n = Node { id: "mc1".into(), node_type: NodeType::Mailchimp,
            config: Some(serde_json::json!({ "server": "us1", "endpoint": "/lists" })) };
        let r = execute_mailchimp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn mailchimp_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "mc2".into(), node_type: NodeType::Mailchimp,
            config: Some(serde_json::json!({ "api_key": "abc-us1", "server": "us1" })) };
        let r = execute_mailchimp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[tokio::test]
    async fn mailchimp_extracts_server_from_key() {
        // api_key format key-us1 → server "us1" extracted automatically
        let c = reqwest::Client::new();
        let n = Node { id: "mc3".into(), node_type: NodeType::Mailchimp,
            config: Some(serde_json::json!({ "api_key": "abc123-us7", "endpoint": "/lists" })) };
        // No server provided — should not fail with "server" error (will fail at network)
        let r = execute_mailchimp(&n, &ctx(), &c).await;
        assert!(!r.error.as_deref().unwrap_or("x").contains("server"));
    }

    // ── ActiveCampaign ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn activecampaign_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "ac1".into(), node_type: NodeType::Activecampaign,
            config: Some(serde_json::json!({ "base_url": "https://acct.api-us1.com", "endpoint": "/contacts" })) };
        let r = execute_activecampaign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn activecampaign_fails_without_base_url() {
        let c = reqwest::Client::new();
        let n = Node { id: "ac2".into(), node_type: NodeType::Activecampaign,
            config: Some(serde_json::json!({ "api_key": "abc", "endpoint": "/contacts" })) };
        let r = execute_activecampaign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("base_url"));
    }

    #[tokio::test]
    async fn activecampaign_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "ac3".into(), node_type: NodeType::Activecampaign,
            config: Some(serde_json::json!({ "api_key": "abc", "base_url": "https://acct.api-us1.com" })) };
        let r = execute_activecampaign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Klaviyo ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn klaviyo_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "kv1".into(), node_type: NodeType::Klaviyo,
            config: Some(serde_json::json!({ "endpoint": "/profiles" })) };
        let r = execute_klaviyo(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn klaviyo_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "kv2".into(), node_type: NodeType::Klaviyo,
            config: Some(serde_json::json!({ "api_key": "pk_abc" })) };
        let r = execute_klaviyo(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Resend ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn resend_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "rs1".into(), node_type: NodeType::Resend,
            config: Some(serde_json::json!({ "endpoint": "/emails" })) };
        let r = execute_resend(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn resend_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "rs2".into(), node_type: NodeType::Resend,
            config: Some(serde_json::json!({ "api_key": "re_abc" })) };
        let r = execute_resend(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 302: Contentful ──────────────────────────────────────────────────────

async fn execute_contentful(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    // Management API uses api.contentful.com; Delivery/Preview use cdn.contentful.com
    let api_type = cfg.get("api_type").and_then(|v| v.as_str()).unwrap_or("delivery");
    let base = match api_type {
        "management" => format!("https://api.contentful.com/spaces/{space_id}"),
        "preview"    => format!("https://preview.contentful.com/spaces/{space_id}"),
        _            => format!("https://cdn.contentful.com/spaces/{space_id}"),
    };
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Contentful request error: {e}")),
    }
}

// ── Slice 303: Algolia ─────────────────────────────────────────────────────────

async fn execute_algolia(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Algolia request error: {e}")),
    }
}

// ── Slice 304: Postmark ────────────────────────────────────────────────────────

async fn execute_postmark(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("POST").to_uppercase();
    let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                serde_json::json!({ "status": status, "body": body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Postmark request error: {e}")),
    }
}

// ── Slice 305: Vonage ──────────────────────────────────────────────────────────

async fn execute_vonage(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("sms").to_string();

    match operation.as_str() {
        "sms" => {
            let to   = cfg.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let from = cfg.get("from").and_then(|v| v.as_str()).unwrap_or("Vonage").to_string();
            let text = cfg.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
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
                    let resp_body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": resp_body }).to_string()
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
            let ep = if endpoint.starts_with('/') { endpoint.clone() } else { format!("/{endpoint}") };
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
                    let resp_body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": resp_body }).to_string()
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
        let n = Node { id: "cf1".into(), node_type: NodeType::Contentful,
            config: Some(serde_json::json!({ "space_id": "sp1", "endpoint": "/entries" })) };
        let r = execute_contentful(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn contentful_fails_without_space_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "cf2".into(), node_type: NodeType::Contentful,
            config: Some(serde_json::json!({ "access_token": "tok", "endpoint": "/entries" })) };
        let r = execute_contentful(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("space_id"));
    }

    #[tokio::test]
    async fn contentful_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "cf3".into(), node_type: NodeType::Contentful,
            config: Some(serde_json::json!({ "access_token": "tok", "space_id": "sp1" })) };
        let r = execute_contentful(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Algolia ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn algolia_fails_without_app_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "al1".into(), node_type: NodeType::Algolia,
            config: Some(serde_json::json!({ "api_key": "key", "endpoint": "/1/indexes/myindex/query" })) };
        let r = execute_algolia(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("app_id"));
    }

    #[tokio::test]
    async fn algolia_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "al2".into(), node_type: NodeType::Algolia,
            config: Some(serde_json::json!({ "app_id": "ABC123", "endpoint": "/1/indexes/myindex/query" })) };
        let r = execute_algolia(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn algolia_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "al3".into(), node_type: NodeType::Algolia,
            config: Some(serde_json::json!({ "app_id": "ABC123", "api_key": "key" })) };
        let r = execute_algolia(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Postmark ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn postmark_fails_without_server_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "pm1".into(), node_type: NodeType::Postmark,
            config: Some(serde_json::json!({ "endpoint": "/email" })) };
        let r = execute_postmark(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("server_token"));
    }

    #[tokio::test]
    async fn postmark_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "pm2".into(), node_type: NodeType::Postmark,
            config: Some(serde_json::json!({ "server_token": "tok-abc" })) };
        let r = execute_postmark(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Vonage ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn vonage_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "vn1".into(), node_type: NodeType::Vonage,
            config: Some(serde_json::json!({ "api_secret": "sec", "operation": "sms", "to": "1234", "text": "hi" })) };
        let r = execute_vonage(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn vonage_fails_without_api_secret() {
        let c = reqwest::Client::new();
        let n = Node { id: "vn2".into(), node_type: NodeType::Vonage,
            config: Some(serde_json::json!({ "api_key": "key", "operation": "sms" })) };
        let r = execute_vonage(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_secret"));
    }

    #[tokio::test]
    async fn vonage_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "vn3".into(), node_type: NodeType::Vonage,
            config: Some(serde_json::json!({ "api_key": "key", "api_secret": "sec", "operation": "bogus" })) };
        let r = execute_vonage(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }
}
// ── Slice 308: Telegram ────────────────────────────────────────────────────────
async fn execute_telegram(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let bot_token = match cfg.get("bot_token").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        _ => return NodeExecutionResult::failed("Telegram requires 'bot_token'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("sendMessage").to_string();
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
                for (k, v) in obj { map.insert(k.clone(), v.clone()); }
            }
        }
    }
    match http_client.post(&base).json(&payload).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string()
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
        let n = Node { id: "s1".into(), node_type: NodeType::Shopify,
            config: Some(serde_json::json!({"token":"shpat_test"})) };
        let r = execute_shopify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("shop"));
    }

    #[tokio::test]
    async fn shopify_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "s2".into(), node_type: NodeType::Shopify,
            config: Some(serde_json::json!({"shop":"test.myshopify.com"})) };
        let r = execute_shopify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    // ── Discord ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn discord_fails_without_webhook_url() {
        let c = reqwest::Client::new();
        let n = Node { id: "d1".into(), node_type: NodeType::Discord,
            config: Some(serde_json::json!({"content":"hello"})) };
        let r = execute_discord(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("webhook_url"));
    }

    #[tokio::test]
    async fn discord_fails_without_content() {
        let c = reqwest::Client::new();
        let n = Node { id: "d2".into(), node_type: NodeType::Discord,
            config: Some(serde_json::json!({"webhook_url":"https://discord.com/api/webhooks/test"})) };
        let r = execute_discord(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("content"));
    }

    #[tokio::test]
    async fn discord_fails_without_config() {
        let c = reqwest::Client::new();
        let n = Node { id: "d3".into(), node_type: NodeType::Discord, config: None };
        let r = execute_discord(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }

    // ── Telegram ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn telegram_fails_without_bot_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "t1".into(), node_type: NodeType::Telegram,
            config: Some(serde_json::json!({"chat_id":"123","text":"hello"})) };
        let r = execute_telegram(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bot_token"));
    }

    #[tokio::test]
    async fn telegram_with_token_attempts_request() {
        let c = reqwest::Client::new();
        let n = Node { id: "t2".into(), node_type: NodeType::Telegram,
            config: Some(serde_json::json!({"bot_token":"123:test","chat_id":"456","text":"hello"})) };
        let r = execute_telegram(&n, &ctx(), &c).await;
        // Network will fail but config validation passes
        assert!(r.error.as_deref().unwrap_or("").contains("Telegram error") || r.output_json.is_some());
    }

    // ── Notion ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn notion_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "n1".into(), node_type: NodeType::Notion,
            config: Some(serde_json::json!({"endpoint":"/v1/databases"})) };
        let r = execute_notion(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn notion_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node { id: "n2".into(), node_type: NodeType::Notion,
            config: Some(serde_json::json!({"token":"secret_test"})) };
        let r = execute_notion(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 310: Replicate ───────────────────────────────────────────────────────
async fn execute_replicate(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Replicate requires 'api_token'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("run").to_string();
    let auth = format!("Token {api_token}");

    match operation.as_str() {
        "run" | "create_prediction" => {
            let version = match cfg.get("version").and_then(|v| v.as_str()) {
                Some(v) if !v.is_empty() => v.to_string(),
                _ => return NodeExecutionResult::failed("Replicate run requires 'version' (model version ID)"),
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Replicate error: {e}")),
            }
        }
        "get_prediction" => {
            let prediction_id = match cfg.get("prediction_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Replicate get_prediction requires 'prediction_id'"),
            };
            let url = format!("https://api.replicate.com/v1/predictions/{prediction_id}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Replicate list error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Replicate unknown operation '{other}'")),
    }
}

// ── Slice 311: Mistral ─────────────────────────────────────────────────────────
async fn execute_mistral(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Mistral requires 'api_key'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("chat").to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("mistral-small-latest").to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed("Mistral chat requires 'messages' or 'prompt'");
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") { body["temperature"] = temp.clone(); }
            if let Some(max_tokens) = cfg.get("max_tokens") { body["max_tokens"] = max_tokens.clone(); }
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Mistral error: {e}")),
            }
        }
        "embeddings" => {
            let model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("mistral-embed").to_string();
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Mistral list models error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Mistral unknown operation '{other}'")),
    }
}

// ── Slice 312: WhatsApp Business ───────────────────────────────────────────────
async fn execute_whatsapp(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let message_type = cfg.get("message_type").and_then(|v| v.as_str()).unwrap_or("text").to_string();
    let api_version = cfg.get("api_version").and_then(|v| v.as_str()).unwrap_or("v18.0");
    let url = format!("https://graph.facebook.com/{api_version}/{phone_number_id}/messages");

    let body = match message_type.as_str() {
        "text" => {
            let text = cfg.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
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
                None => return NodeExecutionResult::failed("WhatsApp template requires 'template_name'"),
            };
            let language_code = cfg.get("language_code").and_then(|v| v.as_str()).unwrap_or("en_US");
            let components = cfg.get("components").cloned().unwrap_or(serde_json::json!([]));
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
        _ => return NodeExecutionResult::failed(format!("WhatsApp unknown message_type '{message_type}'")),
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
                serde_json::json!({ "status": status, "body": resp_body }).to_string()
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("WhatsApp error: {e}")),
    }
}

// ── Slice 313: Google Docs ─────────────────────────────────────────────────────
async fn execute_googledocs(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Docs requires 'access_token'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("get").to_string();
    let auth = format!("Bearer {access_token}");

    match operation.as_str() {
        "get" => {
            let document_id = match cfg.get("document_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Docs get requires 'document_id'"),
            };
            let url = format!("https://docs.googleapis.com/v1/documents/{document_id}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Docs get error: {e}")),
            }
        }
        "create" => {
            let title = cfg.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled Document").to_string();
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Docs create error: {e}")),
            }
        }
        "batch_update" => {
            let document_id = match cfg.get("document_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Docs batch_update requires 'document_id'"),
            };
            let requests = match cfg.get("requests") {
                Some(r) => r.clone(),
                None => return NodeExecutionResult::failed("Google Docs batch_update requires 'requests'"),
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string()
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Docs batch_update error: {e}")),
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
        let n = Node { id: "r1".into(), node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"version":"abc123","input":{}})) };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn replicate_run_fails_without_version() {
        let c = reqwest::Client::new();
        let n = Node { id: "r2".into(), node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"run"})) };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("version"));
    }

    #[tokio::test]
    async fn replicate_get_prediction_fails_without_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "r3".into(), node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"get_prediction"})) };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prediction_id"));
    }

    #[tokio::test]
    async fn replicate_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "r4".into(), node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"invalid"})) };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── Mistral ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn mistral_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "m1".into(), node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"prompt":"hello"})) };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn mistral_chat_fails_without_messages_or_prompt() {
        let c = reqwest::Client::new();
        let n = Node { id: "m2".into(), node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})) };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("messages") || r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn mistral_embeddings_fails_without_input() {
        let c = reqwest::Client::new();
        let n = Node { id: "m3".into(), node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"embeddings"})) };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("input"));
    }

    #[tokio::test]
    async fn mistral_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "m4".into(), node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad_op"})) };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── WhatsApp ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn whatsapp_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "w1".into(), node_type: NodeType::Whatsapp,
            config: Some(serde_json::json!({"phone_number_id":"123","to":"+1234567890"})) };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn whatsapp_fails_without_phone_number_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "w2".into(), node_type: NodeType::Whatsapp,
            config: Some(serde_json::json!({"access_token":"test","to":"+1234567890"})) };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("phone_number_id"));
    }

    #[tokio::test]
    async fn whatsapp_fails_without_to() {
        let c = reqwest::Client::new();
        let n = Node { id: "w3".into(), node_type: NodeType::Whatsapp,
            config: Some(serde_json::json!({"access_token":"test","phone_number_id":"123"})) };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("'to'"));
    }

    #[tokio::test]
    async fn whatsapp_template_fails_without_template_name() {
        let c = reqwest::Client::new();
        let n = Node { id: "w4".into(), node_type: NodeType::Whatsapp,
            config: Some(serde_json::json!({"access_token":"t","phone_number_id":"123","to":"+1","message_type":"template"})) };
        let r = execute_whatsapp(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("template_name"));
    }

    // ── Google Docs ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn googledocs_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "g1".into(), node_type: NodeType::Googledocs,
            config: Some(serde_json::json!({"document_id":"doc123"})) };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn googledocs_get_fails_without_document_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "g2".into(), node_type: NodeType::Googledocs,
            config: Some(serde_json::json!({"access_token":"test","operation":"get"})) };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("document_id"));
    }

    #[tokio::test]
    async fn googledocs_batch_update_fails_without_requests() {
        let c = reqwest::Client::new();
        let n = Node { id: "g3".into(), node_type: NodeType::Googledocs,
            config: Some(serde_json::json!({"access_token":"test","operation":"batch_update","document_id":"doc123"})) };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("requests"));
    }

    #[tokio::test]
    async fn googledocs_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "g4".into(), node_type: NodeType::Googledocs,
            config: Some(serde_json::json!({"access_token":"test","operation":"invalid"})) };
        let r = execute_googledocs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }
}

// ── Slice 314: Perplexity ──────────────────────────────────────────────────────
async fn execute_perplexity(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Perplexity requires 'api_key'"),
    };
    let model = cfg.get("model").and_then(|v| v.as_str())
        .unwrap_or("llama-3.1-sonar-small-128k-online").to_string();
    let messages = if let Some(msgs) = cfg.get("messages") {
        msgs.clone()
    } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
        serde_json::json!([{"role": "user", "content": prompt}])
    } else {
        return NodeExecutionResult::failed("Perplexity requires 'messages' or 'prompt'");
    };
    let mut body = serde_json::json!({ "model": model, "messages": messages });
    if let Some(temp) = cfg.get("temperature") { body["temperature"] = temp.clone(); }
    if let Some(max_tokens) = cfg.get("max_tokens") { body["max_tokens"] = max_tokens.clone(); }
    if let Some(search_domain_filter) = cfg.get("search_domain_filter") { body["search_domain_filter"] = search_domain_filter.clone(); }
    if let Some(return_citations) = cfg.get("return_citations") { body["return_citations"] = return_citations.clone(); }
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
            NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
        }
        Err(e) => NodeExecutionResult::failed(format!("Perplexity error: {e}")),
    }
}

// ── Slice 315: Cohere ──────────────────────────────────────────────────────────
async fn execute_cohere(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Cohere requires 'api_key'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("chat").to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let message = match cfg.get("message").and_then(|v| v.as_str()) {
                Some(m) if !m.is_empty() => m.to_string(),
                _ => return NodeExecutionResult::failed("Cohere chat requires 'message'"),
            };
            let model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("command-r-plus").to_string();
            let mut body = serde_json::json!({ "message": message, "model": model });
            if let Some(temperature) = cfg.get("temperature") { body["temperature"] = temperature.clone(); }
            if let Some(chat_history) = cfg.get("chat_history") { body["chat_history"] = chat_history.clone(); }
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Cohere chat error: {e}")),
            }
        }
        "embed" => {
            let texts = match cfg.get("texts") {
                Some(t) => t.clone(),
                None => return NodeExecutionResult::failed("Cohere embed requires 'texts' (array of strings)"),
            };
            let model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("embed-english-v3.0").to_string();
            let input_type = cfg.get("input_type").and_then(|v| v.as_str()).unwrap_or("search_document").to_string();
            let body = serde_json::json!({ "texts": texts, "model": model, "input_type": input_type });
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
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
            let model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("rerank-english-v3.0").to_string();
            let body = serde_json::json!({ "query": query, "documents": documents, "model": model });
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Cohere rerank error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Cohere unknown operation '{other}'")),
    }
}

// ── Slice 316: Google Drive ────────────────────────────────────────────────────
async fn execute_googledrive(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Drive requires 'access_token'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list").to_string();
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
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
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
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
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
            match http_client.delete(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Drive delete error: {e}")),
            }
        }
        "create_folder" => {
            let name = cfg.get("name").and_then(|v| v.as_str()).unwrap_or("New Folder").to_string();
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Drive create_folder error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Google Drive unknown operation '{other}'")),
    }
}

fn urlencoding_simple(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        c => format!("%{:02X}", c as u32),
    }).collect()
}

// ── Slice 317: WooCommerce ─────────────────────────────────────────────────────
async fn execute_woocommerce(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let endpoint = cfg.get("endpoint").and_then(|v| v.as_str()).unwrap_or("/wp-json/wc/v3/products").to_string();
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let url = format!("{}{}", site_url, endpoint);

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("{consumer_key}:{consumer_secret}").as_bytes());

    let mut req = match method.as_str() {
        "POST"   => http_client.post(&url),
        "PUT"    => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        "PATCH"  => http_client.patch(&url),
        _        => http_client.get(&url),
    };
    req = req.header("Authorization", format!("Basic {encoded}"))
             .header("Content-Type", "application/json");
    if let Some(body) = cfg.get("body") {
        req = req.json(body);
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
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
        let n = Node { id: "p1".into(), node_type: NodeType::Perplexity,
            config: Some(serde_json::json!({"prompt":"hello"})) };
        let r = execute_perplexity(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn perplexity_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node { id: "p2".into(), node_type: NodeType::Perplexity,
            config: Some(serde_json::json!({"api_key":"test"})) };
        let r = execute_perplexity(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("messages") || r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    // ── Cohere ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cohere_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "c1".into(), node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"message":"hello"})) };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn cohere_chat_fails_without_message() {
        let c = reqwest::Client::new();
        let n = Node { id: "c2".into(), node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})) };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("message"));
    }

    #[tokio::test]
    async fn cohere_embed_fails_without_texts() {
        let c = reqwest::Client::new();
        let n = Node { id: "c3".into(), node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"embed"})) };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("texts"));
    }

    #[tokio::test]
    async fn cohere_rerank_fails_without_query() {
        let c = reqwest::Client::new();
        let n = Node { id: "c4".into(), node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"rerank","documents":["doc1"]})) };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn cohere_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "c5".into(), node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"invalid"})) };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── Google Drive ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn googledrive_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "g1".into(), node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"operation":"list"})) };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn googledrive_get_fails_without_file_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "g2".into(), node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"get"})) };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("file_id"));
    }

    #[tokio::test]
    async fn googledrive_delete_fails_without_file_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "g3".into(), node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"delete"})) };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("file_id"));
    }

    #[tokio::test]
    async fn googledrive_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "g4".into(), node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"bad"})) };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── WooCommerce ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn woocommerce_fails_without_consumer_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "w1".into(), node_type: NodeType::Woocommerce,
            config: Some(serde_json::json!({"consumer_secret":"sec","site_url":"https://shop.example.com"})) };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("consumer_key"));
    }

    #[tokio::test]
    async fn woocommerce_fails_without_consumer_secret() {
        let c = reqwest::Client::new();
        let n = Node { id: "w2".into(), node_type: NodeType::Woocommerce,
            config: Some(serde_json::json!({"consumer_key":"ck_test","site_url":"https://shop.example.com"})) };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("consumer_secret"));
    }

    #[tokio::test]
    async fn woocommerce_fails_without_site_url() {
        let c = reqwest::Client::new();
        let n = Node { id: "w3".into(), node_type: NodeType::Woocommerce,
            config: Some(serde_json::json!({"consumer_key":"ck_test","consumer_secret":"cs_test"})) };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("site_url"));
    }
}

// ── Slice 318: Pinecone ────────────────────────────────────────────────────────
async fn execute_pinecone(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Pinecone requires 'api_key'"),
    };
    let index_host = match cfg.get("index_host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed("Pinecone requires 'index_host' (e.g. https://my-index-abc.svc.pinecone.io)"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("query").to_string();

    match operation.as_str() {
        "query" => {
            let vector = match cfg.get("vector") {
                Some(v) => v.clone(),
                None => return NodeExecutionResult::failed("Pinecone query requires 'vector' (float array)"),
            };
            let top_k = cfg.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10);
            let mut body = serde_json::json!({ "vector": vector, "topK": top_k });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) { body["namespace"] = serde_json::json!(ns); }
            if let Some(filter) = cfg.get("filter") { body["filter"] = filter.clone(); }
            if let Some(imd) = cfg.get("include_metadata").and_then(|v| v.as_bool()) { body["includeMetadata"] = serde_json::json!(imd); }
            let url = format!("{}/query", index_host);
            match http_client.post(&url).header("Api-Key", &api_key).header("Content-Type", "application/json").json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Pinecone query error: {e}")),
            }
        }
        "upsert" => {
            let vectors = match cfg.get("vectors") {
                Some(v) => v.clone(),
                None => return NodeExecutionResult::failed("Pinecone upsert requires 'vectors' (array of {id, values, metadata})"),
            };
            let mut body = serde_json::json!({ "vectors": vectors });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) { body["namespace"] = serde_json::json!(ns); }
            let url = format!("{}/vectors/upsert", index_host);
            match http_client.post(&url).header("Api-Key", &api_key).header("Content-Type", "application/json").json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Pinecone upsert error: {e}")),
            }
        }
        "delete" => {
            let ids = match cfg.get("ids") {
                Some(v) => v.clone(),
                None => return NodeExecutionResult::failed("Pinecone delete requires 'ids' (array of vector IDs)"),
            };
            let mut body = serde_json::json!({ "ids": ids });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) { body["namespace"] = serde_json::json!(ns); }
            let url = format!("{}/vectors/delete", index_host);
            match http_client.post(&url).header("Api-Key", &api_key).header("Content-Type", "application/json").json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Pinecone delete error: {e}")),
            }
        }
        "fetch" => {
            let ids = match cfg.get("ids").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(","),
                None => return NodeExecutionResult::failed("Pinecone fetch requires 'ids' (array)"),
            };
            let mut url = format!("{}/vectors/fetch?ids={}", index_host, ids);
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) { url.push_str(&format!("&namespace={ns}")); }
            match http_client.get(&url).header("Api-Key", &api_key).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Pinecone fetch error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Pinecone unknown operation '{other}'")),
    }
}

// ── Slice 319: Together AI ─────────────────────────────────────────────────────
async fn execute_togetherai(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Together AI requires 'api_key'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("chat").to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg.get("model").and_then(|v| v.as_str())
                .unwrap_or("meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo").to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed("Together AI chat requires 'messages' or 'prompt'");
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") { body["temperature"] = temp.clone(); }
            if let Some(max_tokens) = cfg.get("max_tokens") { body["max_tokens"] = max_tokens.clone(); }
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Together AI chat error: {e}")),
            }
        }
        "completions" => {
            let model = cfg.get("model").and_then(|v| v.as_str())
                .unwrap_or("mistralai/Mixtral-8x7B-Instruct-v0.1").to_string();
            let prompt = match cfg.get("prompt").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Together AI completions requires 'prompt'"),
            };
            let mut body = serde_json::json!({ "model": model, "prompt": prompt });
            if let Some(temp) = cfg.get("temperature") { body["temperature"] = temp.clone(); }
            if let Some(max_tokens) = cfg.get("max_tokens") { body["max_tokens"] = max_tokens.clone(); }
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Together AI completions error: {e}")),
            }
        }
        "embeddings" => {
            let model = cfg.get("model").and_then(|v| v.as_str())
                .unwrap_or("togethercomputer/m2-bert-80M-8k-retrieval").to_string();
            let input = match cfg.get("input") {
                Some(i) => i.clone(),
                None => return NodeExecutionResult::failed("Together AI embeddings requires 'input'"),
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
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Together AI embeddings error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Together AI unknown operation '{other}'")),
    }
}

// ── Slice 320: AWS S3 ──────────────────────────────────────────────────────────
async fn execute_awss3(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let region = cfg.get("region").and_then(|v| v.as_str()).unwrap_or("us-east-1").to_string();
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list").to_string();

    let host = if region == "us-east-1" {
        format!("{}.s3.amazonaws.com", bucket)
    } else {
        format!("{}.s3.{}.amazonaws.com", bucket, region)
    };
    let base_url = format!("https://{}", host);

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
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
            let prefix = cfg.get("prefix").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let canonical_query = if prefix.is_empty() {
                "list-type=2".to_string()
            } else {
                format!("list-type=2&prefix={}", sigv4_uri_encode(&prefix))
            };
            let url = format!("{}/?{}", base_url, canonical_query);
            let auth = aws_sigv4_s3_auth(&access_key_id, &secret_access_key, &region, &date_str, &datetime_str, "GET", &host, "/", &canonical_query, EMPTY_HASH);
            match http_client.get(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send().await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
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
            let auth = aws_sigv4_s3_auth(&access_key_id, &secret_access_key, &region, &date_str, &datetime_str, "GET", &host, &key_path, "", EMPTY_HASH);
            match http_client.get(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send().await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
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
            let auth = aws_sigv4_s3_auth(&access_key_id, &secret_access_key, &region, &date_str, &datetime_str, "DELETE", &host, &key_path, "", EMPTY_HASH);
            match http_client.delete(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send().await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 delete_object error: {e}")),
            }
        }
        "put_object" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return NodeExecutionResult::failed("S3 put_object requires 'key'"),
            };
            let body_content = cfg.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let content_type = cfg.get("content_type").and_then(|v| v.as_str()).unwrap_or("application/octet-stream").to_string();
            let key_path = format!("/{}", key.trim_start_matches('/'));
            let url = format!("{}{}", base_url, key_path);
            let payload_hash = {
                use sha2::{Sha256, Digest};
                hex::encode(Sha256::digest(body_content.as_bytes()))
            };
            let auth = aws_sigv4_s3_auth(&access_key_id, &secret_access_key, &region, &date_str, &datetime_str, "PUT", &host, &key_path, "", &payload_hash);
            match http_client.put(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", &payload_hash)
                .header("Content-Type", &content_type)
                .body(body_content)
                .send().await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 put_object error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("S3 unknown operation '{other}'")),
    }
}

fn sigv4_uri_encode(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
            (b as char).to_string()
        }
        b => format!("%{:02X}", b),
    }).collect()
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
    use sha2::{Sha256, Digest};
    use hmac::{Hmac, Mac};
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
        let mut mac = HmacSha256::new_from_slice(
            format!("AWS4{}", secret_access_key).as_bytes()
        ).expect("valid key");
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
        let dy = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 };
        if d < dy { break; }
        d -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days = [31u32,if leap {29} else {28},31,30,31,30,31,31,30,31,30,31];
    let mut m = 0u32;
    for &md in &month_days {
        if d < md { break; }
        d -= md;
        m += 1;
    }
    (y, m + 1, d + 1)
}

// ── Slice 321: Hugging Face ────────────────────────────────────────────────────
async fn execute_huggingface(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Hugging Face requires 'api_token'"),
    };
    let model = match cfg.get("model").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => return NodeExecutionResult::failed("Hugging Face requires 'model' (e.g. gpt2 or facebook/bart-large-cnn)"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("inference").to_string();
    let auth = format!("Bearer {api_token}");

    match operation.as_str() {
        "inference" => {
            let inputs = match cfg.get("inputs") {
                Some(i) => i.clone(),
                None => return NodeExecutionResult::failed("Hugging Face inference requires 'inputs'"),
            };
            let mut body = serde_json::json!({ "inputs": inputs });
            if let Some(params) = cfg.get("parameters") { body["parameters"] = params.clone(); }
            if let Some(options) = cfg.get("options") { body["options"] = options.clone(); }
            let url = format!("https://api-inference.huggingface.co/models/{model}");
            match http_client.post(&url).header("Authorization", &auth).header("Content-Type", "application/json").json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Hugging Face inference error: {e}")),
            }
        }
        "model_info" => {
            let url = format!("https://huggingface.co/api/models/{model}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Hugging Face model_info error: {e}")),
            }
        }
        "list_models" => {
            let search = cfg.get("search").and_then(|v| v.as_str()).unwrap_or("");
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);
            let url = format!("https://huggingface.co/api/models?search={search}&limit={limit}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Hugging Face list_models error: {e}")),
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
        let n = Node { id: "p1".into(), node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"index_host":"https://idx.svc.pinecone.io"})) };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn pinecone_fails_without_index_host() {
        let c = reqwest::Client::new();
        let n = Node { id: "p2".into(), node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"api_key":"test"})) };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("index_host"));
    }

    #[tokio::test]
    async fn pinecone_query_fails_without_vector() {
        let c = reqwest::Client::new();
        let n = Node { id: "p3".into(), node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"query"})) };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vector"));
    }

    #[tokio::test]
    async fn pinecone_upsert_fails_without_vectors() {
        let c = reqwest::Client::new();
        let n = Node { id: "p4".into(), node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"upsert"})) };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vectors"));
    }

    #[tokio::test]
    async fn pinecone_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "p5".into(), node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"bad"})) };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── Together AI ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn togetherai_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "t1".into(), node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"prompt":"hello"})) };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn togetherai_chat_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node { id: "t2".into(), node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})) };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("messages") || r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn togetherai_completions_fails_without_prompt() {
        let c = reqwest::Client::new();
        let n = Node { id: "t3".into(), node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"completions"})) };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn togetherai_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "t4".into(), node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad"})) };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── AWS S3 ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn awss3_fails_without_access_key_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "a1".into(), node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"secret_access_key":"sec","bucket":"my-bucket"})) };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_key_id"));
    }

    #[tokio::test]
    async fn awss3_fails_without_bucket() {
        let c = reqwest::Client::new();
        let n = Node { id: "a2".into(), node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"access_key_id":"key","secret_access_key":"sec"})) };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bucket"));
    }

    #[tokio::test]
    async fn awss3_get_object_fails_without_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "a3".into(), node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"access_key_id":"k","secret_access_key":"s","bucket":"b","operation":"get_object"})) };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key"));
    }

    #[tokio::test]
    async fn awss3_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "a4".into(), node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"access_key_id":"k","secret_access_key":"s","bucket":"b","operation":"bad"})) };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
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
        assert!(auth.starts_with(expected_prefix), "bad header prefix: {auth}");
        let sig = auth.split("Signature=").nth(1).unwrap_or("");
        assert_eq!(sig.len(), 64, "signature must be 64 hex chars, got {}", sig.len());
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()), "signature is not hex");
        assert_ne!(sig, "placeholder");
    }

    #[test]
    fn aws_sigv4_s3_auth_is_deterministic() {
        let (aki, sak, reg, ds, dts, meth, host, uri, qry, ph) = (
            "AKID", "SECRET", "us-west-2", "20260101",
            "20260101T120000Z", "PUT", "mybucket.s3.us-west-2.amazonaws.com",
            "/mykey.txt", "", "abc123hash",
        );
        let a1 = aws_sigv4_s3_auth(aki, sak, reg, ds, dts, meth, host, uri, qry, ph);
        let a2 = aws_sigv4_s3_auth(aki, sak, reg, ds, dts, meth, host, uri, qry, ph);
        assert_eq!(a1, a2);
    }

    #[test]
    fn aws_sigv4_s3_auth_differs_by_secret() {
        let (aki, reg, ds, dts, meth, host, uri, qry, ph) = (
            "AKID", "us-east-1", "20260101", "20260101T000000Z",
            "GET", "b.s3.amazonaws.com", "/", "", "emptyhash",
        );
        let a1 = aws_sigv4_s3_auth(aki, "SECRET1", reg, ds, dts, meth, host, uri, qry, ph);
        let a2 = aws_sigv4_s3_auth(aki, "SECRET2", reg, ds, dts, meth, host, uri, qry, ph);
        assert_ne!(a1, a2, "different secrets must produce different signatures");
    }

    // ── Hugging Face ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn huggingface_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "h1".into(), node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"model":"gpt2","inputs":"hello"})) };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn huggingface_fails_without_model() {
        let c = reqwest::Client::new();
        let n = Node { id: "h2".into(), node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"api_token":"hf_test","inputs":"hello"})) };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("model"));
    }

    #[tokio::test]
    async fn huggingface_inference_fails_without_inputs() {
        let c = reqwest::Client::new();
        let n = Node { id: "h3".into(), node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"api_token":"hf_test","model":"gpt2","operation":"inference"})) };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("inputs"));
    }

    #[tokio::test]
    async fn huggingface_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "h4".into(), node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"api_token":"hf_test","model":"gpt2","operation":"bad"})) };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }
}

// ── Slice 322: Groq ────────────────────────────────────────────────────────────
async fn execute_groq(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Groq requires 'api_key'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("chat").to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg.get("model").and_then(|v| v.as_str())
                .unwrap_or("llama-3.3-70b-versatile").to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed("Groq chat requires 'messages' or 'prompt'");
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") { body["temperature"] = temp.clone(); }
            if let Some(max_tokens) = cfg.get("max_tokens") { body["max_tokens"] = max_tokens.clone(); }
            if let Some(stream) = cfg.get("stream") { body["stream"] = stream.clone(); }
            match http_client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send().await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Groq chat error: {e}")),
            }
        }
        "transcription" => {
            let audio_url = match cfg.get("audio_url").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => return NodeExecutionResult::failed("Groq transcription requires 'audio_url'"),
            };
            let model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("whisper-large-v3").to_string();
            let body = serde_json::json!({ "url": audio_url, "model": model });
            match http_client
                .post("https://api.groq.com/openai/v1/audio/transcriptions")
                .header("Authorization", &auth)
                .json(&body)
                .send().await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Groq transcription error: {e}")),
            }
        }
        "list_models" => {
            match http_client
                .get("https://api.groq.com/openai/v1/models")
                .header("Authorization", &auth)
                .send().await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Groq list_models error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Groq unknown operation '{other}'")),
    }
}

// ── Slice 323: OpenRouter ──────────────────────────────────────────────────────
async fn execute_openrouter(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("OpenRouter requires 'api_key'"),
    };
    let model = match cfg.get("model").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => return NodeExecutionResult::failed("OpenRouter requires 'model' (e.g. openai/gpt-4o or anthropic/claude-3-5-sonnet)"),
    };
    let messages = if let Some(msgs) = cfg.get("messages") {
        msgs.clone()
    } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
        serde_json::json!([{"role": "user", "content": prompt}])
    } else {
        return NodeExecutionResult::failed("OpenRouter requires 'messages' or 'prompt'");
    };
    let mut body = serde_json::json!({ "model": model, "messages": messages });
    if let Some(temp) = cfg.get("temperature") { body["temperature"] = temp.clone(); }
    if let Some(max_tokens) = cfg.get("max_tokens") { body["max_tokens"] = max_tokens.clone(); }
    if let Some(site_url) = cfg.get("site_url").and_then(|v| v.as_str()) { body["site_url"] = serde_json::json!(site_url); }
    if let Some(site_name) = cfg.get("site_name").and_then(|v| v.as_str()) { body["site_name"] = serde_json::json!(site_name); }

    let mut req = http_client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json");
    if let Some(site_url) = cfg.get("site_url").and_then(|v| v.as_str()) {
        req = req.header("HTTP-Referer", site_url);
    }
    if let Some(site_name) = cfg.get("site_name").and_then(|v| v.as_str()) {
        req = req.header("X-Title", site_name);
    }
    match req.json(&body).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
        }
        Err(e) => NodeExecutionResult::failed(format!("OpenRouter error: {e}")),
    }
}

// ── Slice 324: Qdrant ──────────────────────────────────────────────────────────
async fn execute_qdrant(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed("Qdrant requires 'host' (e.g. https://xyz.qdrant.io or http://localhost:6333)"),
    };
    let collection = match cfg.get("collection").and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => return NodeExecutionResult::failed("Qdrant requires 'collection'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("search").to_string();
    let api_key = cfg.get("api_key").and_then(|v| v.as_str()).map(|s| s.to_string());

    let mut builder = |method: &str, url: &str| -> reqwest::RequestBuilder {
        let rb = match method {
            "POST" => http_client.post(url),
            "PUT"  => http_client.put(url),
            "DELETE" => http_client.delete(url),
            _ => http_client.get(url),
        };
        if let Some(ref key) = api_key {
            rb.header("api-key", key).header("Content-Type", "application/json")
        } else {
            rb.header("Content-Type", "application/json")
        }
    };

    match operation.as_str() {
        "search" => {
            let vector = match cfg.get("vector") {
                Some(v) => v.clone(),
                None => return NodeExecutionResult::failed("Qdrant search requires 'vector'"),
            };
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);
            let mut body = serde_json::json!({ "vector": vector, "limit": limit });
            if let Some(filter) = cfg.get("filter") { body["filter"] = filter.clone(); }
            if let Some(with_payload) = cfg.get("with_payload") { body["with_payload"] = with_payload.clone(); }
            let url = format!("{}/collections/{}/points/search", host, collection);
            match builder("POST", &url).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant search error: {e}")),
            }
        }
        "upsert" => {
            let points = match cfg.get("points") {
                Some(p) => p.clone(),
                None => return NodeExecutionResult::failed("Qdrant upsert requires 'points' (array of {id, vector, payload})"),
            };
            let body = serde_json::json!({ "points": points });
            let url = format!("{}/collections/{}/points", host, collection);
            match builder("PUT", &url).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant upsert error: {e}")),
            }
        }
        "delete" => {
            let ids = match cfg.get("ids") {
                Some(v) => v.clone(),
                None => return NodeExecutionResult::failed("Qdrant delete requires 'ids'"),
            };
            let body = serde_json::json!({ "points": ids });
            let url = format!("{}/collections/{}/points/delete", host, collection);
            match builder("POST", &url).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant delete error: {e}")),
            }
        }
        "get_collection" => {
            let url = format!("{}/collections/{}", host, collection);
            match builder("GET", &url).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant get_collection error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Qdrant unknown operation '{other}'")),
    }
}

// ── Slice 325: Cloudinary ──────────────────────────────────────────────────────
async fn execute_cloudinary(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let cloud_name = match cfg.get("cloud_name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'cloud_name'"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'api_key'"),
    };
    let api_secret = match cfg.get("api_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'api_secret'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list").to_string();

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{api_key}:{api_secret}").as_bytes());
    let auth = format!("Basic {encoded}");

    match operation.as_str() {
        "list" => {
            let resource_type = cfg.get("resource_type").and_then(|v| v.as_str()).unwrap_or("image");
            let url = format!("https://api.cloudinary.com/v1_1/{cloud_name}/resources/{resource_type}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary list error: {e}")),
            }
        }
        "upload" => {
            let file = match cfg.get("file").and_then(|v| v.as_str()) {
                Some(f) if !f.is_empty() => f.to_string(),
                _ => return NodeExecutionResult::failed("Cloudinary upload requires 'file' (URL or base64 data URI)"),
            };
            let resource_type = cfg.get("resource_type").and_then(|v| v.as_str()).unwrap_or("image");
            let url = format!("https://api.cloudinary.com/v1_1/{cloud_name}/{resource_type}/upload");
            let mut form_data = std::collections::HashMap::new();
            form_data.insert("file", file.clone());
            form_data.insert("api_key", api_key.clone());
            // Timestamp-based signature would be needed for authenticated uploads
            // Using unsigned upload preset if configured
            if let Some(preset) = cfg.get("upload_preset").and_then(|v| v.as_str()) {
                form_data.insert("upload_preset", preset.to_string());
            }
            if let Some(folder) = cfg.get("folder").and_then(|v| v.as_str()) {
                form_data.insert("folder", folder.to_string());
            }
            match http_client.post(&url).form(&form_data).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary upload error: {e}")),
            }
        }
        "destroy" => {
            let public_id = match cfg.get("public_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Cloudinary destroy requires 'public_id'"),
            };
            let resource_type = cfg.get("resource_type").and_then(|v| v.as_str()).unwrap_or("image");
            let url = format!("https://api.cloudinary.com/v1_1/{cloud_name}/{resource_type}/destroy");
            let body = serde_json::json!({ "public_id": public_id });
            match http_client.post(&url).header("Authorization", &auth).header("Content-Type", "application/json").json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary destroy error: {e}")),
            }
        }
        "transform_url" => {
            let public_id = match cfg.get("public_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Cloudinary transform_url requires 'public_id'"),
            };
            let transformation = cfg.get("transformation").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let resource_type = cfg.get("resource_type").and_then(|v| v.as_str()).unwrap_or("image");
            let format = cfg.get("format").and_then(|v| v.as_str()).unwrap_or("jpg");
            let url = if transformation.is_empty() {
                format!("https://res.cloudinary.com/{cloud_name}/{resource_type}/upload/{public_id}.{format}")
            } else {
                format!("https://res.cloudinary.com/{cloud_name}/{resource_type}/upload/{transformation}/{public_id}.{format}")
            };
            NodeExecutionResult::succeeded(serde_json::json!({ "url": url, "public_id": public_id }).to_string())
        }
        other => NodeExecutionResult::failed(format!("Cloudinary unknown operation '{other}'")),
    }
}

#[cfg(test)]
mod tests_322_325 {
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

    // ── Groq ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn groq_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "g1".into(), node_type: NodeType::Groq,
            config: Some(serde_json::json!({"prompt":"hello"})) };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn groq_chat_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node { id: "g2".into(), node_type: NodeType::Groq,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})) };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("messages") || r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn groq_transcription_fails_without_audio_url() {
        let c = reqwest::Client::new();
        let n = Node { id: "g3".into(), node_type: NodeType::Groq,
            config: Some(serde_json::json!({"api_key":"test","operation":"transcription"})) };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("audio_url"));
    }

    #[tokio::test]
    async fn groq_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "g4".into(), node_type: NodeType::Groq,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad"})) };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── OpenRouter ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn openrouter_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "o1".into(), node_type: NodeType::Openrouter,
            config: Some(serde_json::json!({"model":"openai/gpt-4o","prompt":"hello"})) };
        let r = execute_openrouter(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn openrouter_fails_without_model() {
        let c = reqwest::Client::new();
        let n = Node { id: "o2".into(), node_type: NodeType::Openrouter,
            config: Some(serde_json::json!({"api_key":"test","prompt":"hello"})) };
        let r = execute_openrouter(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("model"));
    }

    #[tokio::test]
    async fn openrouter_fails_without_messages_or_prompt() {
        let c = reqwest::Client::new();
        let n = Node { id: "o3".into(), node_type: NodeType::Openrouter,
            config: Some(serde_json::json!({"api_key":"test","model":"openai/gpt-4o"})) };
        let r = execute_openrouter(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("messages") || r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    // ── Qdrant ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn qdrant_fails_without_host() {
        let c = reqwest::Client::new();
        let n = Node { id: "q1".into(), node_type: NodeType::Qdrant,
            config: Some(serde_json::json!({"collection":"test","operation":"search","vector":[0.1]})) };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn qdrant_fails_without_collection() {
        let c = reqwest::Client::new();
        let n = Node { id: "q2".into(), node_type: NodeType::Qdrant,
            config: Some(serde_json::json!({"host":"http://localhost:6333","operation":"search"})) };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("collection"));
    }

    #[tokio::test]
    async fn qdrant_search_fails_without_vector() {
        let c = reqwest::Client::new();
        let n = Node { id: "q3".into(), node_type: NodeType::Qdrant,
            config: Some(serde_json::json!({"host":"http://localhost:6333","collection":"test","operation":"search"})) };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vector"));
    }

    #[tokio::test]
    async fn qdrant_upsert_fails_without_points() {
        let c = reqwest::Client::new();
        let n = Node { id: "q4".into(), node_type: NodeType::Qdrant,
            config: Some(serde_json::json!({"host":"http://localhost:6333","collection":"test","operation":"upsert"})) };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("points"));
    }

    #[tokio::test]
    async fn qdrant_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "q5".into(), node_type: NodeType::Qdrant,
            config: Some(serde_json::json!({"host":"http://localhost:6333","collection":"test","operation":"bad"})) };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── Cloudinary ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cloudinary_fails_without_cloud_name() {
        let c = reqwest::Client::new();
        let n = Node { id: "cl1".into(), node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"api_key":"k","api_secret":"s"})) };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("cloud_name"));
    }

    #[tokio::test]
    async fn cloudinary_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "cl2".into(), node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"cloud_name":"mycloud","api_secret":"s"})) };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn cloudinary_destroy_fails_without_public_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "cl3".into(), node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"cloud_name":"c","api_key":"k","api_secret":"s","operation":"destroy"})) };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("public_id"));
    }

    #[tokio::test]
    async fn cloudinary_transform_url_succeeds_without_network() {
        let c = reqwest::Client::new();
        let n = Node { id: "cl4".into(), node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"cloud_name":"mycloud","api_key":"k","api_secret":"s","operation":"transform_url","public_id":"sample","transformation":"w_300,h_200"})) };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        // transform_url is local — no network needed
        assert!(r.output_json.as_deref().unwrap_or("").contains("res.cloudinary.com"));
    }
}

async fn execute_gcal(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Calendar requires 'access_token'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list_events").to_string();
    let auth = format!("Bearer {access_token}");
    let base = "https://www.googleapis.com/calendar/v3";

    match operation.as_str() {
        "list_calendars" => {
            let url = format!("{base}/users/me/calendarList");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Calendar list_calendars error: {e}")),
            }
        }
        "list_events" => {
            let calendar_id = cfg.get("calendar_id").and_then(|v| v.as_str()).unwrap_or("primary");
            let mut url = format!("{base}/calendars/{calendar_id}/events");
            if let Some(q) = cfg.get("query").and_then(|v| v.as_str()) {
                if !q.is_empty() { url.push_str(&format!("?q={}", urlencoding_simple(q))); }
            }
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Calendar list_events error: {e}")),
            }
        }
        "get_event" => {
            let calendar_id = cfg.get("calendar_id").and_then(|v| v.as_str()).unwrap_or("primary");
            let event_id = match cfg.get("event_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Calendar get_event requires 'event_id'"),
            };
            let url = format!("{base}/calendars/{calendar_id}/events/{event_id}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Calendar get_event error: {e}")),
            }
        }
        "create_event" => {
            let calendar_id = cfg.get("calendar_id").and_then(|v| v.as_str()).unwrap_or("primary");
            let summary = cfg.get("summary").and_then(|v| v.as_str()).unwrap_or("New Event");
            let start_time = match cfg.get("start_time").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return NodeExecutionResult::failed("Google Calendar create_event requires 'start_time'"),
            };
            let end_time = cfg.get("end_time").and_then(|v| v.as_str()).unwrap_or(&start_time).to_string();
            let timezone = cfg.get("timezone").and_then(|v| v.as_str()).unwrap_or("UTC");
            let description = cfg.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let body = serde_json::json!({
                "summary": summary,
                "description": description,
                "start": { "dateTime": start_time, "timeZone": timezone },
                "end":   { "dateTime": end_time,   "timeZone": timezone }
            });
            let url = format!("{base}/calendars/{calendar_id}/events");
            match http_client.post(&url).header("Authorization", &auth).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Calendar create_event error: {e}")),
            }
        }
        "delete_event" => {
            let calendar_id = cfg.get("calendar_id").and_then(|v| v.as_str()).unwrap_or("primary");
            let event_id = match cfg.get("event_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Calendar delete_event requires 'event_id'"),
            };
            let url = format!("{base}/calendars/{calendar_id}/events/{event_id}");
            match http_client.delete(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "deleted": true }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Calendar delete_event error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Google Calendar unknown operation '{other}'")),
    }
}

async fn execute_docusign(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("DocuSign requires 'access_token'"),
    };
    let account_id = match cfg.get("account_id").and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return NodeExecutionResult::failed("DocuSign requires 'account_id'"),
    };
    let base_url = cfg.get("base_url").and_then(|v| v.as_str()).unwrap_or("https://demo.docusign.net/restapi");
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list_envelopes").to_string();
    let auth = format!("Bearer {access_token}");

    match operation.as_str() {
        "list_envelopes" => {
            let from_date = cfg.get("from_date").and_then(|v| v.as_str()).unwrap_or("2024-01-01");
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes?from_date={from_date}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("DocuSign list_envelopes error: {e}")),
            }
        }
        "get_envelope" => {
            let envelope_id = match cfg.get("envelope_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("DocuSign get_envelope requires 'envelope_id'"),
            };
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes/{envelope_id}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("DocuSign get_envelope error: {e}")),
            }
        }
        "create_envelope" => {
            let body = cfg.get("body").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes");
            match http_client.post(&url).header("Authorization", &auth).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("DocuSign create_envelope error: {e}")),
            }
        }
        "void_envelope" => {
            let envelope_id = match cfg.get("envelope_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("DocuSign void_envelope requires 'envelope_id'"),
            };
            let reason = cfg.get("void_reason").and_then(|v| v.as_str()).unwrap_or("Voided via workflow");
            let body = serde_json::json!({ "status": "voided", "voidedReason": reason });
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes/{envelope_id}");
            match http_client.put(&url).header("Authorization", &auth).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("DocuSign void_envelope error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("DocuSign unknown operation '{other}'")),
    }
}

async fn execute_xero(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Xero requires 'access_token'"),
    };
    let tenant_id = match cfg.get("tenant_id").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Xero requires 'tenant_id'"),
    };
    let endpoint = cfg.get("endpoint").and_then(|v| v.as_str()).unwrap_or("/Contacts");
    let method = cfg.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let url = format!("https://api.xero.com/api.xro/2.0{endpoint}");
    let auth = format!("Bearer {access_token}");

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PUT"  => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        _ => http_client.get(&url),
    };
    req = req
        .header("Authorization", &auth)
        .header("Xero-Tenant-Id", &tenant_id)
        .header("Accept", "application/json");
    if let Some(body) = cfg.get("body") {
        if !matches!(method.as_str(), "GET" | "DELETE") {
            req = req.json(body);
        }
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
        }
        Err(e) => NodeExecutionResult::failed(format!("Xero error: {e}")),
    }
}

async fn execute_calendly(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Calendly requires 'api_key'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("get_current_user").to_string();
    let auth = format!("Bearer {api_key}");
    let base = "https://api.calendly.com";

    match operation.as_str() {
        "get_current_user" => {
            match http_client.get(&format!("{base}/users/me")).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Calendly get_current_user error: {e}")),
            }
        }
        "list_event_types" => {
            let user_uri = match cfg.get("user_uri").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => return NodeExecutionResult::failed("Calendly list_event_types requires 'user_uri'"),
            };
            let url = format!("{base}/event_types?user={}", urlencoding_simple(&user_uri));
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Calendly list_event_types error: {e}")),
            }
        }
        "list_scheduled_events" => {
            let user_uri = match cfg.get("user_uri").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => return NodeExecutionResult::failed("Calendly list_scheduled_events requires 'user_uri'"),
            };
            let status_filter = cfg.get("status").and_then(|v| v.as_str()).unwrap_or("active");
            let url = format!("{base}/scheduled_events?user={}&status={}", urlencoding_simple(&user_uri), status_filter);
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Calendly list_scheduled_events error: {e}")),
            }
        }
        "get_scheduled_event" => {
            let event_uuid = match cfg.get("event_uuid").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => return NodeExecutionResult::failed("Calendly get_scheduled_event requires 'event_uuid'"),
            };
            let url = format!("{base}/scheduled_events/{event_uuid}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Calendly get_scheduled_event error: {e}")),
            }
        }
        "cancel_event" => {
            let event_uuid = match cfg.get("event_uuid").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => return NodeExecutionResult::failed("Calendly cancel_event requires 'event_uuid'"),
            };
            let reason = cfg.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            let body = serde_json::json!({ "reason": reason });
            let url = format!("{base}/scheduled_events/{event_uuid}/cancellation");
            match http_client.post(&url).header("Authorization", &auth).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Calendly cancel_event error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Calendly unknown operation '{other}'")),
    }
}

#[cfg(test)]
mod tests_326_329 {
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

    // ── Google Calendar ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn gcal_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "g1".into(), node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"operation":"list_events"})) };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn gcal_create_event_fails_without_start_time() {
        let c = reqwest::Client::new();
        let n = Node { id: "g2".into(), node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"access_token":"tok","operation":"create_event","summary":"Meeting"})) };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("start_time"));
    }

    #[tokio::test]
    async fn gcal_delete_event_fails_without_event_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "g3".into(), node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"access_token":"tok","operation":"delete_event"})) };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("event_id"));
    }

    #[tokio::test]
    async fn gcal_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "g4".into(), node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"access_token":"tok","operation":"bad"})) };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── DocuSign ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn docusign_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "d1".into(), node_type: NodeType::Docusign,
            config: Some(serde_json::json!({"account_id":"abc"})) };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn docusign_fails_without_account_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "d2".into(), node_type: NodeType::Docusign,
            config: Some(serde_json::json!({"access_token":"tok"})) };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("account_id"));
    }

    #[tokio::test]
    async fn docusign_get_envelope_fails_without_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "d3".into(), node_type: NodeType::Docusign,
            config: Some(serde_json::json!({"access_token":"tok","account_id":"acc","operation":"get_envelope"})) };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("envelope_id"));
    }

    // ── Xero ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn xero_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "x1".into(), node_type: NodeType::Xero,
            config: Some(serde_json::json!({"tenant_id":"tid"})) };
        let r = execute_xero(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn xero_fails_without_tenant_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "x2".into(), node_type: NodeType::Xero,
            config: Some(serde_json::json!({"access_token":"tok"})) };
        let r = execute_xero(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("tenant_id"));
    }

    // ── Calendly ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn calendly_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "ca1".into(), node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"operation":"get_current_user"})) };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn calendly_list_event_types_fails_without_user_uri() {
        let c = reqwest::Client::new();
        let n = Node { id: "ca2".into(), node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"list_event_types"})) };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("user_uri"));
    }

    #[tokio::test]
    async fn calendly_cancel_event_fails_without_uuid() {
        let c = reqwest::Client::new();
        let n = Node { id: "ca3".into(), node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"cancel_event"})) };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("event_uuid"));
    }

    #[tokio::test]
    async fn calendly_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "ca4".into(), node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"bad"})) };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }
}

async fn execute_apify(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Apify requires 'api_token'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("run_actor").to_string();
    let base = "https://api.apify.com/v2";
    let auth = format!("Bearer {api_token}");

    match operation.as_str() {
        "run_actor" => {
            let actor_id = match cfg.get("actor_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Apify run_actor requires 'actor_id'"),
            };
            let input = cfg.get("input").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let url = format!("{base}/acts/{actor_id}/runs");
            match http_client.post(&url).header("Authorization", &auth).json(&input).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Apify run_actor error: {e}")),
            }
        }
        "get_run" => {
            let run_id = match cfg.get("run_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Apify get_run requires 'run_id'"),
            };
            let url = format!("{base}/actor-runs/{run_id}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Apify get_run error: {e}")),
            }
        }
        "get_dataset_items" => {
            let dataset_id = match cfg.get("dataset_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Apify get_dataset_items requires 'dataset_id'"),
            };
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(100);
            let url = format!("{base}/datasets/{dataset_id}/items?limit={limit}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Apify get_dataset_items error: {e}")),
            }
        }
        "list_actors" => {
            let url = format!("{base}/acts");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Apify list_actors error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Apify unknown operation '{other}'")),
    }
}

async fn execute_ganalytics(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Analytics requires 'access_token'"),
    };
    let property_id = match cfg.get("property_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return NodeExecutionResult::failed("Google Analytics requires 'property_id'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("run_report").to_string();
    let auth = format!("Bearer {access_token}");
    let base = "https://analyticsdata.googleapis.com/v1beta";

    match operation.as_str() {
        "run_report" => {
            let date_ranges = cfg.get("date_ranges").cloned()
                .unwrap_or(serde_json::json!([{"startDate":"7daysAgo","endDate":"today"}]));
            let dimensions = cfg.get("dimensions").cloned()
                .unwrap_or(serde_json::json!([{"name":"date"}]));
            let metrics = cfg.get("metrics").cloned()
                .unwrap_or(serde_json::json!([{"name":"sessions"}]));
            let body = serde_json::json!({
                "dateRanges": date_ranges,
                "dimensions": dimensions,
                "metrics": metrics
            });
            let url = format!("{base}/properties/{property_id}:runReport");
            match http_client.post(&url).header("Authorization", &auth).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Analytics run_report error: {e}")),
            }
        }
        "run_realtime_report" => {
            let dimensions = cfg.get("dimensions").cloned()
                .unwrap_or(serde_json::json!([{"name":"country"}]));
            let metrics = cfg.get("metrics").cloned()
                .unwrap_or(serde_json::json!([{"name":"activeUsers"}]));
            let body = serde_json::json!({ "dimensions": dimensions, "metrics": metrics });
            let url = format!("{base}/properties/{property_id}:runRealtimeReport");
            match http_client.post(&url).header("Authorization", &auth).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Analytics run_realtime_report error: {e}")),
            }
        }
        "get_metadata" => {
            let url = format!("{base}/properties/{property_id}/metadata");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Analytics get_metadata error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Google Analytics unknown operation '{other}'")),
    }
}

async fn execute_neon(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Neon requires 'api_key'"),
    };
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list_projects").to_string();
    let auth = format!("Bearer {api_key}");
    let base = "https://console.neon.tech/api/v2";

    match operation.as_str() {
        "list_projects" => {
            match http_client.get(&format!("{base}/projects")).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Neon list_projects error: {e}")),
            }
        }
        "get_project" => {
            let project_id = match cfg.get("project_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Neon get_project requires 'project_id'"),
            };
            let url = format!("{base}/projects/{project_id}");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Neon get_project error: {e}")),
            }
        }
        "create_project" => {
            let name = cfg.get("name").and_then(|v| v.as_str()).unwrap_or("new-project");
            let body = serde_json::json!({ "project": { "name": name } });
            match http_client.post(&format!("{base}/projects")).header("Authorization", &auth).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Neon create_project error: {e}")),
            }
        }
        "list_branches" => {
            let project_id = match cfg.get("project_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Neon list_branches requires 'project_id'"),
            };
            let url = format!("{base}/projects/{project_id}/branches");
            match http_client.get(&url).header("Authorization", &auth).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
                }
                Err(e) => NodeExecutionResult::failed(format!("Neon list_branches error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Neon unknown operation '{other}'")),
    }
}

async fn execute_copper(node: &Node, context: &ExecutionContext, http_client: &reqwest::Client) -> NodeExecutionResult {
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
    let resource = cfg.get("resource").and_then(|v| v.as_str()).unwrap_or("people");
    let operation = cfg.get("operation").and_then(|v| v.as_str()).unwrap_or("list").to_string();
    let base = "https://api.copper.com/developer_api/v1";

    let mut req_builder;
    match operation.as_str() {
        "list" => {
            let body = cfg.get("filter").cloned().unwrap_or(serde_json::json!({}));
            req_builder = http_client.post(&format!("{base}/{resource}/search")).json(&body);
        }
        "get" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper get requires 'record_id'"),
            };
            req_builder = http_client.get(&format!("{base}/{resource}/{id}"));
        }
        "create" => {
            let body = cfg.get("body").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            req_builder = http_client.post(&format!("{base}/{resource}")).json(&body);
        }
        "update" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper update requires 'record_id'"),
            };
            let body = cfg.get("body").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            req_builder = http_client.put(&format!("{base}/{resource}/{id}")).json(&body);
        }
        "delete" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper delete requires 'record_id'"),
            };
            req_builder = http_client.delete(&format!("{base}/{resource}/{id}"));
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
            NodeExecutionResult::succeeded(serde_json::json!({ "status": status, "body": body }).to_string())
        }
        Err(e) => NodeExecutionResult::failed(format!("Copper error: {e}")),
    }
}

#[cfg(test)]
mod tests_330_333 {
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

    // ── Apify ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn apify_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "a1".into(), node_type: NodeType::Apify,
            config: Some(serde_json::json!({"operation":"list_actors"})) };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn apify_run_actor_fails_without_actor_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "a2".into(), node_type: NodeType::Apify,
            config: Some(serde_json::json!({"api_token":"tok","operation":"run_actor"})) };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("actor_id"));
    }

    #[tokio::test]
    async fn apify_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "a3".into(), node_type: NodeType::Apify,
            config: Some(serde_json::json!({"api_token":"tok","operation":"bad"})) };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── Google Analytics ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn ganalytics_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node { id: "ga1".into(), node_type: NodeType::Ganalytics,
            config: Some(serde_json::json!({"property_id":"123456789"})) };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn ganalytics_fails_without_property_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "ga2".into(), node_type: NodeType::Ganalytics,
            config: Some(serde_json::json!({"access_token":"tok"})) };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("property_id"));
    }

    #[tokio::test]
    async fn ganalytics_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "ga3".into(), node_type: NodeType::Ganalytics,
            config: Some(serde_json::json!({"access_token":"tok","property_id":"123","operation":"bad"})) };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }

    // ── Neon ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn neon_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "n1".into(), node_type: NodeType::Neon,
            config: Some(serde_json::json!({"operation":"list_projects"})) };
        let r = execute_neon(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn neon_get_project_fails_without_project_id() {
        let c = reqwest::Client::new();
        let n = Node { id: "n2".into(), node_type: NodeType::Neon,
            config: Some(serde_json::json!({"api_key":"key","operation":"get_project"})) };
        let r = execute_neon(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_id"));
    }

    // ── Copper ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn copper_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node { id: "co1".into(), node_type: NodeType::Copper,
            config: Some(serde_json::json!({"user_email":"a@b.com"})) };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn copper_fails_without_user_email() {
        let c = reqwest::Client::new();
        let n = Node { id: "co2".into(), node_type: NodeType::Copper,
            config: Some(serde_json::json!({"api_key":"key"})) };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("user_email"));
    }

    #[tokio::test]
    async fn copper_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node { id: "co3".into(), node_type: NodeType::Copper,
            config: Some(serde_json::json!({"api_key":"key","user_email":"a@b.com","operation":"bad"})) };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("unknown operation"));
    }
}
