// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! AI-native building blocks beyond raw LLM-provider calls: embeddings,
//! reranking, text splitting, structured output, classification, image
//! generation, speech-to-text and text-to-speech. HTTP nodes target
//! OpenAI-compatible (and Cohere-style) endpoints with a configurable base URL.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use workflow_core::Node;

fn require_api_key(cfg: &serde_json::Value, node: &str) -> Result<String, NodeExecutionResult> {
    match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => Ok(k.to_string()),
        _ => Err(NodeExecutionResult::failed(format!(
            "{node} requires 'api_key'"
        ))),
    }
}

// ── Embedding (OpenAI-compatible) ─────────────────────────────────────────────
pub(super) async fn execute_embedding(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match require_api_key(&cfg, "Embedding") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1/embeddings")
        .to_string();
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("text-embedding-3-small")
        .to_string();
    let input = match cfg.get("input") {
        Some(v) => json_array_or_parse(v),
        None => return NodeExecutionResult::failed("Embedding requires 'input' (string or array)"),
    };
    let payload = serde_json::json!({ "model": model, "input": input });
    let resp = match http_client
        .post(&base_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Embedding request error: {e}")),
    };
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return NodeExecutionResult::failed(format!("Embedding API {}: {}", status.as_u16(), body));
    }
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let embeddings: Vec<serde_json::Value> = parsed["data"]
        .as_array()
        .map(|a| a.iter().map(|d| d["embedding"].clone()).collect())
        .unwrap_or_default();
    NodeExecutionResult::succeeded(
        serde_json::json!({
            "embeddings": embeddings,
            "model": model,
            "usage": parsed.get("usage").cloned().unwrap_or(serde_json::Value::Null),
        })
        .to_string(),
    )
}

// ── Reranker (Cohere / Jina compatible) ───────────────────────────────────────
pub(super) async fn execute_reranker(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match require_api_key(&cfg, "Reranker") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.cohere.com/v1/rerank")
        .to_string();
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("rerank-english-v3.0")
        .to_string();
    let query = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => return NodeExecutionResult::failed("Reranker requires 'query'"),
    };
    let documents = match cfg.get("documents") {
        Some(v) => json_array_or_parse(v),
        None => return NodeExecutionResult::failed("Reranker requires 'documents' (array)"),
    };
    let mut payload = serde_json::json!({ "model": model, "query": query, "documents": documents });
    if let Some(top_n) = cfg.get("top_n").and_then(|v| v.as_u64()) {
        payload["top_n"] = serde_json::json!(top_n);
    }
    match http_client
        .post(&base_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
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
        Err(e) => NodeExecutionResult::failed(format!("Reranker request error: {e}")),
    }
}

// ── Text Splitter (pure compute) ──────────────────────────────────────────────
pub(super) async fn execute_text_splitter(
    node: &Node,
    context: &ExecutionContext,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let text = cfg.get("text").and_then(|v| v.as_str()).unwrap_or("");
    if text.is_empty() {
        return NodeExecutionResult::failed("Text splitter requires 'text'");
    }
    let chunk_size = cfg
        .get("chunk_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000)
        .max(1) as usize;
    let overlap = cfg
        .get("chunk_overlap")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;
    // Step over the text by (chunk_size - overlap) characters; chunk on char
    // boundaries so multi-byte UTF-8 (e.g. Chinese) is never split.
    let chars: Vec<char> = text.chars().collect();
    let step = chunk_size.saturating_sub(overlap).max(1);
    let mut chunks: Vec<String> = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start += step;
    }
    NodeExecutionResult::succeeded(
        serde_json::json!({ "chunks": chunks, "count": chunks.len() }).to_string(),
    )
}

