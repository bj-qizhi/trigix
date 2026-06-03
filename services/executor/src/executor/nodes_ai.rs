// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Western LLM nodes (OpenAI, Gemini, Claude).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

pub(super) async fn execute_openai(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("OpenAI node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("OpenAI node missing 'api_key'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-4o-mini")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("OpenAI node missing 'prompt_template'"),
    };
    let system_prompt = config
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_default();
    let max_tokens: u64 = config
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024);
    let temperature: f64 = config
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    let mut messages = Vec::new();
    if !system_prompt.is_empty() {
        messages.push(serde_json::json!({ "role": "system", "content": system_prompt }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": prompt }));

    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    let resp = match http_client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("OpenAI request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("OpenAI API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("OpenAI parse error: {e}")),
    };

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let usage = parsed
        .get("usage")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
}

pub(super) async fn execute_gemini(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Gemini node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Gemini node missing 'api_key'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gemini-2.0-flash")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Gemini node missing 'prompt_template'"),
    };
    let system_prompt = config
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_default();
    let max_tokens: u64 = config
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024);
    let temperature: f64 = config
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let mut payload = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "maxOutputTokens": max_tokens, "temperature": temperature }
    });
    if !system_prompt.is_empty() {
        payload["systemInstruction"] = serde_json::json!({ "parts": [{ "text": system_prompt }] });
    }

    let resp = match http_client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Gemini request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("Gemini API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Gemini parse error: {e}")),
    };

    let content = parsed["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let usage = parsed
        .get("usageMetadata")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
}

pub(super) async fn execute_claude(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Claude node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Claude node missing 'api_key'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("claude-sonnet-4-6")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Claude node missing 'prompt_template'"),
    };
    let system_prompt = config
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_default();
    let max_tokens: u64 = config
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024);

    let mut payload = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{ "role": "user", "content": prompt }],
    });
    if !system_prompt.is_empty() {
        payload["system"] = serde_json::json!(system_prompt);
    }

    let resp = match http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Claude request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("Claude API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Claude parse error: {e}")),
    };

    let content = parsed["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let usage = parsed
        .get("usage")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
}
