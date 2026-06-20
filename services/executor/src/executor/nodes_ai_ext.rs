// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! Additional third-party LLM provider nodes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── Slice 310: Replicate ───────────────────────────────────────────────────────
pub(super) async fn execute_replicate(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Replicate requires 'api_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("run")
        .to_string();
    let auth = format!("Token {api_token}");

    match operation.as_str() {
        "run" | "create_prediction" => {
            let version = match cfg.get("version").and_then(|v| v.as_str()) {
                Some(v) if !v.is_empty() => v.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Replicate run requires 'version' (model version ID)",
                    )
                }
            };
            let input = cfg.get("input").cloned().unwrap_or(serde_json::json!({}));
            let body = serde_json::json!({ "version": version, "input": input });
            match http_client
                .post("https://api.replicate.com/v1/predictions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Replicate error: {e}")),
            }
        }
        "get_prediction" => {
            let prediction_id = match cfg.get("prediction_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Replicate get_prediction requires 'prediction_id'",
                    )
                }
            };
            let url = format!("https://api.replicate.com/v1/predictions/{prediction_id}");
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Replicate get error: {e}")),
            }
        }
        "list_models" => {
            match http_client
                .get("https://api.replicate.com/v1/models")
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Replicate list error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Replicate unknown operation '{other}'")),
    }
}

// ── Slice 311: Mistral ─────────────────────────────────────────────────────────
pub(super) async fn execute_mistral(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Mistral requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chat")
        .to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("mistral-small-latest")
                .to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed("Mistral chat requires 'messages' or 'prompt'");
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") {
                body["temperature"] = temp.clone();
            }
            if let Some(max_tokens) = cfg.get("max_tokens") {
                body["max_tokens"] = max_tokens.clone();
            }
            match http_client
                .post("https://api.mistral.ai/v1/chat/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Mistral error: {e}")),
            }
        }
        "embeddings" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("mistral-embed")
                .to_string();
            let input = match cfg.get("input") {
                Some(i) => i.clone(),
                None => return NodeExecutionResult::failed("Mistral embeddings requires 'input'"),
            };
            let body = serde_json::json!({ "model": model, "input": input });
            match http_client
                .post("https://api.mistral.ai/v1/embeddings")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Mistral embeddings error: {e}")),
            }
        }
        "list_models" => {
            match http_client
                .get("https://api.mistral.ai/v1/models")
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Mistral list models error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Mistral unknown operation '{other}'")),
    }
}

// ── Slice 314: Perplexity ──────────────────────────────────────────────────────
pub(super) async fn execute_perplexity(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Perplexity requires 'api_key'"),
    };
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("llama-3.1-sonar-small-128k-online")
        .to_string();
    let messages = if let Some(msgs) = cfg.get("messages") {
        msgs.clone()
    } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
        serde_json::json!([{"role": "user", "content": prompt}])
    } else {
        return NodeExecutionResult::failed("Perplexity requires 'messages' or 'prompt'");
    };
    let mut body = serde_json::json!({ "model": model, "messages": messages });
    if let Some(temp) = cfg.get("temperature") {
        body["temperature"] = temp.clone();
    }
    if let Some(max_tokens) = cfg.get("max_tokens") {
        body["max_tokens"] = max_tokens.clone();
    }
    if let Some(search_domain_filter) = cfg.get("search_domain_filter") {
        body["search_domain_filter"] = search_domain_filter.clone();
    }
    if let Some(return_citations) = cfg.get("return_citations") {
        body["return_citations"] = return_citations.clone();
    }
    match http_client
        .post("https://api.perplexity.ai/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Perplexity error: {e}")),
    }
}

