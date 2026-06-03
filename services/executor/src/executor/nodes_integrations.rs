// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! SaaS integration nodes (Slack, GitHub, Jira, Notion, Salesforce, …).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

pub(super) async fn execute_slack(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Slack node requires config"),
    };
    let webhook_url = match config.get("webhook_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Slack node missing 'webhook_url'"),
    };
    let text = match config.get("text").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Slack node missing 'text'"),
    };
    let mut payload = serde_json::json!({ "text": text });
    if let Some(u) = config.get("username").and_then(|v| v.as_str()) {
        let r = resolve_template(u, context);
        if !r.is_empty() {
            payload["username"] = serde_json::json!(r);
        }
    }
    if let Some(c) = config.get("channel").and_then(|v| v.as_str()) {
        let r = resolve_template(c, context);
        if !r.is_empty() {
            payload["channel"] = serde_json::json!(r);
        }
    }
    match http_client.post(&webhook_url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => NodeExecutionResult::succeeded(
            serde_json::json!({ "ok": true, "text": text }).to_string(),
        ),
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Slack webhook {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Slack error: {e}")),
    }
}

pub(super) async fn execute_email(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Email node requires config"),
    };
    let to = match config.get("to").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Email node missing 'to'"),
    };
    let subject = match config.get("subject").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Email node missing 'subject'"),
    };
    let body_text = match config.get("body").and_then(|v| v.as_str()) {
        Some(b) => resolve_template(b, context),
        None => return NodeExecutionResult::failed("Email node missing 'body'"),
    };
    // Send via SendGrid API (api_key from config or credential interpolation).
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Email node missing 'api_key'"),
    };
    let from = config
        .get("from")
        .and_then(|v| v.as_str())
        .map(|f| resolve_template(f, context))
        .unwrap_or_else(|| "noreply@trigix.dev".to_string());

    let payload = serde_json::json!({
        "personalizations": [{ "to": [{ "email": to }] }],
        "from": { "email": from },
        "subject": subject,
        "content": [{ "type": "text/plain", "value": body_text }]
    });

    match http_client
        .post("https://api.sendgrid.com/v3/mail/send")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 202 => {
            NodeExecutionResult::succeeded(
                serde_json::json!({ "ok": true, "to": to, "subject": subject }).to_string(),
            )
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Email API {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Email error: {e}")),
    }
}

pub(super) async fn execute_github(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("GitHub node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("GitHub node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("GitHub node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://api.github.com");
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PATCH" => http_client.patch(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        _ => http_client.get(&url),
    }
    .header("Authorization", format!("Bearer {token}"))
    .header("Accept", "application/vnd.github+json")
    .header("X-GitHub-Api-Version", "2022-11-28")
    .header("User-Agent", "trigix/1.0");

    if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        if let Some(body_val) = resolved.get("body") {
            req = req.json(body_val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok || (200..=299).contains(&status) {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("GitHub API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("GitHub request error: {e}")),
    }
}

pub(super) async fn execute_webhook_send(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Webhook node requires config"),
    };
    let url_tmpl = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Webhook node missing 'url'"),
    };

    let mut req = http_client.post(&url_tmpl);

    // Optional headers object
    if let Some(serde_json::Value::Object(headers)) = cfg.get("headers") {
        for (k, v) in headers {
            if let Some(val) = v.as_str() {
                let resolved = resolve_template(val, context);
                req = req.header(k.as_str(), resolved);
            }
        }
    }

    // Body: resolve template or pass through as-is
    let body_val = if let Some(body_tmpl) = cfg.get("body_template").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        resolved.get("body").cloned().unwrap_or_default()
    } else {
        // Default: send the current input as body
        serde_json::from_str(&context.input_json).unwrap_or(serde_json::Value::Null)
    };

    match req.json(&body_val).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "ok": true }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Webhook POST {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Webhook send error: {e}")),
    }
}

