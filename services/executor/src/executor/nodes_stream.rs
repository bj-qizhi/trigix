// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Streaming (SSE) chat for the direct LLM providers. Emits token deltas
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

/// Read an SSE response line by line, invoking `on_data` with each non-empty
/// `data:` JSON payload (skipping `[DONE]`). All three providers below frame
/// their stream as `data: {json}\n\n`, so the per-provider difference is only in
/// how each payload is interpreted.
async fn read_sse<F: FnMut(&str)>(
    resp: reqwest::Response,
    node_name: &str,
    mut on_data: F,
) -> Result<(), NodeExecutionResult> {
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk
            .map_err(|e| NodeExecutionResult::failed(format!("{node_name} stream error: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(nl) = buf.find('\n') {
            let line: String = buf.drain(..=nl).collect();
            if let Some(d) = line.trim().strip_prefix("data:") {
                let d = d.trim();
                if !d.is_empty() && d != "[DONE]" {
                    on_data(d);
                }
            }
        }
    }
    Ok(())
}

async fn check_ok(
    resp: reqwest::Response,
    node_name: &str,
) -> Result<reqwest::Response, NodeExecutionResult> {
    if resp.status().is_success() {
        Ok(resp)
    } else {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(NodeExecutionResult::failed(format!(
            "{node_name} API {status}: {body}"
        )))
    }
}

/// Stream an OpenAI-compatible chat completion (Bearer auth). Returns the
/// assembled `(content, usage)`.
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
    let resp = check_ok(resp, node_name).await?;

    let mut content = String::new();
    let mut usage = serde_json::Value::Null;
    read_sse(resp, node_name, |d| {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(d) {
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
    })
    .await?;
    Ok((content, usage))
}

/// Stream Gemini's `streamGenerateContent` (key is already in `url`, no auth
/// header). Deltas live at `candidates[0].content.parts[0].text`.
pub(super) async fn stream_gemini_chat(
    http_client: &reqwest::Client,
    url: &str,
    node_id: &str,
    node_name: &str,
    payload: serde_json::Value,
) -> Result<(String, serde_json::Value), NodeExecutionResult> {
    let resp = http_client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| NodeExecutionResult::failed(format!("{node_name} request error: {e}")))?;
    let resp = check_ok(resp, node_name).await?;

    let mut content = String::new();
    let mut usage = serde_json::Value::Null;
    read_sse(resp, node_name, |d| {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(d) {
            if let Some(delta) = v["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                if !delta.is_empty() {
                    content.push_str(delta);
                    emit_token(node_id, delta);
                }
            }
            if v.get("usageMetadata")
                .map(|u| !u.is_null())
                .unwrap_or(false)
            {
                usage = v["usageMetadata"].clone();
            }
        }
    })
    .await?;
    Ok((content, usage))
}

/// Stream Anthropic's messages API (x-api-key). Text deltas arrive as
/// `content_block_delta` events; usage is assembled from message_start/_delta.
pub(super) async fn stream_claude_chat(
    http_client: &reqwest::Client,
    url: &str,
    api_key: &str,
    node_id: &str,
    node_name: &str,
    mut payload: serde_json::Value,
) -> Result<(String, serde_json::Value), NodeExecutionResult> {
    payload["stream"] = serde_json::json!(true);
    let resp = http_client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| NodeExecutionResult::failed(format!("{node_name} request error: {e}")))?;
    let resp = check_ok(resp, node_name).await?;

    let mut content = String::new();
    let mut usage = serde_json::json!({});
    read_sse(resp, node_name, |d| {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(d) {
            match v["type"].as_str() {
                Some("content_block_delta") => {
                    if let Some(delta) = v["delta"]["text"].as_str() {
                        if !delta.is_empty() {
                            content.push_str(delta);
                            emit_token(node_id, delta);
                        }
                    }
                }
                Some("message_start") => {
                    if let Some(u) = v["message"]["usage"].as_object() {
                        for (k, val) in u {
                            usage[k.as_str()] = val.clone();
                        }
                    }
                }
                Some("message_delta") => {
                    if let Some(out) = v["usage"]["output_tokens"].as_u64() {
                        usage["output_tokens"] = serde_json::json!(out);
                    }
                }
                _ => {}
            }
        }
    })
    .await?;
    Ok((content, usage))
}
