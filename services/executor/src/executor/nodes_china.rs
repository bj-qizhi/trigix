// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Chinese enterprise collaboration nodes over their HTTP bot/webhook APIs:
//! Feishu / Lark (飞书), DingTalk (钉钉), WeChat Work (企业微信). All return
//! `{status, body}`.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use workflow_core::Node;

async fn post_json(
    http_client: &reqwest::Client,
    url: &str,
    body: &serde_json::Value,
    node: &str,
) -> NodeExecutionResult {
    match http_client
        .post(url)
        .header("Content-Type", "application/json")
        .json(body)
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
        Err(e) => NodeExecutionResult::failed(format!("{node} request error: {e}")),
    }
}

// ── 飞书 / Lark ───────────────────────────────────────────────────────────────
pub(super) async fn execute_feishu(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let msg_type = cfg
        .get("msg_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .to_string();

    // Custom-bot webhook is the primary path; app mode (tenant token) is the fallback.
    if let Some(webhook) = cfg.get("webhook_url").and_then(|v| v.as_str()) {
        if !webhook.is_empty() {
            let body = match msg_type.as_str() {
                "text" => {
                    let text = cfg.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    serde_json::json!({ "msg_type": "text", "content": { "text": text } })
                }
                "interactive" => {
                    let card = cfg
                        .get("card")
                        .map(json_array_or_parse)
                        .unwrap_or(serde_json::json!({}));
                    serde_json::json!({ "msg_type": "interactive", "card": card })
                }
                other => {
                    return NodeExecutionResult::failed(format!(
                        "Feishu unsupported msg_type '{other}' for webhook (text/interactive)"
                    ))
                }
            };
            return post_json(http_client, webhook, &body, "Feishu").await;
        }
    }

    // App mode: caller supplies a tenant_access_token and a receive_id.
    let token = match cfg.get("tenant_access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed(
            "Feishu requires 'webhook_url' (custom bot) or 'tenant_access_token' + 'receive_id'",
        ),
    };
    let receive_id = match cfg.get("receive_id").and_then(|v| v.as_str()) {
        Some(r) if !r.is_empty() => r.to_string(),
        _ => return NodeExecutionResult::failed("Feishu app mode requires 'receive_id'"),
    };
    let receive_id_type = cfg
        .get("receive_id_type")
        .and_then(|v| v.as_str())
        .unwrap_or("open_id");
    let text = cfg.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let content = serde_json::json!({ "text": text }).to_string();
    let body = serde_json::json!({
        "receive_id": receive_id,
        "msg_type": "text",
        "content": content,
    });
    let url = format!(
        "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type={receive_id_type}"
    );
    match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Feishu request error: {e}")),
    }
}

// ── 钉钉 / DingTalk (自定义机器人) ─────────────────────────────────────────────
pub(super) async fn execute_dingtalk(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("DingTalk requires 'access_token' (robot token)"),
    };
    let msg_type = cfg
        .get("msg_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .to_string();

    let body = match msg_type.as_str() {
        "text" => {
            let content = match cfg.get("content").and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return NodeExecutionResult::failed("DingTalk text requires 'content'"),
            };
            serde_json::json!({ "msgtype": "text", "text": { "content": content } })
        }
        "markdown" => {
            let content = match cfg.get("content").and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return NodeExecutionResult::failed("DingTalk markdown requires 'content'"),
            };
            let title = cfg
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("notice");
            serde_json::json!({
                "msgtype": "markdown",
                "markdown": { "title": title, "text": content }
            })
        }
        other => {
            return NodeExecutionResult::failed(format!(
                "DingTalk unsupported msg_type '{other}' (text/markdown)"
            ))
        }
    };

    let mut url = format!("https://oapi.dingtalk.com/robot/send?access_token={access_token}");
    // Optional signed-webhook security (加签).
    if let Some(secret) = cfg.get("secret").and_then(|v| v.as_str()) {
        if !secret.is_empty() {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let string_to_sign = format!("{ts}\n{secret}");
            let mut mac =
                Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC any key length");
            mac.update(string_to_sign.as_bytes());
            let sign =
                base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
            url.push_str(&format!(
                "&timestamp={ts}&sign={}",
                urlencoding::encode(&sign)
            ));
        }
    }
    post_json(http_client, &url, &body, "DingTalk").await
}

// ── 企业微信 / WeChat Work (群机器人) ──────────────────────────────────────────
pub(super) async fn execute_wecom(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let key = match cfg.get("key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("WeChat Work requires 'key' (group robot key)"),
    };
    let msg_type = cfg
        .get("msg_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .to_string();
    let content = match cfg.get("content").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return NodeExecutionResult::failed("WeChat Work requires 'content'"),
    };
    let body = match msg_type.as_str() {
        "text" => serde_json::json!({ "msgtype": "text", "text": { "content": content } }),
        "markdown" => {
            serde_json::json!({ "msgtype": "markdown", "markdown": { "content": content } })
        }
        other => {
            return NodeExecutionResult::failed(format!(
                "WeChat Work unsupported msg_type '{other}' (text/markdown)"
            ))
        }
    };
    let url = format!("https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key={key}");
    post_json(http_client, &url, &body, "WeChat Work").await
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
    async fn feishu_requires_webhook_or_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "f1".into(),
            node_type: NodeType::Feishu,
            config: Some(serde_json::json!({"msg_type":"text","text":"hi"})),
        };
        let r = execute_feishu(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("webhook_url"));
    }

    #[tokio::test]
    async fn dingtalk_requires_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d1".into(),
            node_type: NodeType::Dingtalk,
            config: Some(serde_json::json!({"msg_type":"text","content":"hi"})),
        };
        let r = execute_dingtalk(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn dingtalk_text_requires_content() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d2".into(),
            node_type: NodeType::Dingtalk,
            config: Some(serde_json::json!({"access_token":"tok","msg_type":"text"})),
        };
        let r = execute_dingtalk(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("content"));
    }

    #[tokio::test]
    async fn wecom_requires_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w1".into(),
            node_type: NodeType::Wecom,
            config: Some(serde_json::json!({"content":"hi"})),
        };
        let r = execute_wecom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key"));
    }
}
