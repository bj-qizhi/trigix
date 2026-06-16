// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! FTP node (suppaftp, plain FTP). Blocking client driven on a blocking thread
//! so it never stalls the async runtime. Binary payloads cross as base64.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use std::io::Cursor;
use suppaftp::FtpStream;
use workflow_core::Node;

pub(super) async fn execute_ftp(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return NodeExecutionResult::failed("FTP requires 'host'"),
    };
    let port = cfg.get("port").and_then(|v| v.as_u64()).unwrap_or(21);
    let user = cfg
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("anonymous")
        .to_string();
    let pass = cfg
        .get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();
    let path = cfg
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let content_b64 = cfg
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let mut ftp = FtpStream::connect((host.as_str(), port as u16))
            .map_err(|e| format!("connect error: {e}"))?;
        ftp.login(&user, &pass)
            .map_err(|e| format!("login error: {e}"))?;

        let out = match operation.as_str() {
            "list" => {
                let dir = if path.is_empty() {
                    None
                } else {
                    Some(path.as_str())
                };
                let files = ftp.nlst(dir).map_err(|e| format!("list error: {e}"))?;
                serde_json::json!({ "files": files, "count": files.len() })
            }
            "download" => {
                if path.is_empty() {
                    return Err("download requires 'path'".into());
                }
                let cursor = ftp
                    .retr_as_buffer(&path)
                    .map_err(|e| format!("download error: {e}"))?;
                let bytes = cursor.into_inner();
                serde_json::json!({
                    "content_base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
                    "size": bytes.len(),
                })
            }
            "upload" => {
                if path.is_empty() {
                    return Err("upload requires 'path'".into());
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(content_b64.trim())
                    .map_err(|e| format!("'content' is not valid base64: {e}"))?;
                let mut reader = Cursor::new(bytes);
                let n = ftp
                    .put_file(&path, &mut reader)
                    .map_err(|e| format!("upload error: {e}"))?;
                serde_json::json!({ "uploaded": true, "size": n })
            }
            "delete" => {
                if path.is_empty() {
                    return Err("delete requires 'path'".into());
                }
                ftp.rm(&path).map_err(|e| format!("delete error: {e}"))?;
                serde_json::json!({ "deleted": true })
            }
            other => return Err(format!("unknown operation '{other}'")),
        };
        let _ = ftp.quit();
        Ok(out)
    })
    .await;

    match result {
        Ok(Ok(v)) => NodeExecutionResult::succeeded(v.to_string()),
        Ok(Err(e)) => NodeExecutionResult::failed(format!("FTP {e}")),
        Err(e) => NodeExecutionResult::failed(format!("FTP task error: {e}")),
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
    async fn ftp_requires_host() {
        let n = Node {
            id: "f1".into(),
            node_type: NodeType::Ftp,
            config: Some(serde_json::json!({"operation":"list"})),
        };
        let r = execute_ftp(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }
}