pub(super) async fn execute_jira(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Jira node requires config"),
    };
    let base_url = match cfg.get("base_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Jira node missing 'base_url'"),
    };
    let email = match cfg.get("email").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Jira node missing 'email'"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Jira node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Jira node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);

    // Jira uses HTTP Basic auth: base64(email:token)
    use base64::Engine as _;
    let credentials = base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
    let auth_header = format!("Basic {credentials}");

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        "PATCH" => http_client.patch(&url),
        _ => http_client.get(&url),
    }
    .header("Authorization", auth_header)
    .header("Accept", "application/json")
    .header("Content-Type", "application/json")
    .header("User-Agent", "trigix/1.0");

    if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        if let Some(body_val) = resolved.get("body") {
            req = req.json(body_val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Jira API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Jira request error: {e}")),
    }
}

pub(super) async fn execute_notion(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Notion node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Notion node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Notion node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let base_url = "https://api.notion.com";
    let url = format!("{}{}", base_url, endpoint);

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PATCH" => http_client.patch(&url),
        "DELETE" => http_client.delete(&url),
        _ => http_client.get(&url),
    }
    .header("Authorization", format!("Bearer {token}"))
    .header("Notion-Version", "2022-06-28")
    .header("Content-Type", "application/json")
    .header("User-Agent", "trigix/1.0");

    if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        if let Some(body_val) = resolved.get("body") {
            req = req.json(body_val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Notion API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Notion request error: {e}")),
    }
}

pub(super) async fn execute_linear(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Linear node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Linear node missing 'token'"),
    };
    let query = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) => resolve_template(q, context),
        None => return NodeExecutionResult::failed("Linear node missing 'query'"),
    };

    // Build the GraphQL payload
    let mut payload = serde_json::json!({ "query": query });
    if let Some(vars_tmpl) = cfg.get("variables") {
        let resolved = resolve_config_strings(vars_tmpl, context);
        payload["variables"] = resolved;
    }

    match http_client
        .post("https://api.linear.app/graphql")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "trigix/1.0")
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if (200..=299).contains(&status) {
                // Check for GraphQL errors
                if let Some(errs) = body_json.get("errors") {
                    if !errs.is_null() && errs.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                        return NodeExecutionResult::failed(format!(
                            "Linear GraphQL errors: {errs}"
                        ));
                    }
                }
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "data": body_json.get("data").cloned().unwrap_or(body_json) }).to_string()
                )
            } else {
                NodeExecutionResult::failed(format!("Linear API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Linear request error: {e}")),
    }
}

pub(super) async fn execute_airtable(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Airtable node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Airtable node missing 'token'"),
    };
    let base_id = match cfg.get("base_id").and_then(|v| v.as_str()) {
        Some(b) => resolve_template(b, context),
        None => return NodeExecutionResult::failed("Airtable node missing 'base_id'"),
    };
    let table = match cfg.get("table").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Airtable node missing 'table'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let record_id = cfg
        .get("record_id")
        .and_then(|v| v.as_str())
        .map(|r| resolve_template(r, context));

    // Build URL: https://api.airtable.com/v0/{baseId}/{tableId}[/{recordId}]
    let base = format!(
        "https://api.airtable.com/v0/{}/{}",
        base_id,
        urlencoding::encode(&table)
    );
    let url = match &record_id {
        Some(rid) if !rid.is_empty() => format!("{base}/{rid}"),
        _ => base.clone(),
    };

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PATCH" => http_client.patch(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        _ => {
            // GET with optional filterByFormula
            let mut get_req = http_client.get(&url);
            if let Some(formula) = cfg.get("filter_formula").and_then(|v| v.as_str()) {
                let resolved = resolve_template(formula, context);
                get_req = get_req.query(&[("filterByFormula", resolved)]);
            }
            if let Some(max_records) = cfg.get("max_records").and_then(|v| v.as_u64()) {
                get_req = get_req.query(&[("maxRecords", max_records.to_string())]);
            }
            get_req
        }
    }
    .header("Authorization", format!("Bearer {token}"))
    .header("Content-Type", "application/json")
    .header("User-Agent", "trigix/1.0");

    if method != "GET" && method != "DELETE" {
        if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
            let resolved =
                resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
            if let Some(body_val) = resolved.get("body") {
                req = req.json(body_val);
            }
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Airtable API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Airtable request error: {e}")),
    }
}