// ── Slice 315: Cohere ──────────────────────────────────────────────────────────
pub(super) async fn execute_cohere(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Cohere requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chat")
        .to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let message = match cfg.get("message").and_then(|v| v.as_str()) {
                Some(m) if !m.is_empty() => m.to_string(),
                _ => return NodeExecutionResult::failed("Cohere chat requires 'message'"),
            };
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("command-r-plus")
                .to_string();
            let mut body = serde_json::json!({ "message": message, "model": model });
            if let Some(temperature) = cfg.get("temperature") {
                body["temperature"] = temperature.clone();
            }
            if let Some(chat_history) = cfg.get("chat_history") {
                body["chat_history"] = chat_history.clone();
            }
            match http_client
                .post("https://api.cohere.com/v1/chat")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Cohere chat error: {e}")),
            }
        }
        "embed" => {
            let texts = match cfg.get("texts") {
                Some(t) => t.clone(),
                None => {
                    return NodeExecutionResult::failed(
                        "Cohere embed requires 'texts' (array of strings)",
                    )
                }
            };
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("embed-english-v3.0")
                .to_string();
            let input_type = cfg
                .get("input_type")
                .and_then(|v| v.as_str())
                .unwrap_or("search_document")
                .to_string();
            let body =
                serde_json::json!({ "texts": texts, "model": model, "input_type": input_type });
            match http_client
                .post("https://api.cohere.com/v1/embed")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Cohere embed error: {e}")),
            }
        }
        "classify" => {
            let inputs = match cfg.get("inputs") {
                Some(i) => i.clone(),
                None => return NodeExecutionResult::failed("Cohere classify requires 'inputs'"),
            };
            let examples = match cfg.get("examples") {
                Some(e) => e.clone(),
                None => return NodeExecutionResult::failed("Cohere classify requires 'examples'"),
            };
            let body = serde_json::json!({ "inputs": inputs, "examples": examples });
            match http_client
                .post("https://api.cohere.com/v1/classify")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Cohere classify error: {e}")),
            }
        }
        "rerank" => {
            let query = match cfg.get("query").and_then(|v| v.as_str()) {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => return NodeExecutionResult::failed("Cohere rerank requires 'query'"),
            };
            let documents = match cfg.get("documents") {
                Some(d) => d.clone(),
                None => return NodeExecutionResult::failed("Cohere rerank requires 'documents'"),
            };
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("rerank-english-v3.0")
                .to_string();
            let body =
                serde_json::json!({ "query": query, "documents": documents, "model": model });
            match http_client
                .post("https://api.cohere.com/v1/rerank")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Cohere rerank error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Cohere unknown operation '{other}'")),
    }
}

// ── Slice 319: Together AI ─────────────────────────────────────────────────────
pub(super) async fn execute_togetherai(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Together AI requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chat")
        .to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo")
                .to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed(
                    "Together AI chat requires 'messages' or 'prompt'",
                );
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") {
                body["temperature"] = temp.clone();
            }
            if let Some(max_tokens) = cfg.get("max_tokens") {
                body["max_tokens"] = max_tokens.clone();
            }
            match http_client
                .post("https://api.together.xyz/v1/chat/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Together AI chat error: {e}")),
            }
        }
        "completions" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("mistralai/Mixtral-8x7B-Instruct-v0.1")
                .to_string();
            let prompt = match cfg.get("prompt").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => {
                    return NodeExecutionResult::failed("Together AI completions requires 'prompt'")
                }
            };
            let mut body = serde_json::json!({ "model": model, "prompt": prompt });
            if let Some(temp) = cfg.get("temperature") {
                body["temperature"] = temp.clone();
            }
            if let Some(max_tokens) = cfg.get("max_tokens") {
                body["max_tokens"] = max_tokens.clone();
            }
            match http_client
                .post("https://api.together.xyz/v1/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Together AI completions error: {e}"))
                }
            }
        }
        "embeddings" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("togethercomputer/m2-bert-80M-8k-retrieval")
                .to_string();
            let input = match cfg.get("input") {
                Some(i) => i.clone(),
                None => {
                    return NodeExecutionResult::failed("Together AI embeddings requires 'input'")
                }
            };
            let body = serde_json::json!({ "model": model, "input": input });
            match http_client
                .post("https://api.together.xyz/v1/embeddings")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Together AI embeddings error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Together AI unknown operation '{other}'")),
    }
}

// ── Slice 321: Hugging Face ────────────────────────────────────────────────────
pub(super) async fn execute_huggingface(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Hugging Face requires 'api_token'"),
    };
    let model = match cfg.get("model").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Hugging Face requires 'model' (e.g. gpt2 or facebook/bart-large-cnn)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("inference")
        .to_string();
    let auth = format!("Bearer {api_token}");

    match operation.as_str() {
        "inference" => {
            let inputs = match cfg.get("inputs") {
                Some(i) => i.clone(),
                None => {
                    return NodeExecutionResult::failed("Hugging Face inference requires 'inputs'")
                }
            };
            let mut body = serde_json::json!({ "inputs": inputs });
            if let Some(params) = cfg.get("parameters") {
                body["parameters"] = params.clone();
            }
            if let Some(options) = cfg.get("options") {
                body["options"] = options.clone();
            }
            let url = format!("https://api-inference.huggingface.co/models/{model}");
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Hugging Face inference error: {e}")),
            }
        }
        "model_info" => {
            let url = format!("https://huggingface.co/api/models/{model}");
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Hugging Face model_info error: {e}"))
                }
            }
        }
        "list_models" => {
            let search = cfg.get("search").and_then(|v| v.as_str()).unwrap_or("");
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);
            let url = format!("https://huggingface.co/api/models?search={search}&limit={limit}");
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Hugging Face list_models error: {e}"))
                }
            }
        }
        other => NodeExecutionResult::failed(format!("Hugging Face unknown operation '{other}'")),
    }
}

