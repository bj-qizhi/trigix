// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Object-storage nodes over HTTP: Google Cloud Storage (JSON API, caller
//! supplies an OAuth2 access token) and Azure Blob Storage (REST, caller
//! supplies a SAS token). Both return `{status, body}`.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── Google Cloud Storage (JSON API) ───────────────────────────────────────────
pub(super) async fn execute_gcs(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "GCS requires 'access_token' (OAuth2 bearer token for the storage scope)",
            )
        }
    };
    let bucket = match cfg.get("bucket").and_then(|v| v.as_str()) {
        Some(b) if !b.is_empty() => b.to_string(),
        _ => return NodeExecutionResult::failed("GCS requires 'bucket'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();
    let object = || -> Result<String, NodeExecutionResult> {
        match cfg.get("object").and_then(|v| v.as_str()) {
            Some(o) if !o.is_empty() => Ok(o.to_string()),
            _ => Err(NodeExecutionResult::failed(format!(
                "GCS {operation} requires 'object'"
            ))),
        }
    };
    let bearer = format!("Bearer {access_token}");

    // Text responses (download) and JSON responses (metadata/list) both wrapped
    // into {status, body}; body parses to JSON when possible, else raw string.
    let finish = |op: &'static str| {
        move |resp: Result<reqwest::Response, reqwest::Error>| async move {
            match resp {
                Ok(r) => {
                    let status = r.status().as_u16();
                    let text = r.text().await.unwrap_or_default();
                    let body = serde_json::from_str::<serde_json::Value>(&text)
                        .unwrap_or(serde_json::Value::String(text));
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("GCS {op} error: {e}")),
            }
        }
    };

    match operation.as_str() {
        "list" => {
            let mut url = format!("https://storage.googleapis.com/storage/v1/b/{bucket}/o");
            if let Some(prefix) = cfg.get("prefix").and_then(|v| v.as_str()) {
                if !prefix.is_empty() {
                    url.push_str(&format!("?prefix={}", urlencoding::encode(prefix)));
                }
            }
            finish("list")(
                http_client
                    .get(&url)
                    .header("Authorization", &bearer)
                    .send()
                    .await,
            )
            .await
        }
        "get" => {
            let obj = match object() {
                Ok(o) => o,
                Err(e) => return e,
            };
            let url = format!(
                "https://storage.googleapis.com/storage/v1/b/{bucket}/o/{}",
                urlencoding::encode(&obj)
            );
            finish("get")(
                http_client
                    .get(&url)
                    .header("Authorization", &bearer)
                    .send()
                    .await,
            )
            .await
        }
        "download" => {
            let obj = match object() {
                Ok(o) => o,
                Err(e) => return e,
            };
            let url = format!(
                "https://storage.googleapis.com/storage/v1/b/{bucket}/o/{}?alt=media",
                urlencoding::encode(&obj)
            );
            finish("download")(
                http_client
                    .get(&url)
                    .header("Authorization", &bearer)
                    .send()
                    .await,
            )
            .await
        }
        "upload" => {
            let obj = match object() {
                Ok(o) => o,
                Err(e) => return e,
            };
            let content = cfg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content_type = cfg
                .get("content_type")
                .and_then(|v| v.as_str())
                .unwrap_or("text/plain")
                .to_string();
            let url = format!(
                "https://storage.googleapis.com/upload/storage/v1/b/{bucket}/o?uploadType=media&name={}",
                urlencoding::encode(&obj)
            );
            finish("upload")(
                http_client
                    .post(&url)
                    .header("Authorization", &bearer)
                    .header("Content-Type", content_type)
                    .body(content)
                    .send()
                    .await,
            )
            .await
        }
        "delete" => {
            let obj = match object() {
                Ok(o) => o,
                Err(e) => return e,
            };
            let url = format!(
                "https://storage.googleapis.com/storage/v1/b/{bucket}/o/{}",
                urlencoding::encode(&obj)
            );
            finish("delete")(
                http_client
                    .delete(&url)
                    .header("Authorization", &bearer)
                    .send()
                    .await,
            )
            .await
        }
        other => NodeExecutionResult::failed(format!("GCS unknown operation '{other}'")),
    }
}

