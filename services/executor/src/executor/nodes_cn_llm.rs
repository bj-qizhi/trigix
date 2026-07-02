// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Chinese-vendor LLM nodes (DeepSeek / Qwen / Zhipu / Moonshot / Doubao /
//! MiniMax / ERNIE / Hunyuan) and their shared OpenAI-compatible helpers.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── 国内大模型通用 OpenAI-compatible helper ──────────────────────────────
#[allow(clippy::too_many_arguments)]
async fn openai_compat_chat(
    node_name: &str,
    node_id: &str,
    api_key: &str,
    base_url: &str,
    model: &str,
    messages: serde_json::Value,
    max_tokens: u64,
    temperature: f64,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    // When someone is watching this execution, stream tokens live; the assembled
    // final content/usage is identical to the non-streamed path below.
    if super::nodes_stream::streaming_enabled() {
        return match super::nodes_stream::stream_openai_chat(
            http_client,
            base_url,
            api_key,
            node_id,
            node_name,
            payload,
        )
        .await
        {
            Ok((content, usage)) => NodeExecutionResult::succeeded(
                serde_json::json!({ "content": content, "model": model, "usage": usage })
                    .to_string(),
            ),
            Err(e) => e,
        };
    }

    // Retry transient failures (connect/timeout, 429, 5xx) with exponential
    // backoff — these OpenAI-compatible providers routinely rate-limit, and a
    // plain LLM node otherwise fails the whole run on a blip. Covers every
    // node that routes through this shared helper.
    let mut delay = std::time::Duration::from_millis(500);
    let mut attempt = 0u32;
    let resp = loop {
        attempt += 1;
        match http_client
            .post(base_url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(r) => {
                let s = r.status();
                if (s.as_u16() == 429 || s.is_server_error()) && attempt < 3 {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                break r;
            }
            Err(e) => {
                if (e.is_timeout() || e.is_connect()) && attempt < 3 {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                return NodeExecutionResult::failed(format!("{node_name} request error: {e}"));
            }
        }
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!(
            "{node_name} API {}: {}",
            status.as_u16(),
            body
        ));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("{node_name} parse error: {e}")),
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

fn extract_chat_fields(
    node_name: &str,
    config: &serde_json::Value,
    context: &ExecutionContext,
    default_model: &str,
) -> Result<(String, String, serde_json::Value, u64, f64), NodeExecutionResult> {
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => {
            return Err(NodeExecutionResult::failed(format!(
                "{node_name} missing 'api_key'"
            )))
        }
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(default_model)
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => {
            return Err(NodeExecutionResult::failed(format!(
                "{node_name} missing 'prompt_template'"
            )))
        }
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

    Ok((
        api_key,
        model,
        serde_json::Value::Array(messages),
        max_tokens,
        temperature,
    ))
}

// Optional per-node endpoint override. When `base_url` is set in the config the
// node talks to that OpenAI-compatible URL instead of the provider default —
// this unlocks newer models served on different endpoints (e.g. MiniMax M-series,
// ERNIE 4.5+/5.0 on Qianfan v2) without changing the request shape.
fn resolve_base_url(
    config: &serde_json::Value,
    context: &ExecutionContext,
    default_url: &str,
) -> String {
    config
        .get("base_url")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| resolve_template(s, context))
        .unwrap_or_else(|| default_url.to_string())
}

// ── Generic OpenAI-compatible chat ───────────────────────────────────────────

/// Default endpoint + model for a known `provider`. Empty strings mean "no
/// built-in default", so the node then requires `base_url` / `model` in config.
fn provider_defaults(provider: &str) -> (&'static str, &'static str) {
    match provider {
        "grok" => ("https://api.x.ai/v1/chat/completions", "grok-4.3"),
        "ollama" => ("http://localhost:11434/v1/chat/completions", ""),
        "deepseek" => (
            "https://api.deepseek.com/v1/chat/completions",
            "deepseek-v4-flash",
        ),
        "qwen" => (
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "qwen-max",
        ),
        "zhipu" => (
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "glm-4.6",
        ),
        "moonshot" => ("https://api.moonshot.cn/v1/chat/completions", "kimi-latest"),
        "doubao" => (
            "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
            "",
        ),
        "hunyuan" => (
            "https://api.hunyuan.cloud.tencent.com/v1/chat/completions",
            "hunyuan-turbos-latest",
        ),
        _ => ("", ""),
    }
}

/// One node for any OpenAI-compatible chat endpoint. `config.provider` picks a
/// built-in default base URL + model (the former per-vendor nodes); `base_url`
/// overrides for anything else. Streaming + retries come from openai_compat_chat.
pub(super) async fn execute_openai_compat(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("OpenAI-compatible node requires config"),
    };
    let provider = config
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let (default_url, default_model) = provider_defaults(&provider);
    let base_url = resolve_base_url(config, context, default_url);
    if base_url.is_empty() {
        return NodeExecutionResult::failed(
            "OpenAI-compatible node needs a 'base_url' or a known 'provider'",
        );
    }
    let name = if provider.is_empty() {
        "OpenAI-compatible".to_string()
    } else {
        provider.clone()
    };
    // Self-hosted gateways like Ollama accept any key; default one so the shared
    // chat helper (which always sends a bearer token) still works keyless.
    let mut effective = config.clone();
    let missing_key = config
        .get("api_key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .is_none();
    if provider == "ollama" && missing_key {
        if let Some(obj) = effective.as_object_mut() {
            obj.insert("api_key".to_string(), serde_json::json!("ollama"));
        }
    }
    let (api_key, model, messages, max_tokens, temperature) =
        match extract_chat_fields(&name, &effective, context, default_model) {
            Ok(v) => v,
            Err(e) => return e,
        };
    openai_compat_chat(
        &name,
        &node.id,
        &api_key,
        &base_url,
        &model,
        messages,
        max_tokens,
        temperature,
        http_client,
    )
    .await
}
pub(super) async fn execute_azure_openai(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Azure OpenAI node requires config"),
    };
    let field = |k: &str| {
        config
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| resolve_template(s, context))
    };
    let endpoint = match field("endpoint") {
        Some(e) if !e.is_empty() => e.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Azure OpenAI requires 'endpoint' (e.g. https://my-res.openai.azure.com)",
            )
        }
    };
    let deployment = match field("deployment") {
        Some(d) if !d.is_empty() => d,
        _ => return NodeExecutionResult::failed("Azure OpenAI requires 'deployment'"),
    };
    let api_key = match field("api_key") {
        Some(k) if !k.is_empty() => k,
        _ => return NodeExecutionResult::failed("Azure OpenAI requires 'api_key'"),
    };
    let api_version = field("api_version").unwrap_or_else(|| "2024-02-01".to_string());
    let prompt = match field("prompt_template") {
        Some(p) => p,
        None => return NodeExecutionResult::failed("Azure OpenAI missing 'prompt_template'"),
    };
    let system_prompt = field("system_prompt").unwrap_or_default();
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
    let url = format!(
        "{endpoint}/openai/deployments/{deployment}/chat/completions?api-version={api_version}"
    );
    let payload = serde_json::json!({
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    let resp = match http_client
        .post(&url)
        .header("api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Azure OpenAI request error: {e}")),
    };
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return NodeExecutionResult::failed(format!(
            "Azure OpenAI API {}: {}",
            status.as_u16(),
            body
        ));
    }
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Azure OpenAI parse error: {e}")),
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
        serde_json::json!({ "content": content, "model": deployment, "usage": usage }).to_string(),
    )
}