// ── Slice 322: Groq ────────────────────────────────────────────────────────────
pub(super) async fn execute_groq(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Groq requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chat")
        .to_string();
    let auth = format!("Bearer {api_key}");

    match operation.as_str() {
        "chat" => {
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("llama-3.3-70b-versatile")
                .to_string();
            let messages = if let Some(msgs) = cfg.get("messages") {
                msgs.clone()
            } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
                serde_json::json!([{"role": "user", "content": prompt}])
            } else {
                return NodeExecutionResult::failed("Groq chat requires 'messages' or 'prompt'");
            };
            let mut body = serde_json::json!({ "model": model, "messages": messages });
            if let Some(temp) = cfg.get("temperature") {
                body["temperature"] = temp.clone();
            }
            if let Some(max_tokens) = cfg.get("max_tokens") {
                body["max_tokens"] = max_tokens.clone();
            }
            if let Some(stream) = cfg.get("stream") {
                body["stream"] = stream.clone();
            }
            match http_client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Groq chat error: {e}")),
            }
        }
        "transcription" => {
            let audio_url = match cfg.get("audio_url").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => return NodeExecutionResult::failed("Groq transcription requires 'audio_url'"),
            };
            let model = cfg
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("whisper-large-v3")
                .to_string();
            let body = serde_json::json!({ "url": audio_url, "model": model });
            match http_client
                .post("https://api.groq.com/openai/v1/audio/transcriptions")
                .header("Authorization", &auth)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Groq transcription error: {e}")),
            }
        }
        "list_models" => {
            match http_client
                .get("https://api.groq.com/openai/v1/models")
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Groq list_models error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Groq unknown operation '{other}'")),
    }
}

