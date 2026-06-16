// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Message-broker nodes reachable over HTTP: Kafka (Confluent REST Proxy) and
//! RabbitMQ (Management HTTP API). Both return `{status, body}`.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use workflow_core::Node;

// ── Kafka (Confluent REST Proxy v2) ───────────────────────────────────────────
pub(super) async fn execute_kafka(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let proxy_url = match cfg.get("proxy_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed(
            "Kafka requires 'proxy_url' (Confluent REST Proxy base, e.g. http://localhost:8082)",
        ),
    };
    let topic = match cfg.get("topic").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Kafka requires 'topic'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("produce")
        .to_string();

    match operation.as_str() {
        "produce" => {
            // Either a single `value` or a ready-made `records` array.
            let records = if let Some(records) = cfg.get("records") {
                json_array_or_parse(records)
            } else if let Some(value) = cfg.get("value") {
                let mut record = serde_json::json!({ "value": json_array_or_parse(value) });
                if let Some(key) = cfg.get("key") {
                    record["key"] = json_array_or_parse(key);
                }
                if let Some(p) = cfg.get("partition").and_then(|v| v.as_u64()) {
                    record["partition"] = serde_json::json!(p);
                }
                serde_json::json!([record])
            } else {
                return NodeExecutionResult::failed(
                    "Kafka produce requires 'value' (or a 'records' array)",
                );
            };
            let body = serde_json::json!({ "records": records });
            let url = format!("{proxy_url}/topics/{topic}");
            let mut req = http_client
                .post(&url)
                .header("Content-Type", "application/vnd.kafka.json.v2+json")
                .header("Accept", "application/vnd.kafka.v2+json")
                .json(&body);
            if let Some(key) = cfg.get("api_key").and_then(|v| v.as_str()) {
                let secret = cfg.get("api_secret").and_then(|v| v.as_str()).unwrap_or("");
                let enc =
                    base64::engine::general_purpose::STANDARD.encode(format!("{key}:{secret}"));
                req = req.header("Authorization", format!("Basic {enc}"));
            }
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Kafka produce error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Kafka unknown operation '{other}'")),
    }
}

// ── RabbitMQ (Management HTTP API) ─────────────────────────────────────────────
pub(super) async fn execute_rabbitmq(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "RabbitMQ requires 'host' (Management API base, e.g. http://localhost:15672)",
            )
        }
    };
    let username = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("RabbitMQ requires 'username'"),
    };
    let password = cfg
        .get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("publish")
        .to_string();
    // vhost defaults to "/" and must be percent-encoded in the path.
    let vhost = cfg.get("vhost").and_then(|v| v.as_str()).unwrap_or("/");
    let vhost_enc = urlencoding::encode(vhost).to_string();

    let auth = base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));

    let send = |rb: reqwest::RequestBuilder, op: &'static str| async move {
        match rb
            .header("Authorization", format!("Basic {auth}"))
            .header("Content-Type", "application/json")
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
            Err(e) => NodeExecutionResult::failed(format!("RabbitMQ {op} error: {e}")),
        }
    };

    match operation.as_str() {
        "publish" => {
            let payload = match cfg.get("payload").and_then(|v| v.as_str()) {
                Some(p) => p.to_string(),
                None => return NodeExecutionResult::failed("RabbitMQ publish requires 'payload'"),
            };
            // Default exchange ("") routes by queue name via routing_key.
            let exchange = cfg.get("exchange").and_then(|v| v.as_str()).unwrap_or("");
            let routing_key = cfg
                .get("routing_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let body = serde_json::json!({
                "properties": {},
                "routing_key": routing_key,
                "payload": payload,
                "payload_encoding": "string",
            });
            let url = format!("{host}/api/exchanges/{vhost_enc}/{exchange}/publish");
            send(http_client.post(&url).json(&body), "publish").await
        }
        "get" => {
            let queue = match cfg.get("queue").and_then(|v| v.as_str()) {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => return NodeExecutionResult::failed("RabbitMQ get requires 'queue'"),
            };
            let count = cfg.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
            let body = serde_json::json!({
                "count": count,
                "ackmode": "ack_requeue_true",
                "encoding": "auto",
            });
            let url = format!("{host}/api/queues/{vhost_enc}/{queue}/get");
            send(http_client.post(&url).json(&body), "get").await
        }
        "list_queues" => {
            let url = format!("{host}/api/queues/{vhost_enc}");
            send(http_client.get(&url), "list_queues").await
        }
        other => NodeExecutionResult::failed(format!("RabbitMQ unknown operation '{other}'")),
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
    async fn kafka_requires_proxy_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "k1".into(),
            node_type: NodeType::Kafka,
            config: Some(serde_json::json!({"topic":"t","operation":"produce"})),
        };
        let r = execute_kafka(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("proxy_url"));
    }

    #[tokio::test]
    async fn kafka_produce_requires_value() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "k2".into(),
            node_type: NodeType::Kafka,
            config: Some(serde_json::json!({
                "proxy_url":"http://localhost:8082","topic":"t","operation":"produce"
            })),
        };
        let r = execute_kafka(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("value"));
    }

    #[tokio::test]
    async fn rabbitmq_requires_host() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r1".into(),
            node_type: NodeType::Rabbitmq,
            config: Some(serde_json::json!({"username":"guest","operation":"publish"})),
        };
        let r = execute_rabbitmq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn rabbitmq_publish_requires_payload() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r2".into(),
            node_type: NodeType::Rabbitmq,
            config: Some(serde_json::json!({
                "host":"http://localhost:15672","username":"guest","password":"guest",
                "operation":"publish"
            })),
        };
        let r = execute_rabbitmq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("payload"));
    }

    #[tokio::test]
    async fn rabbitmq_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r3".into(),
            node_type: NodeType::Rabbitmq,
            config: Some(serde_json::json!({
                "host":"http://localhost:15672","username":"guest","operation":"nope"
            })),
        };
        let r = execute_rabbitmq(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }
}