// Shared OpenAI-compatible chat call returning the assistant message content.
async fn chat_once(
    http_client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    payload: &serde_json::Value,
    node: &str,
) -> Result<String, NodeExecutionResult> {
    let resp = http_client
        .post(base_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(payload)
        .send()
        .await
        .map_err(|e| NodeExecutionResult::failed(format!("{node} request error: {e}")))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(NodeExecutionResult::failed(format!(
            "{node} API {}: {}",
            status.as_u16(),
            body
        )));
    }
    let parsed: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| NodeExecutionResult::failed(format!("{node} parse error: {e}")))?;
    Ok(parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

// ── Structured Output (LLM → JSON) ────────────────────────────────────────────
pub(super) async fn execute_structured_output(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match require_api_key(&cfg, "Structured output") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1/chat/completions")
        .to_string();
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-4o-mini")
        .to_string();
    let prompt = match cfg.get("prompt_template").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("Structured output requires 'prompt_template'"),
    };
    // A schema hint (if any) is folded into the system instruction.
    let mut system = "Respond with a single valid JSON object and nothing else.".to_string();
    if let Some(schema) = cfg.get("schema") {
        system.push_str(&format!(
            " Conform to this JSON schema: {}",
            json_array_or_parse(schema)
        ));
    }
    let payload = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt },
        ],
        "response_format": { "type": "json_object" },
    });
    match chat_once(
        http_client,
        &base_url,
        &api_key,
        &payload,
        "Structured output",
    )
    .await
    {
        Ok(content) => {
            let data = serde_json::from_str::<serde_json::Value>(&content)
                .unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "data": data, "raw": content, "model": model }).to_string(),
            )
        }
        Err(e) => e,
    }
}

// ── Classifier (LLM → one of N categories) ────────────────────────────────────
pub(super) async fn execute_classifier(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match require_api_key(&cfg, "Classifier") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1/chat/completions")
        .to_string();
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-4o-mini")
        .to_string();
    let input = match cfg.get("input").and_then(|v| v.as_str()) {
        Some(i) if !i.is_empty() => i.to_string(),
        _ => return NodeExecutionResult::failed("Classifier requires 'input'"),
    };
    let categories: Vec<String> = match cfg.get("categories").map(json_array_or_parse) {
        Some(serde_json::Value::Array(a)) if !a.is_empty() => a
            .iter()
            .filter_map(|c| c.as_str().map(|s| s.to_string()))
            .collect(),
        _ => {
            return NodeExecutionResult::failed(
                "Classifier requires 'categories' (non-empty array)",
            )
        }
    };
    let list = categories.join(", ");
    let system = format!(
        "You are a classifier. Choose exactly one category from: [{list}]. Reply with only the category label, nothing else."
    );
    let payload = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": input },
        ],
        "temperature": 0,
    });
    match chat_once(http_client, &base_url, &api_key, &payload, "Classifier").await {
        Ok(content) => {
            let answer = content.trim();
            // Snap to a known category (case-insensitive contains) when possible.
            let category = categories
                .iter()
                .find(|c| answer.eq_ignore_ascii_case(c))
                .or_else(|| {
                    categories
                        .iter()
                        .find(|c| answer.to_lowercase().contains(&c.to_lowercase()))
                })
                .cloned()
                .unwrap_or_else(|| answer.to_string());
            NodeExecutionResult::succeeded(
                serde_json::json!({ "category": category, "raw": content }).to_string(),
            )
        }
        Err(e) => e,
    }
}

// ── Image generation (OpenAI-compatible images) ───────────────────────────────
pub(super) async fn execute_image_gen(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match require_api_key(&cfg, "Image generation") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1/images/generations")
        .to_string();
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("dall-e-3");
    let prompt = match cfg.get("prompt").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("Image generation requires 'prompt'"),
    };
    let size = cfg
        .get("size")
        .and_then(|v| v.as_str())
        .unwrap_or("1024x1024");
    let n = cfg.get("n").and_then(|v| v.as_u64()).unwrap_or(1);
    let payload = serde_json::json!({ "model": model, "prompt": prompt, "size": size, "n": n });
    match http_client
        .post(&base_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
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
        Err(e) => NodeExecutionResult::failed(format!("Image generation request error: {e}")),
    }
}

// ── Speech-to-Text (Whisper transcription, multipart) ─────────────────────────
pub(super) async fn execute_speech_to_text(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match require_api_key(&cfg, "Speech-to-text") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1/audio/transcriptions")
        .to_string();
    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("whisper-1")
        .to_string();
    let audio_b64 = match cfg.get("audio_base64").and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return NodeExecutionResult::failed("Speech-to-text requires 'audio_base64'"),
    };
    let filename = cfg
        .get("filename")
        .and_then(|v| v.as_str())
        .unwrap_or("audio.mp3")
        .to_string();
    let audio = match base64::engine::general_purpose::STANDARD.decode(audio_b64.trim()) {
        Ok(b) => b,
        Err(e) => {
            return NodeExecutionResult::failed(format!(
                "Speech-to-text 'audio_base64' is not valid base64: {e}"
            ))
        }
    };
    let part = reqwest::multipart::Part::bytes(audio).file_name(filename);
    let mut form = reqwest::multipart::Form::new()
        .text("model", model)
        .part("file", part);
    if let Some(lang) = cfg.get("language").and_then(|v| v.as_str()) {
        if !lang.is_empty() {
            form = form.text("language", lang.to_string());
        }
    }
    match http_client
        .post(&base_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            let parsed = serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or(serde_json::Value::String(text.clone()));
            // OpenAI returns { text: "…" }; surface it directly when present.
            let transcript = parsed
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or(text);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "text": transcript }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Speech-to-text request error: {e}")),
    }
}