// ── Google Vertex AI (Gemini, generateContent) ────────────────────────────────
// Stays fully HTTP by taking a caller-supplied OAuth2 access token (same model
// as the GCS node) instead of signing a service-account JWT.
pub(super) async fn execute_vertex(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Vertex AI node requires config"),
    };
    let field = |k: &str| {
        config
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| resolve_template(s, context))
    };
    let access_token =
        match field("access_token") {
            Some(t) if !t.is_empty() => t,
            _ => return NodeExecutionResult::failed(
                "Vertex AI requires 'access_token' (OAuth2 bearer for the cloud-platform scope)",
            ),
        };
    let project = match field("project") {
        Some(p) if !p.is_empty() => p,
        _ => return NodeExecutionResult::failed("Vertex AI requires 'project'"),
    };
    let location = field("location")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "us-central1".to_string());
    let model = field("model")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "gemini-1.5-flash".to_string());
    let prompt = match field("prompt_template") {
        Some(p) if !p.is_empty() => p,
        _ => return NodeExecutionResult::failed("Vertex AI requires 'prompt_template'"),
    };
    let max_tokens = config
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024);
    let temperature = config
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    let mut payload = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "maxOutputTokens": max_tokens, "temperature": temperature },
    });
    if let Some(sys) = field("system_prompt").filter(|s| !s.is_empty()) {
        payload["systemInstruction"] = serde_json::json!({ "parts": [{ "text": sys }] });
    }

    let url = format!(
        "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:generateContent"
    );
    let resp = match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Vertex AI request error: {e}")),
    };
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return NodeExecutionResult::failed(format!("Vertex AI API {}: {}", status.as_u16(), body));
    }
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Vertex AI parse error: {e}")),
    };
    // Concatenate all text parts of the first candidate.
    let content = parsed["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let usage = parsed
        .get("usageMetadata")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
}