pub(super) async fn execute_discord(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Discord node requires config"),
    };
    let webhook_url = match cfg.get("webhook_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Discord node missing 'webhook_url'"),
    };
    let content = match cfg.get("content").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Discord node missing 'content'"),
    };

    let mut payload = serde_json::json!({ "content": content });
    if let Some(u) = cfg.get("username").and_then(|v| v.as_str()) {
        let r = resolve_template(u, context);
        if !r.is_empty() {
            payload["username"] = serde_json::json!(r);
        }
    }
    if let Some(a) = cfg.get("avatar_url").and_then(|v| v.as_str()) {
        let r = resolve_template(a, context);
        if !r.is_empty() {
            payload["avatar_url"] = serde_json::json!(r);
        }
    }

    match http_client.post(&webhook_url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 204 => {
            NodeExecutionResult::succeeded(
                serde_json::json!({ "ok": true, "content": content }).to_string(),
            )
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Discord webhook {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Discord error: {e}")),
    }
}

pub(super) async fn execute_teams(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Teams node requires config"),
    };
    let webhook_url = match cfg.get("webhook_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Teams node missing 'webhook_url'"),
    };
    let text = match cfg.get("text").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Teams node missing 'text'"),
    };
    let title = cfg
        .get("title")
        .and_then(|v| v.as_str())
        .map(|t| resolve_template(t, context))
        .unwrap_or_default();
    let color = cfg
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("0078D4");
    let color = color.trim_start_matches('#');

    // MessageCard format (works with all Teams webhook URLs including Power Automate connectors)
    let payload = serde_json::json!({
        "@type": "MessageCard",
        "@context": "http://schema.org/extensions",
        "themeColor": color,
        "summary": if title.is_empty() { text.chars().take(80).collect::<String>() } else { title.clone() },
        "sections": [{
            "activityTitle": if title.is_empty() { serde_json::Value::Null } else { serde_json::json!(title) },
            "text": text,
        }],
    });

    match http_client.post(&webhook_url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => NodeExecutionResult::succeeded(
            serde_json::json!({ "ok": true, "text": text }).to_string(),
        ),
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Teams webhook {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Teams error: {e}")),
    }
}

pub(super) async fn execute_sheets(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Sheets node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Sheets node missing 'token'"),
    };
    let spreadsheet_id = match cfg.get("spreadsheet_id").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Sheets node missing 'spreadsheet_id'"),
    };
    let range = match cfg.get("range").and_then(|v| v.as_str()) {
        Some(r) => resolve_template(r, context),
        None => return NodeExecutionResult::failed("Sheets node missing 'range'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let value_input = cfg
        .get("value_input_option")
        .and_then(|v| v.as_str())
        .unwrap_or("USER_ENTERED");

    let encoded_range = urlencoding::encode(&range);
    let base = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}/values/{encoded_range}"
    );

    let resp = match method.as_str() {
        "APPEND" => {
            let url = format!("{base}:append?valueInputOption={value_input}");
            let values_raw = cfg.get("values").and_then(|v| v.as_str()).unwrap_or("[]");
            let resolved = resolve_config_strings(&serde_json::json!({ "v": values_raw }), context);
            let values = resolved.get("v").cloned().unwrap_or(serde_json::json!([]));
            let body = serde_json::json!({ "values": values });
            http_client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&body)
                .send()
                .await
        }
        "UPDATE" => {
            let url = format!("{base}?valueInputOption={value_input}");
            let values_raw = cfg.get("values").and_then(|v| v.as_str()).unwrap_or("[]");
            let resolved = resolve_config_strings(&serde_json::json!({ "v": values_raw }), context);
            let values = resolved.get("v").cloned().unwrap_or(serde_json::json!([]));
            let body =
                serde_json::json!({ "range": range, "majorDimension": "ROWS", "values": values });
            http_client
                .put(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&body)
                .send()
                .await
        }
        "CLEAR" => {
            let url = format!("{base}:clear");
            http_client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&serde_json::json!({}))
                .send()
                .await
        }
        _ => {
            // GET — read values
            http_client
                .get(&base)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
        }
    };

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let body_text = r.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                // For GET, extract the values array for convenience
                let values = body_json.get("values").cloned();
                let mut out = serde_json::json!({ "status": status, "body": body_json });
                if let Some(v) = values {
                    out["values"] = v;
                }
                NodeExecutionResult::succeeded(out.to_string())
            } else {
                NodeExecutionResult::failed(format!("Sheets API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Sheets request error: {e}")),
    }
}

