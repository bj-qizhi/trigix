// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! IMAP node (imap crate over native-tls). Blocking client driven on a blocking
//! thread. Reads mailbox listings and recent message envelopes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

fn bytes_to_string(b: Option<&[u8]>) -> String {
    b.map(|s| String::from_utf8_lossy(s).to_string())
        .unwrap_or_default()
}

pub(super) async fn execute_imap(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return NodeExecutionResult::failed("IMAP requires 'host'"),
    };
    let port = cfg.get("port").and_then(|v| v.as_u64()).unwrap_or(993) as u16;
    let user = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("IMAP requires 'username'"),
    };
    let pass = cfg
        .get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mailbox = cfg
        .get("mailbox")
        .and_then(|v| v.as_str())
        .unwrap_or("INBOX")
        .to_string();
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_messages")
        .to_string();
    let limit = cfg
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(10)
        .max(1);

    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| format!("TLS init error: {e}"))?;
        let client = imap::connect((host.as_str(), port), host.as_str(), &tls)
            .map_err(|e| format!("connect error: {e}"))?;
        let mut session = client
            .login(&user, &pass)
            .map_err(|e| format!("login error: {e:?}"))?;

        let out = match operation.as_str() {
            "list_mailboxes" => {
                let names = session
                    .list(None, Some("*"))
                    .map_err(|e| format!("list error: {e}"))?;
                let mailboxes: Vec<String> = names.iter().map(|n| n.name().to_string()).collect();
                serde_json::json!({ "mailboxes": mailboxes, "count": mailboxes.len() })
            }
            "list_messages" => {
                let mb = session
                    .select(&mailbox)
                    .map_err(|e| format!("select error: {e}"))?;
                let exists = mb.exists;
                if exists == 0 {
                    serde_json::json!({ "messages": [], "count": 0, "exists": 0 })
                } else {
                    let start = exists.saturating_sub(limit as u32 - 1).max(1);
                    let range = format!("{start}:{exists}");
                    let fetches = session
                        .fetch(&range, "(ENVELOPE INTERNALDATE FLAGS)")
                        .map_err(|e| format!("fetch error: {e}"))?;
                    let mut messages: Vec<serde_json::Value> = fetches
                        .iter()
                        .map(|f| {
                            let env = f.envelope();
                            let subject =
                                env.map(|e| bytes_to_string(e.subject)).unwrap_or_default();
                            let date = env.map(|e| bytes_to_string(e.date)).unwrap_or_default();
                            let from = env
                                .and_then(|e| e.from.as_ref())
                                .and_then(|addrs| addrs.first())
                                .map(|a| {
                                    format!(
                                        "{}@{}",
                                        bytes_to_string(a.mailbox),
                                        bytes_to_string(a.host)
                                    )
                                })
                                .unwrap_or_default();
                            serde_json::json!({
                                "seq": f.message,
                                "subject": subject,
                                "from": from,
                                "date": date,
                            })
                        })
                        .collect();
                    messages.reverse(); // newest first
                    serde_json::json!({
                        "messages": messages,
                        "count": messages.len(),
                        "exists": exists,
                    })
                }
            }
            other => return Err(format!("unknown operation '{other}'")),
        };
        let _ = session.logout();
        Ok(out)
    })
    .await;

    match result {
        Ok(Ok(v)) => NodeExecutionResult::succeeded(v.to_string()),
        Ok(Err(e)) => NodeExecutionResult::failed(format!("IMAP {e}")),
        Err(e) => NodeExecutionResult::failed(format!("IMAP task error: {e}")),
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
    async fn imap_requires_host() {
        let n = Node {
            id: "i1".into(),
            node_type: NodeType::Imap,
            config: Some(serde_json::json!({"username":"u","password":"p"})),
        };
        let r = execute_imap(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn imap_requires_username() {
        let n = Node {
            id: "i2".into(),
            node_type: NodeType::Imap,
            config: Some(serde_json::json!({"host":"imap.example.com"})),
        };
        let r = execute_imap(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("username"));
    }
}
