// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! AWS nodes signed with Signature Version 4 (no AWS SDK dependency): a shared
//! SigV4 signer plus SQS and SNS over their query (form-encoded) protocol.
//! Both return `{status, body}` (AWS replies are XML, surfaced as text).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use workflow_core::Node;

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Computes the SigV4 `Authorization` header for a request. `headers` must
/// already include every header that will be sent and that participates in the
/// signature (here: `host` and `content-type`); `x-amz-date` is added by this
/// function. Returns `(authorization, amz_date)`.
#[allow(clippy::too_many_arguments)]
fn sigv4_authorization(
    method: &str,
    host: &str,
    canonical_uri: &str,
    canonical_query: &str,
    extra_headers: &[(String, String)],
    body: &[u8],
    access_key: &str,
    secret_key: &str,
    region: &str,
    service: &str,
    amz_date: &str, // YYYYMMDDTHHMMSSZ
) -> String {
    let date_stamp = &amz_date[..8]; // YYYYMMDD

    // Build the canonical (sorted, lowercased) header set: host + x-amz-date + extras.
    let mut headers: Vec<(String, String)> = vec![
        ("host".to_string(), host.to_string()),
        ("x-amz-date".to_string(), amz_date.to_string()),
    ];
    for (k, v) in extra_headers {
        headers.push((k.to_lowercase(), v.trim().to_string()));
    }
    headers.sort_by(|a, b| a.0.cmp(&b.0));
    let canonical_headers: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed_headers: String = headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");

    let payload_hash = sha256_hex(body);
    let canonical_request = format!(
        "{method}\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    let scope = format!("{date_stamp}/{region}/{service}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );

    // Derive the signing key.
    let k_date = hmac_sha256(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

    format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    )
}