pub(super) async fn execute_redis(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Redis node requires config"),
    };
    let url_raw = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return NodeExecutionResult::failed("Redis node missing 'url'"),
    };
    let url = resolve_template(url_raw, context);
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("get");

    let client = match redis::Client::open(url.as_str()) {
        Ok(c) => c,
        Err(e) => return NodeExecutionResult::failed(format!("Redis client error: {e}")),
    };
    let mut con = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => return NodeExecutionResult::failed(format!("Redis connect failed: {e}")),
    };

    let key = cfg
        .get("key")
        .and_then(|v| v.as_str())
        .map(|k| resolve_template(k, context))
        .unwrap_or_default();
    let value_resolved = cfg
        .get("value")
        .and_then(|v| v.as_str())
        .map(|v| resolve_template(v, context))
        .unwrap_or_default();
    let field = cfg
        .get("field")
        .and_then(|v| v.as_str())
        .map(|f| resolve_template(f, context))
        .unwrap_or_default();
    let ttl = cfg.get("ttl_secs").and_then(|v| v.as_i64()).unwrap_or(0);
    let amount = cfg.get("amount").and_then(|v| v.as_i64()).unwrap_or(1);

    let raw: Result<redis::Value, redis::RedisError> = match operation {
        "get" => redis::cmd("GET").arg(&key).query_async(&mut con).await,
        "set" => {
            let mut cmd = redis::cmd("SET");
            cmd.arg(&key).arg(&value_resolved);
            if ttl > 0 {
                cmd.arg("EX").arg(ttl);
            }
            cmd.query_async(&mut con).await
        }
        "del" => redis::cmd("DEL").arg(&key).query_async(&mut con).await,
        "exists" => redis::cmd("EXISTS").arg(&key).query_async(&mut con).await,
        "incr" => redis::cmd("INCR").arg(&key).query_async(&mut con).await,
        "decr" => redis::cmd("DECR").arg(&key).query_async(&mut con).await,
        "incrby" => {
            redis::cmd("INCRBY")
                .arg(&key)
                .arg(amount)
                .query_async(&mut con)
                .await
        }
        "expire" => {
            redis::cmd("EXPIRE")
                .arg(&key)
                .arg(ttl)
                .query_async(&mut con)
                .await
        }
        "ttl" => redis::cmd("TTL").arg(&key).query_async(&mut con).await,
        "hget" => {
            redis::cmd("HGET")
                .arg(&key)
                .arg(&field)
                .query_async(&mut con)
                .await
        }
        "hset" => {
            redis::cmd("HSET")
                .arg(&key)
                .arg(&field)
                .arg(&value_resolved)
                .query_async(&mut con)
                .await
        }
        "hdel" => {
            redis::cmd("HDEL")
                .arg(&key)
                .arg(&field)
                .query_async(&mut con)
                .await
        }
        "hgetall" => redis::cmd("HGETALL").arg(&key).query_async(&mut con).await,
        "lpush" => {
            redis::cmd("LPUSH")
                .arg(&key)
                .arg(&value_resolved)
                .query_async(&mut con)
                .await
        }
        "lpop" => redis::cmd("LPOP").arg(&key).query_async(&mut con).await,
        "rpush" => {
            redis::cmd("RPUSH")
                .arg(&key)
                .arg(&value_resolved)
                .query_async(&mut con)
                .await
        }
        "rpop" => redis::cmd("RPOP").arg(&key).query_async(&mut con).await,
        "llen" => redis::cmd("LLEN").arg(&key).query_async(&mut con).await,
        "ping" => redis::cmd("PING").query_async(&mut con).await,
        "keys" => redis::cmd("KEYS").arg(&key).query_async(&mut con).await,
        op => return NodeExecutionResult::failed(format!("Unknown Redis operation: {op}")),
    };

    match raw {
        Ok(val) => {
            // hgetall with old Redis (<7) returns Array of alternating k/v;
            // with RESP3 it returns a Map. redis_value_to_json handles both.
            let json_val = redis_value_to_json(val);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "value": json_val, "operation": operation, "key": key })
                    .to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Redis {operation} error: {e}")),
    }
}

