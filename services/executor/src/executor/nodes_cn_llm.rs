// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Chinese-vendor LLM nodes (DeepSeek / Qwen / Zhipu / Moonshot / Doubao /
//! MiniMax / ERNIE / Hunyuan) and their shared OpenAI-compatible helpers.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── 国内大模型通用 OpenAI-compatible helper ──────────────────────────────
async fn openai_compat_chat(
    node_name: &str,
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

    let resp = match http_client
        .post(base_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("{node_name} request error: {e}")),
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

// ── xAI Grok ─────────────────────────────────────────────────────────────────
pub(super) async fn execute_grok(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Grok node requires config"),
    };
    let (api_key, model, messages, max_tokens, temperature) =
        match extract_chat_fields("Grok", config, context, "grok-2-latest") {
            Ok(v) => v,
            Err(e) => return e,
        };
    openai_compat_chat(
        "Grok",
        &api_key,
        "https://api.x.ai/v1/chat/completions",
        &model,
        messages,
        max_tokens,
        temperature,
        http_client,
    )
    .await
}

// ── Ollama (self-hosted, OpenAI-compatible) ───────────────────────────────────
pub(super) async fn execute_ollama(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Ollama node requires config"),
    };
    // Self-hosted: base URL is configurable and the API key is optional.
    let base_url = config
        .get("base_url")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_else(|| "http://localhost:11434/v1/chat/completions".to_string());
    let api_key = config
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(|k| resolve_template(k, context))
        .unwrap_or_else(|| "ollama".to_string());
    // Reuse the shared field extraction, but the api_key is optional here so we
    // only need model/prompt/system/max_tokens/temperature from it.
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("llama3.2")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Ollama missing 'prompt_template'"),
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
    openai_compat_chat(
        "Ollama",
        &api_key,
        &base_url,
        &model,
        serde_json::Value::Array(messages),
        max_tokens,
        temperature,
        http_client,
    )
    .await
}

// ── Azure OpenAI ─────────────────────────────────────────────────────────────
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

// ── DeepSeek ─────────────────────────────────────────────────────────────────
pub(super) async fn execute_deepseek(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("DeepSeek node requires config"),
    };
    let (api_key, model, messages, max_tokens, temperature) =
        match extract_chat_fields("DeepSeek", config, context, "deepseek-chat") {
            Ok(v) => v,
            Err(e) => return e,
        };
    openai_compat_chat(
        "DeepSeek",
        &api_key,
        "https://api.deepseek.com/v1/chat/completions",
        &model,
        messages,
        max_tokens,
        temperature,
        http_client,
    )
    .await
}

// ── Qwen (通义千问 / DashScope) ───────────────────────────────────────────────
pub(super) async fn execute_qwen(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Qwen node requires config"),
    };
    let (api_key, model, messages, max_tokens, temperature) =
        match extract_chat_fields("Qwen", config, context, "qwen-max") {
            Ok(v) => v,
            Err(e) => return e,
        };
    openai_compat_chat(
        "Qwen",
        &api_key,
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        &model,
        messages,
        max_tokens,
        temperature,
        http_client,
    )
    .await
}

// ── Zhipu (智谱AI / GLM) ─────────────────────────────────────────────────────
pub(super) async fn execute_zhipu(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Zhipu node requires config"),
    };
    let (api_key, model, messages, max_tokens, temperature) =
        match extract_chat_fields("Zhipu", config, context, "glm-4") {
            Ok(v) => v,
            Err(e) => return e,
        };
    openai_compat_chat(
        "Zhipu",
        &api_key,
        "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        &model,
        messages,
        max_tokens,
        temperature,
        http_client,
    )
    .await
}

// ── Moonshot (月之暗面 / Kimi) ────────────────────────────────────────────────
pub(super) async fn execute_moonshot(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Moonshot node requires config"),
    };
    let (api_key, model, messages, max_tokens, temperature) =
        match extract_chat_fields("Moonshot", config, context, "moonshot-v1-8k") {
            Ok(v) => v,
            Err(e) => return e,
        };
    openai_compat_chat(
        "Moonshot",
        &api_key,
        "https://api.moonshot.cn/v1/chat/completions",
        &model,
        messages,
        max_tokens,
        temperature,
        http_client,
    )
    .await
}

// ── Doubao (豆包 / 火山引擎) ──────────────────────────────────────────────────
pub(super) async fn execute_doubao(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Doubao node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Doubao missing 'api_key'"),
    };
    // Doubao uses endpoint_id as the model identifier
    let model = match config.get("endpoint_id").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Doubao missing 'endpoint_id'"),
    };
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Doubao missing 'prompt_template'"),
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

    openai_compat_chat(
        "Doubao",
        &api_key,
        "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
        &model,
        serde_json::Value::Array(messages),
        max_tokens,
        temperature,
        http_client,
    )
    .await
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
    let group_id = match config.get("group_id").and_then(|v| v.as_str()) {
        Some(g) => resolve_template(g, context),
        None => return NodeExecutionResult::failed("MiniMax missing 'group_id'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("abab6.5s-chat")
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
    let secret_key = match config.get("secret_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Ernie missing 'secret_key' (client_secret)"),
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

// ── Hunyuan (腾讯混元) ────────────────────────────────────────────────────────
pub(super) async fn execute_hunyuan(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Hunyuan node requires config"),
    };
    let (api_key, model, messages, max_tokens, temperature) =
        match extract_chat_fields("Hunyuan", config, context, "hunyuan-standard") {
            Ok(v) => v,
            Err(e) => return e,
        };
    openai_compat_chat(
        "Hunyuan",
        &api_key,
        "https://api.hunyuan.cloud.tencent.com/v1/chat/completions",
        &model,
        messages,
        max_tokens,
        temperature,
        http_client,
    )
    .await
}
