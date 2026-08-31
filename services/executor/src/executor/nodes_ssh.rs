// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! SSH (command exec) and SFTP nodes built on russh / russh-sftp — a pure-Rust
//! SSH implementation, so the workspace needs no libssh2/system library to
//! build. Password authentication; binary SFTP payloads cross as base64.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use russh::client;
use russh::keys::ssh_key::Fingerprint;
use std::sync::Arc;
use workflow_core::Node;

struct PinnedHostKey {
    expected: Fingerprint,
}

impl client::Handler for PinnedHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let observed = match server_public_key {
            russh::keys::PublicKeyOrCertificate::PublicKey { key, .. } => {
                key.fingerprint(russh::keys::HashAlg::Sha256)
            }
            russh::keys::PublicKeyOrCertificate::Certificate(certificate) => {
                Fingerprint::new(russh::keys::HashAlg::Sha256, certificate.public_key())
            }
        };
        Ok(fingerprints_match(&self.expected, &observed))
    }
}

fn parse_host_key_fingerprint(value: &str) -> Result<Fingerprint, String> {
    let fingerprint = value
        .trim()
        .parse::<Fingerprint>()
        .map_err(|_| "host_key_fingerprint must be an OpenSSH SHA-256 fingerprint".to_string())?;
    if !matches!(fingerprint, Fingerprint::Sha256(_)) {
        return Err("host_key_fingerprint must use SHA-256".to_string());
    }
    Ok(fingerprint)
}

fn fingerprints_match(expected: &Fingerprint, observed: &Fingerprint) -> bool {
    expected == observed
}

struct Conn {
    host: String,
    port: u16,
    user: String,
    pass: String,
    private_key: Option<String>,
    passphrase: Option<String>,
    host_key_fingerprint: Fingerprint,
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
    let opt = |k: &str| {
        cfg.get(k)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };
    let port = match cfg.get("port") {
        None | Some(serde_json::Value::Null) => 22,
        Some(value) => match value.as_u64() {
            Some(port) if (1..=u16::MAX as u64).contains(&port) => port,
            _ => {
                return Err(NodeExecutionResult::failed(format!(
                    "{node} 'port' must be an integer between 1 and 65535"
                )))
            }
        },
    };
    let host_key_fingerprint = match cfg
        .get("host_key_fingerprint")
        .and_then(|value| value.as_str())
    {
        Some(value) if !value.trim().is_empty() => match parse_host_key_fingerprint(value) {
            Ok(fingerprint) => fingerprint,
            Err(error) => return Err(NodeExecutionResult::failed(format!("{node} {error}"))),
        },
        _ => {
            return Err(NodeExecutionResult::failed(format!(
                "{node} requires 'host_key_fingerprint'"
            )))
        }
    };
    Ok(Conn {
        host,
        port: port as u16,
        user,
        pass: cfg
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        private_key: opt("private_key"),
        passphrase: opt("passphrase"),
        host_key_fingerprint,
    })
}

async fn connect(conn: &Conn) -> Result<client::Handle<PinnedHostKey>, String> {
    // Decode the key (if any) up front so a malformed key fails before any socket.
    let keypair = match &conn.private_key {
        Some(pem) => Some(
            russh::keys::decode_secret_key(pem, conn.passphrase.as_deref())
                .map_err(|e| format!("private key error: {e}"))?,
        ),
        None => None,
    };

    let config = Arc::new(client::Config::default());
    let handler = PinnedHostKey {
        expected: conn.host_key_fingerprint,
    };
    let mut handle = client::connect(config, (conn.host.as_str(), conn.port), handler)
        .await
        .map_err(|e| format!("connect or host-key verification error: {e}"))?;

    let authed = match keypair {
        // Public-key auth when a private key is supplied, else password.
        Some(key) => {
            let hash_algorithm = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|e| format!("RSA negotiation error: {e}"))?
                .flatten();
            handle
                .authenticate_publickey(
                    conn.user.clone(),
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), hash_algorithm),
                )
                .await
                .map_err(|e| format!("auth error: {e}"))?
        }
        None => handle
            .authenticate_password(conn.user.clone(), conn.pass.clone())
            .await
            .map_err(|e| format!("auth error: {e}"))?,
    };
    if !authed.success() {
        return Err(format!(
            "authentication failed ({})",
            if conn.private_key.is_some() {
                "public key"
            } else {
                "password"
            }
        ));
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

    const HOST_FP: &str = "SHA256:JQ6FV0rf7qqJHZqIj4zNH8eV0oB8KLKh9Pph3FTD98g";

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
            config: Some(serde_json::json!({
                "host":"h","username":"u","host_key_fingerprint":HOST_FP
            })),
        };
        let r = execute_ssh(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("command"));
    }

    #[tokio::test]
    async fn ssh_rejects_malformed_private_key() {
        // A bad key is rejected before any socket is opened.
        let n = Node {
            id: "s3".into(),
            node_type: NodeType::Ssh,
            config: Some(serde_json::json!({
                "host":"192.0.2.1","username":"u","command":"ls",
                "host_key_fingerprint":HOST_FP,
                "private_key":"not-a-real-key"
            })),
        };
        let r = execute_ssh(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("private key"));
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

    #[tokio::test]
    async fn ssh_requires_host_key_fingerprint_before_connecting() {
        let n = Node {
            id: "s4".into(),
            node_type: NodeType::Ssh,
            config: Some(serde_json::json!({
                "host":"192.0.2.1","username":"u","command":"ls"
            })),
        };
        let r = execute_ssh(&n, &ctx()).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("host_key_fingerprint"));
    }

    #[test]
    fn host_key_fingerprint_requires_sha256() {
        let sha512 = format!("SHA512:{}", "A".repeat(86));
        assert!(parse_host_key_fingerprint(&sha512)
            .unwrap_err()
            .contains("SHA-256"));
        assert!(parse_host_key_fingerprint("not-a-fingerprint").is_err());
    }

    #[test]
    fn host_key_fingerprint_matches_only_exact_digest() {
        let expected = parse_host_key_fingerprint(HOST_FP).unwrap();
        let same = parse_host_key_fingerprint(HOST_FP).unwrap();
        let different =
            parse_host_key_fingerprint("SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
                .unwrap();
        assert!(fingerprints_match(&expected, &same));
        assert!(!fingerprints_match(&expected, &different));
    }

    #[test]
    fn ssh_rejects_out_of_range_port() {
        for port in [
            serde_json::json!(0),
            serde_json::json!(65536),
            serde_json::json!(-1),
            serde_json::json!("22"),
        ] {
            let result = read_conn(
                &serde_json::json!({
                    "host":"h","username":"u","port":port,
                    "host_key_fingerprint":HOST_FP
                }),
                "SSH",
            );
            match result {
                Err(error) => assert!(error.error.as_deref().unwrap_or("").contains("65535")),
                Ok(_) => panic!("invalid port was accepted"),
            }
        }
    }
}
