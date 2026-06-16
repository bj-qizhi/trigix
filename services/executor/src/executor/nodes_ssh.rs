// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! SSH (command exec) and SFTP nodes built on russh / russh-sftp — a pure-Rust
//! SSH implementation, so the workspace needs no libssh2/system library to
//! build. Password authentication; binary SFTP payloads cross as base64.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use russh::client;
use std::sync::Arc;
use workflow_core::Node;

// Accept any host key (workflow nodes connect to user-configured hosts; a known-
// hosts policy would be a future enhancement).
struct AcceptAll;

#[async_trait::async_trait]
impl client::Handler for AcceptAll {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

struct Conn {
    host: String,
    port: u16,
    user: String,
    pass: String,
}

fn read_conn(cfg: &serde_json::Value, node: &str) -> Result<Conn, NodeExecutionResult> {
    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => {
            return Err(NodeExecutionResult::failed(format!(
                "{node} requires 'host'"
            )))
        }
    };
    let user = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => {
            return Err(NodeExecutionResult::failed(format!(
                "{node} requires 'username'"
            )))
        }
    };
    Ok(Conn {
        host,
        port: cfg.get("port").and_then(|v| v.as_u64()).unwrap_or(22) as u16,
        user,
        pass: cfg
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

async fn connect(conn: &Conn) -> Result<client::Handle<AcceptAll>, String> {
    let config = Arc::new(client::Config::default());
    let mut handle = client::connect(config, (conn.host.as_str(), conn.port), AcceptAll)
        .await
        .map_err(|e| format!("connect error: {e}"))?;
    let authed = handle
        .authenticate_password(conn.user.clone(), conn.pass.clone())
        .await
        .map_err(|e| format!("auth error: {e}"))?;
    if !authed {
        return Err("authentication failed (password)".into());
    }
    Ok(handle)
}

// ── SSH exec ──────────────────────────────────────────────────────────────────
pub(super) async fn execute_ssh(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let conn = match read_conn(&cfg, "SSH") {
        Ok(c) => c,
        Err(e) => return e,
    };
    let command = match cfg.get("command").and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => return NodeExecutionResult::failed("SSH requires 'command'"),
    };

    let handle = match connect(&conn).await {
        Ok(h) => h,
        Err(e) => return NodeExecutionResult::failed(format!("SSH {e}")),
    };
    let mut channel = match handle.channel_open_session().await {
        Ok(c) => c,
        Err(e) => return NodeExecutionResult::failed(format!("SSH channel error: {e}")),
    };
    if let Err(e) = channel.exec(true, command.as_bytes()).await {
        return NodeExecutionResult::failed(format!("SSH exec error: {e}"));
    }

    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let mut exit_status: u32 = 0;
    while let Some(msg) = channel.wait().await {
        match msg {
            russh::ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
            russh::ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
            russh::ChannelMsg::ExitStatus { exit_status: code } => exit_status = code,
            russh::ChannelMsg::Eof | russh::ChannelMsg::Close => {}
            _ => {}
        }
    }

    NodeExecutionResult::succeeded(
        serde_json::json!({
            "stdout": String::from_utf8_lossy(&stdout),
            "stderr": String::from_utf8_lossy(&stderr),
            "exit_status": exit_status,
        })
        .to_string(),
    )
}

// ── SFTP ──────────────────────────────────────────────────────────────────────
pub(super) async fn execute_sftp(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let conn = match read_conn(&cfg, "SFTP") {
        Ok(c) => c,
        Err(e) => return e,
    };
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

    let handle = match connect(&conn).await {
        Ok(h) => h,
        Err(e) => return NodeExecutionResult::failed(format!("SFTP {e}")),
    };
    let channel = match handle.channel_open_session().await {
        Ok(c) => c,
        Err(e) => return NodeExecutionResult::failed(format!("SFTP channel error: {e}")),
    };
    if let Err(e) = channel.request_subsystem(true, "sftp").await {
        return NodeExecutionResult::failed(format!("SFTP subsystem error: {e}"));
    }
    let sftp = match russh_sftp::client::SftpSession::new(channel.into_stream()).await {
        Ok(s) => s,
        Err(e) => return NodeExecutionResult::failed(format!("SFTP session error: {e}")),
    };

    let out = match operation.as_str() {
        "list" => {
            let dir = if path.is_empty() {
                ".".to_string()
            } else {
                path.clone()
            };
            match sftp.read_dir(dir).await {
                Ok(entries) => {
                    let files: Vec<String> = entries.map(|e| e.file_name()).collect();
                    serde_json::json!({ "files": files, "count": files.len() })
                }
                Err(e) => return NodeExecutionResult::failed(format!("SFTP list error: {e}")),
            }
        }
        "download" => {
            if path.is_empty() {
                return NodeExecutionResult::failed("SFTP download requires 'path'");
            }
            match sftp.read(path).await {
                Ok(bytes) => serde_json::json!({
                    "content_base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
                    "size": bytes.len(),
                }),
                Err(e) => return NodeExecutionResult::failed(format!("SFTP download error: {e}")),
            }
        }
        "upload" => {
            if path.is_empty() {
                return NodeExecutionResult::failed("SFTP upload requires 'path'");
            }
            let content_b64 = cfg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let bytes = match base64::engine::general_purpose::STANDARD.decode(content_b64.trim()) {
                Ok(b) => b,
                Err(e) => {
                    return NodeExecutionResult::failed(format!(
                        "SFTP 'content' is not valid base64: {e}"
                    ))
                }
            };
            match sftp.write(path, &bytes).await {
                Ok(()) => serde_json::json!({ "uploaded": true, "size": bytes.len() }),
                Err(e) => return NodeExecutionResult::failed(format!("SFTP upload error: {e}")),
            }
        }
        "delete" => {
            if path.is_empty() {
                return NodeExecutionResult::failed("SFTP delete requires 'path'");
            }
            match sftp.remove_file(path).await {
                Ok(()) => serde_json::json!({ "deleted": true }),
                Err(e) => return NodeExecutionResult::failed(format!("SFTP delete error: {e}")),
            }
        }
        other => return NodeExecutionResult::failed(format!("SFTP unknown operation '{other}'")),
    };
    let _ = sftp.close().await;
    NodeExecutionResult::succeeded(out.to_string())
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
    async fn ssh_requires_host() {
        let n = Node {
            id: "s1".into(),
            node_type: NodeType::Ssh,
            config: Some(serde_json::json!({"username":"u","command":"ls"})),
        };
        let r = execute_ssh(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn ssh_requires_command() {
        let n = Node {
            id: "s2".into(),
            node_type: NodeType::Ssh,
            config: Some(serde_json::json!({"host":"h","username":"u"})),
        };
        let r = execute_ssh(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("command"));
    }

    #[tokio::test]
    async fn sftp_requires_username() {
        let n = Node {
            id: "sf1".into(),
            node_type: NodeType::Sftp,
            config: Some(serde_json::json!({"host":"h","operation":"list"})),
        };
        let r = execute_sftp(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("username"));
    }
}