pub(super) async fn execute_elasticsearch(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Elasticsearch node requires config"),
    };
    let base_url_raw = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return NodeExecutionResult::failed("Elasticsearch node missing 'url'"),
    };
    let base_url = resolve_template(base_url_raw, context)
        .trim_end_matches('/')
        .to_string();
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .map(|e| resolve_template(e, context))
        .unwrap_or_else(|| "/_search".to_string());
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("{base_url}{endpoint}");

    let body_val = cfg.get("body").and_then(|v| v.as_str()).map(|s| {
        let resolved = resolve_template(s, context);
        serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
    });

    let mut builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Content-Type", "application/json");

    // Optional auth: api_key or username/password
    if let Some(api_key) = cfg.get("api_key").and_then(|v| v.as_str()) {
        let key = resolve_template(api_key, context);
        builder = builder.header("Authorization", format!("ApiKey {key}"));
    } else if let (Some(user), Some(pass)) = (
        cfg.get("username").and_then(|v| v.as_str()),
        cfg.get("password").and_then(|v| v.as_str()),
    ) {
        let user = resolve_template(user, context);
        let pass = resolve_template(pass, context);
        builder = builder.basic_auth(user, Some(pass));
    }

    if let Some(ref v) = body_val {
        if v != &serde_json::Value::Null {
            builder = builder.json(v);
        }
    }

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let took = json.get("took").cloned().unwrap_or(serde_json::Value::Null);
                let hits_total = json
                    .pointer("/hits/total/value")
                    .or_else(|| json.pointer("/hits/total"))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": json, "took": took, "hits_total": hits_total }).to_string()
                )
            } else {
                NodeExecutionResult::failed(format!("Elasticsearch {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Elasticsearch request error: {e}")),
    }
}

pub(super) async fn execute_pagerduty(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("PagerDuty node requires config"),
    };
    let routing_key = match cfg.get("routing_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("PagerDuty node missing 'routing_key'"),
    };
    let summary = match cfg.get("summary").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("PagerDuty node missing 'summary'"),
    };
    let event_action = cfg
        .get("event_action")
        .and_then(|v| v.as_str())
        .unwrap_or("trigger");
    let severity = cfg
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("error");
    let source = cfg
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_else(|| "trigix".to_string());
    let dedup_key = cfg
        .get("dedup_key")
        .and_then(|v| v.as_str())
        .map(|k| resolve_template(k, context));

    let mut body = serde_json::json!({
        "routing_key": routing_key,
        "event_action": event_action,
        "payload": {
            "summary": summary,
            "severity": severity,
            "source": source,
        }
    });
    if let Some(dk) = dedup_key {
        body["dedup_key"] = serde_json::Value::String(dk);
    }
    // Optional extra payload fields
    for field in &["component", "group", "class"] {
        if let Some(val) = cfg.get(field).and_then(|v| v.as_str()) {
            let resolved = resolve_template(val, context);
            body["payload"][field] = serde_json::Value::String(resolved);
        }
    }

    match http_client
        .post("https://events.pagerduty.com/v2/enqueue")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let msg = json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let dk = json
                    .get("dedup_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "message": msg, "dedup_key": dk })
                        .to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("PagerDuty API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("PagerDuty request error: {e}")),
    }
}

