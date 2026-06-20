// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! Chat / messaging integration nodes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

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
}