// ── MiniMax ───────────────────────────────────────────────────────────────────
pub(super) async fn execute_minimax(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("MiniMax node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("MiniMax missing 'api_key'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("MiniMax-Text-01")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("MiniMax missing 'prompt_template'"),
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

    // OpenAI-compatible mode (newest MiniMax M-series) when base_url is set —
    // these are served on a standard /chat/completions endpoint without GroupId.
    if let Some(base) = config
        .get("base_url")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let base = resolve_template(base, context);
        return openai_compat_chat(
            "MiniMax",
            &node.id,
            &api_key,
            &base,
            &model,
            serde_json::Value::Array(messages),
            max_tokens,
            temperature,
            http_client,
        )
        .await;
    }

    // Legacy chatcompletion_v2 path (requires group_id).
    let group_id = match config.get("group_id").and_then(|v| v.as_str()) {
        Some(g) => resolve_template(g, context),
        None => {
            return NodeExecutionResult::failed(
                "MiniMax missing 'group_id' (or set 'base_url' for OpenAI-compatible mode)",
            )
        }
    };
    let url = format!("https://api.minimax.chat/v1/text/chatcompletion_v2?GroupId={group_id}");
    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    let resp = match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("MiniMax request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("MiniMax API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("MiniMax parse error: {e}")),
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

// ── Ernie (百度文心一言) — OAuth2 token exchange ───────────────────────────────
pub(super) async fn execute_ernie(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Ernie node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Ernie missing 'api_key' (client_id)"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("ernie-4.0-8k")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Ernie missing 'prompt_template'"),
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

    // OpenAI-compatible mode (ERNIE 4.5 / 5.0 / X1 on Qianfan v2) when base_url is
    // set — bearer auth with api_key, no OAuth token exchange or secret_key needed.
    if let Some(base) = config
        .get("base_url")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let base = resolve_template(base, context);
        let mut messages = Vec::new();
        if !system_prompt.is_empty() {
            messages.push(serde_json::json!({ "role": "system", "content": system_prompt }));
        }
        messages.push(serde_json::json!({ "role": "user", "content": prompt }));
        return openai_compat_chat(
            "Ernie",
            &node.id,
            &api_key,
            &base,
            &model,
            serde_json::Value::Array(messages),
            max_tokens,
            temperature,
            http_client,
        )
        .await;
    }

    // Legacy wenxinworkshop path requires the client_secret for OAuth2 exchange.
    let secret_key = match config.get("secret_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed(
            "Ernie missing 'secret_key' (or set 'base_url' for Qianfan v2 OpenAI-compatible mode)",
        ),
    };

    // Step 1: exchange client credentials for access_token
    let token_url = format!(
        "https://aip.baidubce.com/oauth/2.0/token?grant_type=client_credentials&client_id={api_key}&client_secret={secret_key}"
    );
    let token_resp = match http_client.post(&token_url).send().await {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Ernie token request error: {e}")),
    };
    let token_body = token_resp.text().await.unwrap_or_default();
    let token_json: serde_json::Value = match serde_json::from_str(&token_body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Ernie token parse error: {e}")),
    };
    let access_token = match token_json.get("access_token").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            let err = token_json
                .get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return NodeExecutionResult::failed(format!("Ernie token error: {err}"));
        }
    };

    // Step 2: call chat API
    let mut messages = Vec::new();
    if !system_prompt.is_empty() {
        messages.push(serde_json::json!({ "role": "user", "content": &system_prompt }));
        messages.push(serde_json::json!({ "role": "assistant", "content": "好的，我明白了。" }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": prompt }));

    let chat_url = format!(
        "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop/chat/{model}?access_token={access_token}"
    );
    let payload = serde_json::json!({
        "messages": messages,
        "max_output_tokens": max_tokens,
        "temperature": temperature,
    });

    let resp = match http_client
        .post(&chat_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Ernie chat request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("Ernie API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Ernie parse error: {e}")),
    };

    if let Some(err_msg) = parsed.get("error_msg").and_then(|v| v.as_str()) {
        return NodeExecutionResult::failed(format!("Ernie API error: {err_msg}"));
    }

    let content = parsed["result"].as_str().unwrap_or("").to_string();
    let usage = parsed
        .get("usage")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
}