pub(super) async fn execute_hubspot(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("HubSpot node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("HubSpot node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("HubSpot node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://api.hubapi.com{endpoint}");

    let body_val = cfg.get("body").and_then(|v| v.as_str()).map(|s| {
        let resolved = resolve_template(s, context);
        serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
    });

    let builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    let builder = match body_val {
        Some(ref v) if v != &serde_json::Value::Null => builder.json(v),
        _ => builder,
    };

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("HubSpot API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("HubSpot request error: {e}")),
    }
}

pub(super) async fn execute_zendesk(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Zendesk node requires config"),
    };
    let subdomain = match cfg.get("subdomain").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Zendesk node missing 'subdomain'"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Zendesk node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Zendesk node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://{subdomain}.zendesk.com/api/v2{endpoint}");

    let body_val = cfg.get("body").and_then(|v| v.as_str()).map(|s| {
        let resolved = resolve_template(s, context);
        serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
    });

    let builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    let builder = match body_val {
        Some(ref v) if v != &serde_json::Value::Null => builder.json(v),
        _ => builder,
    };

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Zendesk API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Zendesk request error: {e}")),
    }
}

pub(super) async fn execute_twilio(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Twilio node requires config"),
    };
    let account_sid = match cfg.get("account_sid").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'account_sid'"),
    };
    let auth_token = match cfg.get("auth_token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'auth_token'"),
    };
    let to = match cfg.get("to").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'to'"),
    };
    let from = match cfg.get("from").and_then(|v| v.as_str()) {
        Some(f) => resolve_template(f, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'from'"),
    };
    let body = match cfg.get("body").and_then(|v| v.as_str()) {
        Some(b) => resolve_template(b, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'body'"),
    };

    let url = format!("https://api.twilio.com/2010-04-01/Accounts/{account_sid}/Messages.json");
    let params = [
        ("To", to.as_str()),
        ("From", from.as_str()),
        ("Body", body.as_str()),
    ];

    let resp = http_client
        .post(&url)
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&params)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let sid = json
                    .get("sid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let msg_status = json
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "sid": sid, "status": msg_status, "to": to, "from": from, "body": json }).to_string()
                )
            } else {
                NodeExecutionResult::failed(format!("Twilio API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Twilio request error: {e}")),
    }
}

pub(super) async fn execute_stripe(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Stripe node requires config"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Stripe node missing 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Stripe node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://api.stripe.com/v1{}", endpoint);

    let body_val = cfg
        .get("body")
        .and_then(|v| v.as_str())
        .map(|s| {
            let resolved = resolve_template(s, context);
            serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
        })
        .unwrap_or(serde_json::Value::Null);

    let builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Stripe-Version", "2024-06-20");

    let builder = if (method == "POST" || method == "PATCH") && body_val.is_object() {
        // Form-encode flat object for Stripe v1 API
        let params: Vec<(String, String)> = body_val
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let val = v
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| v.to_string());
                (k.clone(), val)
            })
            .collect();
        builder.form(&params)
    } else if method == "GET" && body_val.is_object() {
        let params: Vec<(String, String)> = body_val
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let val = v
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| v.to_string());
                (k.clone(), val)
            })
            .collect();
        builder.query(&params)
    } else {
        builder
    };

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let id = json
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let obj = json
                    .get("object")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "id": id, "object": obj, "body": json })
                        .to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Stripe API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Stripe request error: {e}")),
    }
}