// form-urlencode a set of params into a sorted body (also the canonical form).
fn form_encode(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

// Shared driver for the SQS/SNS query protocol.
async fn aws_query_call(
    http_client: &reqwest::Client,
    service: &str,
    host: &str,
    region: &str,
    access_key: &str,
    secret_key: &str,
    params: Vec<(String, String)>,
    node_name: &str,
) -> NodeExecutionResult {
    let body = form_encode(&params);
    let amz_date = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let content_type = "application/x-www-form-urlencoded";
    let authorization = sigv4_authorization(
        "POST",
        host,
        "/",
        "",
        &[("content-type".to_string(), content_type.to_string())],
        body.as_bytes(),
        access_key,
        secret_key,
        region,
        service,
        &amz_date,
    );

    let url = format!("https://{host}/");
    match http_client
        .post(&url)
        .header("Content-Type", content_type)
        .header("X-Amz-Date", &amz_date)
        .header("Authorization", authorization)
        .body(body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": text }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("{node_name} request error: {e}")),
    }
}

// ── AWS SQS ───────────────────────────────────────────────────────────────────
pub(super) async fn execute_sqs(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let access_key = match cfg.get("access_key_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("SQS requires 'access_key_id'"),
    };
    let secret_key = match cfg.get("secret_access_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("SQS requires 'secret_access_key'"),
    };
    let region = cfg
        .get("region")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("us-east-1")
        .to_string();
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("send")
        .to_string();
    let queue_url = match cfg.get("queue_url").and_then(|v| v.as_str()) {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => return NodeExecutionResult::failed("SQS requires 'queue_url'"),
    };

    let mut params = vec![
        ("QueueUrl".to_string(), queue_url),
        ("Version".to_string(), "2012-11-05".to_string()),
    ];
    match operation.as_str() {
        "send" => {
            let msg = match cfg.get("message_body").and_then(|v| v.as_str()) {
                Some(m) => m.to_string(),
                None => return NodeExecutionResult::failed("SQS send requires 'message_body'"),
            };
            params.push(("Action".to_string(), "SendMessage".to_string()));
            params.push(("MessageBody".to_string(), msg));
            if let Some(group) = cfg.get("message_group_id").and_then(|v| v.as_str()) {
                if !group.is_empty() {
                    params.push(("MessageGroupId".to_string(), group.to_string()));
                }
            }
        }
        "receive" => {
            params.push(("Action".to_string(), "ReceiveMessage".to_string()));
            let max = cfg
                .get("max_messages")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            params.push(("MaxNumberOfMessages".to_string(), max.to_string()));
        }
        "delete" => {
            let handle = match cfg.get("receipt_handle").and_then(|v| v.as_str()) {
                Some(h) if !h.is_empty() => h.to_string(),
                _ => return NodeExecutionResult::failed("SQS delete requires 'receipt_handle'"),
            };
            params.push(("Action".to_string(), "DeleteMessage".to_string()));
            params.push(("ReceiptHandle".to_string(), handle));
        }
        other => return NodeExecutionResult::failed(format!("SQS unknown operation '{other}'")),
    }

    let host = format!("sqs.{region}.amazonaws.com");
    aws_query_call(
        http_client,
        "sqs",
        &host,
        &region,
        &access_key,
        &secret_key,
        params,
        "SQS",
    )
    .await
}

// ── AWS SNS ───────────────────────────────────────────────────────────────────
pub(super) async fn execute_sns(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let access_key = match cfg.get("access_key_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("SNS requires 'access_key_id'"),
    };
    let secret_key = match cfg.get("secret_access_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("SNS requires 'secret_access_key'"),
    };
    let region = cfg
        .get("region")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("us-east-1")
        .to_string();
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("publish")
        .to_string();

    let mut params = vec![("Version".to_string(), "2010-03-31".to_string())];
    match operation.as_str() {
        "publish" => {
            let message = match cfg.get("message").and_then(|v| v.as_str()) {
                Some(m) => m.to_string(),
                None => return NodeExecutionResult::failed("SNS publish requires 'message'"),
            };
            params.push(("Action".to_string(), "Publish".to_string()));
            params.push(("Message".to_string(), message));
            // Exactly one of topic_arn / target_arn / phone_number addresses the publish.
            if let Some(topic) = cfg.get("topic_arn").and_then(|v| v.as_str()) {
                if !topic.is_empty() {
                    params.push(("TopicArn".to_string(), topic.to_string()));
                }
            }
            if let Some(target) = cfg.get("target_arn").and_then(|v| v.as_str()) {
                if !target.is_empty() {
                    params.push(("TargetArn".to_string(), target.to_string()));
                }
            }
            if let Some(phone) = cfg.get("phone_number").and_then(|v| v.as_str()) {
                if !phone.is_empty() {
                    params.push(("PhoneNumber".to_string(), phone.to_string()));
                }
            }
            if let Some(subject) = cfg.get("subject").and_then(|v| v.as_str()) {
                if !subject.is_empty() {
                    params.push(("Subject".to_string(), subject.to_string()));
                }
            }
        }
        other => return NodeExecutionResult::failed(format!("SNS unknown operation '{other}'")),
    }

    let host = format!("sns.{region}.amazonaws.com");
    aws_query_call(
        http_client,
        "sns",
        &host,
        &region,
        &access_key,
        &secret_key,
        params,
        "SNS",
    )
    .await
}

// ── AWS Bedrock (InvokeModel) ─────────────────────────────────────────────────
// Reuses the SigV4 signer with a JSON body. The request body is model-native
// (the caller supplies the schema their model expects), so this stays a thin,
// faithful pass-through rather than guessing per-model payloads.
pub(super) async fn execute_bedrock(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let access_key = match cfg.get("access_key_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Bedrock requires 'access_key_id'"),
    };
    let secret_key = match cfg.get("secret_access_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Bedrock requires 'secret_access_key'"),
    };
    let region = cfg
        .get("region")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("us-east-1")
        .to_string();
    let model_id = match cfg.get("model_id").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Bedrock requires 'model_id' (e.g. anthropic.claude-3-5-sonnet-20240620-v1:0)",
            )
        }
    };
    // Model-native request body (JSON object/string accepted).
    let body_value = match cfg.get("body") {
        Some(b) => json_array_or_parse(b),
        None => return NodeExecutionResult::failed("Bedrock requires 'body' (model-native JSON)"),
    };
    let body = body_value.to_string();

    let host = format!("bedrock-runtime.{region}.amazonaws.com");
    // Path segment for the model id must be percent-encoded (it contains ':').
    let canonical_uri = format!("/model/{}/invoke", urlencoding::encode(&model_id));
    let amz_date = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let authorization = sigv4_authorization(
        "POST",
        &host,
        &canonical_uri,
        "",
        &[("content-type".to_string(), "application/json".to_string())],
        body.as_bytes(),
        &access_key,
        &secret_key,
        &region,
        "bedrock",
        &amz_date,
    );

    let url = format!("https://{host}{canonical_uri}");
    match http_client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("X-Amz-Date", &amz_date)
        .header("Authorization", authorization)
        .body(body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            let parsed = serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or(serde_json::Value::String(text));
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": parsed }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Bedrock request error: {e}")),
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

    // AWS SigV4 published test vector "get-vanilla": a GET to
    // host example.amazonaws.com at 20150830T123600Z must yield this exact
    // signature. Proves the canonical-request / signing-key math is correct.
    #[test]
    fn sigv4_matches_aws_get_vanilla_vector() {
        let auth = sigv4_authorization(
            "GET",
            "example.amazonaws.com",
            "/",
            "",
            &[],
            b"",
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "service",
            "20150830T123600Z",
        );
        assert!(
            auth.ends_with(
                "Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
            ),
            "unexpected authorization: {auth}"
        );
        assert!(auth.contains("SignedHeaders=host;x-amz-date"));
        assert!(auth.contains("Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request"));
    }

    #[tokio::test]
    async fn sqs_requires_credentials() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s1".into(),
            node_type: NodeType::Sqs,
            config: Some(serde_json::json!({"operation":"send","queue_url":"https://q"})),
        };
        let r = execute_sqs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_key_id"));
    }

    #[tokio::test]
    async fn sqs_send_requires_message_body() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s2".into(),
            node_type: NodeType::Sqs,
            config: Some(serde_json::json!({
                "access_key_id":"AK","secret_access_key":"sk",
                "queue_url":"https://q","operation":"send"
            })),
        };
        let r = execute_sqs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("message_body"));
    }

    #[tokio::test]
    async fn sns_publish_requires_message() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "n1".into(),
            node_type: NodeType::Sns,
            config: Some(serde_json::json!({
                "access_key_id":"AK","secret_access_key":"sk",
                "operation":"publish","topic_arn":"arn:aws:sns:…"
            })),
        };
        let r = execute_sns(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("message"));
    }

    #[tokio::test]
    async fn sqs_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s3".into(),
            node_type: NodeType::Sqs,
            config: Some(serde_json::json!({
                "access_key_id":"AK","secret_access_key":"sk",
                "queue_url":"https://q","operation":"nope"
            })),
        };
        let r = execute_sqs(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    #[tokio::test]
    async fn bedrock_requires_credentials() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "b1".into(),
            node_type: NodeType::Bedrock,
            config: Some(serde_json::json!({"model_id":"anthropic.claude","body":{}})),
        };
        let r = execute_bedrock(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_key_id"));
    }

    #[tokio::test]
    async fn bedrock_requires_body() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "b2".into(),
            node_type: NodeType::Bedrock,
            config: Some(serde_json::json!({
                "access_key_id":"AK","secret_access_key":"sk",
                "model_id":"anthropic.claude-3-5-sonnet-20240620-v1:0"
            })),
        };
        let r = execute_bedrock(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("body"));
    }
}
