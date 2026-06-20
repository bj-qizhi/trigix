// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! Misc SaaS integration nodes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

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

pub(super) async fn execute_gcal(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Calendar requires 'access_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_events")
        .to_string();
    let auth = format!("Bearer {access_token}");
    let base = "https://www.googleapis.com/calendar/v3";

    match operation.as_str() {
        "list_calendars" => {
            let url = format!("{base}/users/me/calendarList");
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
                    "Google Calendar list_calendars error: {e}"
                )),
            }
        }
        "list_events" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let mut url = format!("{base}/calendars/{calendar_id}/events");
            if let Some(q) = cfg.get("query").and_then(|v| v.as_str()) {
                if !q.is_empty() {
                    url.push_str(&format!("?q={}", urlencoding_simple(q)));
                }
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
                Err(e) => {
                    NodeExecutionResult::failed(format!("Google Calendar list_events error: {e}"))
                }
            }
        }
        "get_event" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let event_id = match cfg.get("event_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Google Calendar get_event requires 'event_id'",
                    )
                }
            };
            let url = format!("{base}/calendars/{calendar_id}/events/{event_id}");
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
                    NodeExecutionResult::failed(format!("Google Calendar get_event error: {e}"))
                }
            }
        }
        "create_event" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let summary = cfg
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("New Event");
            let start_time = match cfg.get("start_time").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Google Calendar create_event requires 'start_time'",
                    )
                }
            };
            let end_time = cfg
                .get("end_time")
                .and_then(|v| v.as_str())
                .unwrap_or(&start_time)
                .to_string();
            let timezone = cfg
                .get("timezone")
                .and_then(|v| v.as_str())
                .unwrap_or("UTC");
            let description = cfg
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let body = serde_json::json!({
                "summary": summary,
                "description": description,
                "start": { "dateTime": start_time, "timeZone": timezone },
                "end":   { "dateTime": end_time,   "timeZone": timezone }
            });
            let url = format!("{base}/calendars/{calendar_id}/events");
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
                Err(e) => {
                    NodeExecutionResult::failed(format!("Google Calendar create_event error: {e}"))
                }
            }
        }
        "delete_event" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let event_id = match cfg.get("event_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Google Calendar delete_event requires 'event_id'",
                    )
                }
            };
            let url = format!("{base}/calendars/{calendar_id}/events/{event_id}");
            match http_client
                .delete(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "deleted": true }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Google Calendar delete_event error: {e}"))
                }
            }
        }
        other => {
            NodeExecutionResult::failed(format!("Google Calendar unknown operation '{other}'"))
        }
    }
}

pub(super) async fn execute_apify(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Apify requires 'api_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("run_actor")
        .to_string();
    let base = "https://api.apify.com/v2";
    let auth = format!("Bearer {api_token}");

    match operation.as_str() {
        "run_actor" => {
            let actor_id = match cfg.get("actor_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Apify run_actor requires 'actor_id'"),
            };
            let input = cfg
                .get("input")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let url = format!("{base}/acts/{actor_id}/runs");
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .json(&input)
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
                Err(e) => NodeExecutionResult::failed(format!("Apify run_actor error: {e}")),
            }
        }
        "get_run" => {
            let run_id = match cfg.get("run_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Apify get_run requires 'run_id'"),
            };
            let url = format!("{base}/actor-runs/{run_id}");
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
                Err(e) => NodeExecutionResult::failed(format!("Apify get_run error: {e}")),
            }
        }
        "get_dataset_items" => {
            let dataset_id = match cfg.get("dataset_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Apify get_dataset_items requires 'dataset_id'",
                    )
                }
            };
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(100);
            let url = format!("{base}/datasets/{dataset_id}/items?limit={limit}");
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
                    NodeExecutionResult::failed(format!("Apify get_dataset_items error: {e}"))
                }
            }
        }
        "list_actors" => {
            let url = format!("{base}/acts");
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
                Err(e) => NodeExecutionResult::failed(format!("Apify list_actors error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Apify unknown operation '{other}'")),
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

    #[tokio::test]
    async fn gcal_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g1".into(),
            node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"operation":"list_events"})),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn gcal_create_event_fails_without_start_time() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g2".into(),
            node_type: NodeType::Gcal,
            config: Some(
                serde_json::json!({"access_token":"tok","operation":"create_event","summary":"Meeting"}),
            ),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("start_time"));
    }

    #[tokio::test]
    async fn gcal_delete_event_fails_without_event_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g3".into(),
            node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"access_token":"tok","operation":"delete_event"})),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("event_id"));
    }

    #[tokio::test]
    async fn gcal_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g4".into(),
            node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"access_token":"tok","operation":"bad"})),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── DocuSign ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn apify_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a1".into(),
            node_type: NodeType::Apify,
            config: Some(serde_json::json!({"operation":"list_actors"})),
        };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn apify_run_actor_fails_without_actor_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a2".into(),
            node_type: NodeType::Apify,
            config: Some(serde_json::json!({"api_token":"tok","operation":"run_actor"})),
        };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("actor_id"));
    }

    #[tokio::test]
    async fn apify_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a3".into(),
            node_type: NodeType::Apify,
            config: Some(serde_json::json!({"api_token":"tok","operation":"bad"})),
        };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Google Analytics ──────────────────────────────────────────────────────
}
