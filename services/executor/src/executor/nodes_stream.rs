// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Streaming (SSE) chat for OpenAI-compatible endpoints. Emits token deltas
//! through execution_core's task-local sink while accumulating the full text,
//! so a node returns the *same* final output_json whether or not anyone is
//! listening — streaming is a transparent overlay, not a different result.

use crate::runtime::NodeExecutionResult;
use execution_core::emit_token;
use futures::StreamExt;

/// True when a live token sink is installed for the current task (i.e. someone
/// is watching this execution). Falls back to the non-streaming path otherwise.
pub(super) fn streaming_enabled() -> bool {
    execution_core::TOKEN_SINK
        .try_with(|s| s.is_some())
        .unwrap_or(false)
}

/// Stream an OpenAI-compatible chat completion. `payload` is the ordinary
/// (non-streamed) request body; this flips on `stream`, emits each content
/// delta to the sink for `node_id`, and returns the assembled `(content,
/// usage)`. Any HTTP/stream error becomes a failed `NodeExecutionResult`.
pub(super) async fn stream_openai_chat(
    http_client: &reqwest::Client,
    url: &str,
    api_key: &str,
    node_id: &str,
    node_name: &str,
    mut payload: serde_json::Value,
) -> Result<(String, serde_json::Value), NodeExecutionResult> {
    payload["stream"] = serde_json::json!(true);
    if let Some(o) = payload.as_object_mut() {
        o.entry("stream_options")
            .or_insert_with(|| serde_json::json!({ "include_usage": true }));
    }
    let resp = http_client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| NodeExecutionResult::failed(format!("{node_name} request error: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(NodeExecutionResult::failed(format!(
            "{node_name} API {}: {}",
            status.as_u16(),
            body
        )));
    }

    let mut content = String::new();
    let mut usage = serde_json::Value::Null;
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk
            .map_err(|e| NodeExecutionResult::failed(format!("{node_name} stream error: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        // Process each complete line. OpenAI SSE frames are `data: {json}\n\n`.
        while let Some(nl) = buf.find('\n') {
            let line: String = buf.drain(..=nl).collect();
            let data = match line.trim().strip_prefix("data:") {
                Some(d) => d.trim().to_string(),
                None => continue,
            };
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                    if !delta.is_empty() {
                        content.push_str(delta);
                        emit_token(node_id, delta);
                    }
                }
                if v.get("usage").map(|u| !u.is_null()).unwrap_or(false) {
                    usage = v["usage"].clone();
                }
            }
        }
    }
    Ok((content, usage))
}