pub(super) async fn execute_shopify(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Shopify node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let shop = match cfg.get("shop").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Shopify node missing 'shop'"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Shopify node missing 'token'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/products.json");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let api_version = cfg
        .get("api_version")
        .and_then(|v| v.as_str())
        .unwrap_or("2024-01");

    let url = format!("https://{shop}.myshopify.com/admin/api/{api_version}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("X-Shopify-Access-Token", &token)
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
        Err(e) => NodeExecutionResult::failed(format!("Shopify request error: {e}")),
    }
}

pub(super) async fn execute_datadog(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Datadog node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Datadog node missing 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Datadog node missing 'endpoint'"),
    };
    let site = cfg
        .get("site")
        .and_then(|v| v.as_str())
        .unwrap_or("datadoghq.com");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let app_key = cfg
        .get("app_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let url = format!("https://api.{site}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("DD-API-KEY", &api_key)
        .header("Content-Type", "application/json");

    if !app_key.is_empty() {
        req = req.header("DD-APPLICATION-KEY", &app_key);
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
        Err(e) => NodeExecutionResult::failed(format!("Datadog request error: {e}")),
    }
}

pub(super) async fn execute_salesforce(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Salesforce node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Salesforce node missing 'token' (OAuth access token)",
            )
        }
    };
    let instance_url = match cfg.get("instance_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed("Salesforce node missing 'instance_url'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/services/data/v59.0/sobjects");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("{instance_url}{endpoint}");

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
        Err(e) => NodeExecutionResult::failed(format!("Salesforce request error: {e}")),
    }
}

pub(super) async fn execute_freshdesk(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Freshdesk node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Freshdesk node missing 'api_key'"),
    };
    let domain = match cfg.get("domain").and_then(|v| v.as_str()) {
        Some(d) if !d.is_empty() => d.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Freshdesk node missing 'domain' (e.g. yourcompany.freshdesk.com)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Freshdesk node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    // Freshdesk uses HTTP Basic auth: api_key as username, "X" as password
    use base64::Engine as _;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!("{api_key}:X").as_bytes());

    let url = format!("https://{domain}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Freshdesk request error: {e}")),
    }
}

pub(super) async fn execute_mailgun(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Mailgun node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Mailgun node missing 'api_key'"),
    };
    let domain = match cfg.get("domain").and_then(|v| v.as_str()) {
        Some(d) if !d.is_empty() => d.to_string(),
        _ => return NodeExecutionResult::failed("Mailgun node missing 'domain' (sending domain)"),
    };
    let to = match cfg.get("to").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Mailgun node missing 'to' address"),
    };
    let from = cfg
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or("noreply@example.com")
        .to_string();
    let subject = cfg
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("(no subject)")
        .to_string();

    // Support html or text content
    let html = cfg.get("html").and_then(|v| v.as_str()).map(str::to_string);
    let text = cfg.get("text").and_then(|v| v.as_str()).map(str::to_string);
    let region = cfg.get("region").and_then(|v| v.as_str()).unwrap_or("us");
    let base = if region == "eu" {
        "api.eu.mailgun.net"
    } else {
        "api.mailgun.net"
    };

    use base64::Engine as _;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!("api:{api_key}").as_bytes());

    let url = format!("https://{base}/v3/{domain}/messages");

    let mut params = vec![
        ("from".to_string(), from),
        ("to".to_string(), to),
        ("subject".to_string(), subject),
    ];
    if let Some(h) = html {
        params.push(("html".to_string(), h));
    }
    if let Some(t) = text {
        params.push(("text".to_string(), t));
    }

    match client
        .post(&url)
        .header("Authorization", format!("Basic {credentials}"))
        .form(&params)
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
        Err(e) => NodeExecutionResult::failed(format!("Mailgun request error: {e}")),
    }
}

