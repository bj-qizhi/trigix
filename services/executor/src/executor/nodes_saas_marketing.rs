// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! Marketing / email / analytics integration nodes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

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
        _ => match api_key.split('-').next_back() {
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

pub(super) async fn execute_ganalytics(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
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
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("run_report")
        .to_string();
    let auth = format!("Bearer {access_token}");
    let base = "https://analyticsdata.googleapis.com/v1beta";

    match operation.as_str() {
        "run_report" => {
            let date_ranges = cfg
                .get("date_ranges")
                .cloned()
                .unwrap_or(serde_json::json!([{"startDate":"7daysAgo","endDate":"today"}]));
            let dimensions = cfg
                .get("dimensions")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"date"}]));
            let metrics = cfg
                .get("metrics")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"sessions"}]));
            let body = serde_json::json!({
                "dateRanges": date_ranges,
                "dimensions": dimensions,
                "metrics": metrics
            });
            let url = format!("{base}/properties/{property_id}:runReport");
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
                    NodeExecutionResult::failed(format!("Google Analytics run_report error: {e}"))
                }
            }
        }
        "run_realtime_report" => {
            let dimensions = cfg
                .get("dimensions")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"country"}]));
            let metrics = cfg
                .get("metrics")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"activeUsers"}]));
            let body = serde_json::json!({ "dimensions": dimensions, "metrics": metrics });
            let url = format!("{base}/properties/{property_id}:runRealtimeReport");
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
                Err(e) => NodeExecutionResult::failed(format!(
                    "Google Analytics run_realtime_report error: {e}"
                )),
            }
        }
        "get_metadata" => {
            let url = format!("{base}/properties/{property_id}/metadata");
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
                    NodeExecutionResult::failed(format!("Google Analytics get_metadata error: {e}"))
                }
            }
        }
        other => {
            NodeExecutionResult::failed(format!("Google Analytics unknown operation '{other}'"))
        }
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
    async fn ganalytics_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ga1".into(),
            node_type: NodeType::Ganalytics,
            config: Some(serde_json::json!({"property_id":"123456789"})),
        };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn ganalytics_fails_without_property_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ga2".into(),
            node_type: NodeType::Ganalytics,
            config: Some(serde_json::json!({"access_token":"tok"})),
        };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("property_id"));
    }

    #[tokio::test]
    async fn ganalytics_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ga3".into(),
            node_type: NodeType::Ganalytics,
            config: Some(
                serde_json::json!({"access_token":"tok","property_id":"123","operation":"bad"}),
            ),
        };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Neon ──────────────────────────────────────────────────────────────────
}