// ── Slice 323: OpenRouter ──────────────────────────────────────────────────────
pub(super) async fn execute_openrouter(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("OpenRouter requires 'api_key'"),
    };
    let model =
        match cfg.get("model").and_then(|v| v.as_str()) {
            Some(m) if !m.is_empty() => m.to_string(),
            _ => return NodeExecutionResult::failed(
                "OpenRouter requires 'model' (e.g. openai/gpt-4o or anthropic/claude-3-5-sonnet)",
            ),
        };
    let messages = if let Some(msgs) = cfg.get("messages") {
        msgs.clone()
    } else if let Some(prompt) = cfg.get("prompt").and_then(|v| v.as_str()) {
        serde_json::json!([{"role": "user", "content": prompt}])
    } else {
        return NodeExecutionResult::failed("OpenRouter requires 'messages' or 'prompt'");
    };
    let mut body = serde_json::json!({ "model": model, "messages": messages });
    if let Some(temp) = cfg.get("temperature") {
        body["temperature"] = temp.clone();
    }
    if let Some(max_tokens) = cfg.get("max_tokens") {
        body["max_tokens"] = max_tokens.clone();
    }
    if let Some(site_url) = cfg.get("site_url").and_then(|v| v.as_str()) {
        body["site_url"] = serde_json::json!(site_url);
    }
    if let Some(site_name) = cfg.get("site_name").and_then(|v| v.as_str()) {
        body["site_name"] = serde_json::json!(site_name);
    }

    let mut req = http_client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json");
    if let Some(site_url) = cfg.get("site_url").and_then(|v| v.as_str()) {
        req = req.header("HTTP-Referer", site_url);
    }
    if let Some(site_name) = cfg.get("site_name").and_then(|v| v.as_str()) {
        req = req.header("X-Title", site_name);
    }
    match req.json(&body).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("OpenRouter error: {e}")),
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
    async fn replicate_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r1".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"version":"abc123","input":{}})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn replicate_run_fails_without_version() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r2".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"run"})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("version"));
    }

    #[tokio::test]
    async fn replicate_get_prediction_fails_without_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r3".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"get_prediction"})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prediction_id"));
    }

    #[tokio::test]
    async fn replicate_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r4".into(),
            node_type: NodeType::Replicate,
            config: Some(serde_json::json!({"api_token":"test","operation":"invalid"})),
        };
        let r = execute_replicate(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Mistral ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn mistral_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m1".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"prompt":"hello"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn mistral_chat_fails_without_messages_or_prompt() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m2".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    #[tokio::test]
    async fn mistral_embeddings_fails_without_input() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m3".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"embeddings"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("input"));
    }

    #[tokio::test]
    async fn mistral_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m4".into(),
            node_type: NodeType::Mistral,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad_op"})),
        };
        let r = execute_mistral(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── WhatsApp ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn perplexity_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p1".into(),
            node_type: NodeType::Perplexity,
            config: Some(serde_json::json!({"prompt":"hello"})),
        };
        let r = execute_perplexity(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn perplexity_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p2".into(),
            node_type: NodeType::Perplexity,
            config: Some(serde_json::json!({"api_key":"test"})),
        };
        let r = execute_perplexity(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    // ── Cohere ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cohere_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c1".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"message":"hello"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn cohere_chat_fails_without_message() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c2".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("message"));
    }

    #[tokio::test]
    async fn cohere_embed_fails_without_texts() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c3".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"embed"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("texts"));
    }

    #[tokio::test]
    async fn cohere_rerank_fails_without_query() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c4".into(),
            node_type: NodeType::Cohere,
            config: Some(
                serde_json::json!({"api_key":"test","operation":"rerank","documents":["doc1"]}),
            ),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn cohere_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c5".into(),
            node_type: NodeType::Cohere,
            config: Some(serde_json::json!({"api_key":"test","operation":"invalid"})),
        };
        let r = execute_cohere(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Google Drive ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn togetherai_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t1".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"prompt":"hello"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn togetherai_chat_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t2".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    #[tokio::test]
    async fn togetherai_completions_fails_without_prompt() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t3".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"completions"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn togetherai_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "t4".into(),
            node_type: NodeType::Togetherai,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad"})),
        };
        let r = execute_togetherai(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── AWS S3 ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn huggingface_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h1".into(),
            node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"model":"gpt2","inputs":"hello"})),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn huggingface_fails_without_model() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h2".into(),
            node_type: NodeType::Huggingface,
            config: Some(serde_json::json!({"api_token":"hf_test","inputs":"hello"})),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("model"));
    }

    #[tokio::test]
    async fn huggingface_inference_fails_without_inputs() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h3".into(),
            node_type: NodeType::Huggingface,
            config: Some(
                serde_json::json!({"api_token":"hf_test","model":"gpt2","operation":"inference"}),
            ),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("inputs"));
    }

    #[tokio::test]
    async fn huggingface_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "h4".into(),
            node_type: NodeType::Huggingface,
            config: Some(
                serde_json::json!({"api_token":"hf_test","model":"gpt2","operation":"bad"}),
            ),
        };
        let r = execute_huggingface(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    #[tokio::test]
    async fn groq_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g1".into(),
            node_type: NodeType::Groq,
            config: Some(serde_json::json!({"prompt":"hello"})),
        };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn groq_chat_fails_without_prompt_or_messages() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g2".into(),
            node_type: NodeType::Groq,
            config: Some(serde_json::json!({"api_key":"test","operation":"chat"})),
        };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    #[tokio::test]
    async fn groq_transcription_fails_without_audio_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g3".into(),
            node_type: NodeType::Groq,
            config: Some(serde_json::json!({"api_key":"test","operation":"transcription"})),
        };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("audio_url"));
    }

    #[tokio::test]
    async fn groq_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g4".into(),
            node_type: NodeType::Groq,
            config: Some(serde_json::json!({"api_key":"test","operation":"bad"})),
        };
        let r = execute_groq(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── OpenRouter ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn openrouter_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "o1".into(),
            node_type: NodeType::Openrouter,
            config: Some(serde_json::json!({"model":"openai/gpt-4o","prompt":"hello"})),
        };
        let r = execute_openrouter(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn openrouter_fails_without_model() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "o2".into(),
            node_type: NodeType::Openrouter,
            config: Some(serde_json::json!({"api_key":"test","prompt":"hello"})),
        };
        let r = execute_openrouter(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("model"));
    }

    #[tokio::test]
    async fn openrouter_fails_without_messages_or_prompt() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "o3".into(),
            node_type: NodeType::Openrouter,
            config: Some(serde_json::json!({"api_key":"test","model":"openai/gpt-4o"})),
        };
        let r = execute_openrouter(&n, &ctx(), &c).await;
        assert!(
            r.error.as_deref().unwrap_or("").contains("messages")
                || r.error.as_deref().unwrap_or("").contains("prompt")
        );
    }

    // ── Qdrant ────────────────────────────────────────────────────────────────
}