pub(super) async fn execute_asana(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Asana node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Asana node missing 'token' (Personal Access Token)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Asana node missing 'endpoint' (e.g. /tasks)"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://app.asana.com/api/1.0{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Asana request error: {e}")),
    }
}

pub(super) async fn execute_servicenow(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("ServiceNow node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let instance = match cfg.get("instance").and_then(|v| v.as_str()) {
        Some(i) if !i.is_empty() => i.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "ServiceNow node missing 'instance' (e.g. myco.service-now.com)",
            )
        }
    };
    let username = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("ServiceNow node missing 'username'"),
    };
    let password = match cfg.get("password").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("ServiceNow node missing 'password'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/api/now/table/incident");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    use base64::Engine as _;
    let credentials = base64::engine::general_purpose::STANDARD
        .encode(format!("{username}:{password}").as_bytes());

    let url = format!("https://{instance}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
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
        Err(e) => NodeExecutionResult::failed(format!("ServiceNow request error: {e}")),
    }
}

pub(super) async fn execute_confluence(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Confluence node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let base_url = match cfg.get("base_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Confluence node missing 'base_url' (e.g. https://myco.atlassian.net/wiki)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Confluence node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    // Support either Bearer token or Basic auth (email + api_token)
    let auth_header = if let Some(token) = cfg
        .get("token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        format!("Bearer {token}")
    } else {
        let email = cfg.get("email").and_then(|v| v.as_str()).unwrap_or("");
        let api_token = cfg.get("api_token").and_then(|v| v.as_str()).unwrap_or("");
        if email.is_empty() || api_token.is_empty() {
            return NodeExecutionResult::failed(
                "Confluence node requires either 'token' or both 'email' and 'api_token'",
            );
        }
        use base64::Engine as _;
        let creds = base64::engine::general_purpose::STANDARD
            .encode(format!("{email}:{api_token}").as_bytes());
        format!("Basic {creds}")
    };

    let url = format!("{base_url}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", auth_header)
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
        Err(e) => NodeExecutionResult::failed(format!("Confluence request error: {e}")),
    }
}

pub(super) async fn execute_bitbucket(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Bitbucket node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let username = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("Bitbucket node missing 'username'"),
    };
    let app_password = match cfg.get("app_password").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("Bitbucket node missing 'app_password'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Bitbucket node missing 'endpoint' (e.g. /repositories/workspace/slug)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    use base64::Engine as _;
    let credentials = base64::engine::general_purpose::STANDARD
        .encode(format!("{username}:{app_password}").as_bytes());

    let url = format!("https://api.bitbucket.org/2.0{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Bitbucket request error: {e}")),
    }
}

pub(super) async fn execute_azure_devops(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Azure DevOps node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let pat = match cfg.get("pat").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Azure DevOps node missing 'pat' (Personal Access Token)",
            )
        }
    };
    let organization = match cfg.get("organization").and_then(|v| v.as_str()) {
        Some(o) if !o.is_empty() => o.to_string(),
        _ => return NodeExecutionResult::failed("Azure DevOps node missing 'organization'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Azure DevOps node missing 'endpoint'"),
    };
    let project = cfg
        .get("project")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let api_ver = cfg
        .get("api_version")
        .and_then(|v| v.as_str())
        .unwrap_or("7.1");

    // ADO uses Basic auth with empty username and PAT as password
    use base64::Engine as _;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!(":{pat}").as_bytes());

    // Build base URL: https://dev.azure.com/{org}/{project}/_apis{endpoint}
    let base = if project.is_empty() {
        format!("https://dev.azure.com/{organization}/_apis{endpoint}")
    } else {
        format!("https://dev.azure.com/{organization}/{project}/_apis{endpoint}")
    };
    let url = if base.contains('?') {
        format!("{base}&api-version={api_ver}")
    } else {
        format!("{base}?api-version={api_ver}")
    };

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Azure DevOps request error: {e}")),
    }
}