// ── Text-to-Speech ────────────────────────────────────────────────────────────
pub(super) async fn execute_tts(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match require_api_key(&cfg, "Text-to-speech") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1/audio/speech")
        .to_string();
    let model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("tts-1");
    let input = match cfg.get("input").and_then(|v| v.as_str()) {
        Some(i) if !i.is_empty() => i.to_string(),
        _ => return NodeExecutionResult::failed("Text-to-speech requires 'input'"),
    };
    let voice = cfg.get("voice").and_then(|v| v.as_str()).unwrap_or("alloy");
    let format = cfg.get("format").and_then(|v| v.as_str()).unwrap_or("mp3");
    let payload = serde_json::json!({
        "model": model, "input": input, "voice": voice, "response_format": format,
    });
    match http_client
        .post(&base_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return NodeExecutionResult::failed(format!(
                    "Text-to-speech API {}: {}",
                    status.as_u16(),
                    body
                ));
            }
            match resp.bytes().await {
                Ok(b) => NodeExecutionResult::succeeded(
                    serde_json::json!({
                        "audio_base64": base64::engine::general_purpose::STANDARD.encode(&b),
                        "format": format,
                    })
                    .to_string(),
                ),
                Err(e) => NodeExecutionResult::failed(format!("Text-to-speech read error: {e}")),
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Text-to-speech request error: {e}")),
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
    async fn embedding_requires_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "e1".into(),
            node_type: NodeType::Embedding,
            config: Some(serde_json::json!({"input":"hello"})),
        };
        let r = execute_embedding(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn reranker_requires_query() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r1".into(),
            node_type: NodeType::Reranker,
            config: Some(serde_json::json!({"api_key":"k","documents":["a","b"]})),
        };
        let r = execute_reranker(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn text_splitter_chunks_with_overlap() {
        let n = Node {
            id: "ts1".into(),
            node_type: NodeType::TextSplitter,
            config: Some(serde_json::json!({
                "text":"abcdefghij","chunk_size":4,"chunk_overlap":1
            })),
        };
        let out: serde_json::Value = serde_json::from_str(
            execute_text_splitter(&n, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        // step = 3 → chunks: abcd, defg, ghij, j
        let chunks = out["chunks"].as_array().unwrap();
        assert_eq!(chunks[0], "abcd");
        assert_eq!(chunks[1], "defg");
        assert!(out["count"].as_u64().unwrap() >= 3);
    }

    #[tokio::test]
    async fn text_splitter_handles_multibyte() {
        let n = Node {
            id: "ts2".into(),
            node_type: NodeType::TextSplitter,
            config: Some(serde_json::json!({
                "text":"你好世界你好","chunk_size":2,"chunk_overlap":0
            })),
        };
        let out: serde_json::Value = serde_json::from_str(
            execute_text_splitter(&n, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out["chunks"][0], "你好");
        assert_eq!(out["count"], 3);
    }

    #[tokio::test]
    async fn classifier_requires_categories() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl1".into(),
            node_type: NodeType::Classifier,
            config: Some(serde_json::json!({"api_key":"k","input":"hi"})),
        };
        let r = execute_classifier(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("categories"));
    }

    #[tokio::test]
    async fn image_gen_requires_prompt() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ig1".into(),
            node_type: NodeType::ImageGen,
            config: Some(serde_json::json!({"api_key":"k"})),
        };
        let r = execute_image_gen(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn stt_requires_audio() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "stt1".into(),
            node_type: NodeType::SpeechToText,
            config: Some(serde_json::json!({"api_key":"k"})),
        };
        let r = execute_speech_to_text(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("audio_base64"));
    }

    #[tokio::test]
    async fn tts_requires_input() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "tts1".into(),
            node_type: NodeType::Tts,
            config: Some(serde_json::json!({"api_key":"k"})),
        };
        let r = execute_tts(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("input"));
    }
}