// ── Azure Blob Storage (REST + SAS) ───────────────────────────────────────────
pub(super) async fn execute_azure_blob(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let account = match cfg.get("account").and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return NodeExecutionResult::failed("Azure Blob requires 'account' (storage account)"),
    };
    let container = match cfg.get("container").and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => return NodeExecutionResult::failed("Azure Blob requires 'container'"),
    };
    let sas = match cfg.get("sas_token").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.trim_start_matches('?').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Azure Blob requires 'sas_token' (shared access signature)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();
    let base = format!("https://{account}.blob.core.windows.net/{container}");
    let blob = || -> Result<String, NodeExecutionResult> {
        match cfg.get("blob").and_then(|v| v.as_str()) {
            Some(b) if !b.is_empty() => Ok(b.to_string()),
            _ => Err(NodeExecutionResult::failed(format!(
                "Azure Blob {operation} requires 'blob' (blob name)"
            ))),
        }
    };

    let finish = |op: &'static str| {
        move |resp: Result<reqwest::Response, reqwest::Error>| async move {
            match resp {
                Ok(r) => {
                    let status = r.status().as_u16();
                    let text = r.text().await.unwrap_or_default();
                    let body = serde_json::from_str::<serde_json::Value>(&text)
                        .unwrap_or(serde_json::Value::String(text));
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Azure Blob {op} error: {e}")),
            }
        }
    };

    match operation.as_str() {
        "list" => {
            let url = format!("{base}?restype=container&comp=list&{sas}");
            finish("list")(http_client.get(&url).send().await).await
        }
        "get" => {
            let name = match blob() {
                Ok(b) => b,
                Err(e) => return e,
            };
            let url = format!("{base}/{}?{sas}", urlencoding::encode(&name));
            finish("get")(http_client.get(&url).send().await).await
        }
        "put" => {
            let name = match blob() {
                Ok(b) => b,
                Err(e) => return e,
            };
            let content = cfg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content_type = cfg
                .get("content_type")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string();
            let url = format!("{base}/{}?{sas}", urlencoding::encode(&name));
            finish("put")(
                http_client
                    .put(&url)
                    .header("x-ms-blob-type", "BlockBlob")
                    .header("Content-Type", content_type)
                    .body(content)
                    .send()
                    .await,
            )
            .await
        }
        "delete" => {
            let name = match blob() {
                Ok(b) => b,
                Err(e) => return e,
            };
            let url = format!("{base}/{}?{sas}", urlencoding::encode(&name));
            finish("delete")(http_client.delete(&url).send().await).await
        }
        other => NodeExecutionResult::failed(format!("Azure Blob unknown operation '{other}'")),
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
    async fn gcs_requires_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g1".into(),
            node_type: NodeType::Gcs,
            config: Some(serde_json::json!({"bucket":"b","operation":"list"})),
        };
        let r = execute_gcs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn gcs_get_requires_object() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g2".into(),
            node_type: NodeType::Gcs,
            config: Some(serde_json::json!({"access_token":"t","bucket":"b","operation":"get"})),
        };
        let r = execute_gcs(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("object"));
    }

    #[tokio::test]
    async fn gcs_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g3".into(),
            node_type: NodeType::Gcs,
            config: Some(serde_json::json!({"access_token":"t","bucket":"b","operation":"nope"})),
        };
        let r = execute_gcs(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    #[tokio::test]
    async fn azure_blob_requires_sas() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a1".into(),
            node_type: NodeType::AzureBlob,
            config: Some(serde_json::json!({"account":"acct","container":"c","operation":"list"})),
        };
        let r = execute_azure_blob(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("sas_token"));
    }

    #[tokio::test]
    async fn azure_blob_get_requires_blob() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a2".into(),
            node_type: NodeType::AzureBlob,
            config: Some(serde_json::json!({
                "account":"acct","container":"c","sas_token":"sv=…","operation":"get"
            })),
        };
        let r = execute_azure_blob(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("blob"));
    }
}
