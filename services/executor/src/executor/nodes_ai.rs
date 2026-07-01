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
        .unwrap_or("gpt-5.4-mini")
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

    // When someone is watching this execution, stream tokens live; the final
    // output_json is identical to the non-streamed path.
    if super::nodes_stream::streaming_enabled() {
        return match super::nodes_stream::stream_openai_chat(
            http_client,
            "https://api.openai.com/v1/chat/completions",
            &api_key,
            &node.id,
            "OpenAI",
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
        .unwrap_or("gemini-2.5-flash")
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

#[derive(serde::Serialize)]
struct RagQueryRequest {
    tenant_id: String,
    kb: String,
    query: String,
    top_k: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_score: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    rerank: bool,
}

/// Retrieval-Augmented Generation: query a pgvector knowledge base through the
/// AI runtime (`POST /v1/rag/query`) and return the retrieved chunks as JSON.
/// Downstream nodes can reference them via `{{node_id.results}}`.
pub(super) async fn execute_rag(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
    let base_url = match ai_runtime_base_url {
        Some(url) => url,
        None => {
            return NodeExecutionResult::failed(
                "RAG node requires AI_RUNTIME_BASE_URL to be configured",
            )
        }
    };
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("RAG node requires config"),
    };
    let kb = match cfg.get("kb").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("RAG node missing 'kb'"),
    };
    let query_tmpl = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("RAG node missing 'query'"),
    };
    let query = resolve_template(query_tmpl, context);
    let tenant_id = cfg
        .get("tenant_id")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "tenant-1".to_string());
    let top_k = node_config_u64(node, "top_k").unwrap_or(4).clamp(1, 50);
    let mode = cfg
        .get("mode")
        .and_then(|v| v.as_str())
        .filter(|s| *s == "vector" || *s == "hybrid")
        .map(|s| s.to_string());
    let min_score = cfg.get("min_score").and_then(|v| v.as_f64());
    let rerank = cfg.get("rerank").and_then(|v| v.as_bool()).unwrap_or(false);

    let endpoint = format!("{}/v1/rag/query", base_url.trim_end_matches('/'));
    let request = RagQueryRequest {
        tenant_id,
        kb,
        query,
        top_k,
        mode,
        min_score,
        rerank,
    };

    match client.post(&endpoint).json(&request).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.text().await {
                    Ok(body) => NodeExecutionResult::succeeded(body),
                    Err(e) => {
                        NodeExecutionResult::failed(format!("Failed to read RAG response: {e}"))
                    }
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                NodeExecutionResult::failed(format!("AI Runtime returned {status}: {body}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Failed to reach AI Runtime: {e}")),
    }
}

#[cfg(test)]
mod rag_tests {
    use super::*;
    use workflow_core::NodeType;

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{\"q\":\"billing\"}".into(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    fn rag_node(config: serde_json::Value) -> Node {
        Node {
            id: "rag1".into(),
            node_type: NodeType::Rag,
            config: Some(config),
        }
    }

    #[tokio::test]
    async fn fails_without_ai_runtime_base_url() {
        let c = reqwest::Client::new();
        let n = rag_node(serde_json::json!({ "kb": "docs", "query": "x" }));
        let r = execute_rag(&n, &ctx(), &c, None).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("AI_RUNTIME_BASE_URL"));
    }

    #[tokio::test]
    async fn fails_without_kb() {
        let c = reqwest::Client::new();
        let n = rag_node(serde_json::json!({ "query": "x" }));
        let r = execute_rag(&n, &ctx(), &c, Some("http://localhost:9")).await;
        assert!(r.error.as_deref().unwrap_or("").contains("kb"));
    }

    #[tokio::test]
    async fn fails_without_query() {
        let c = reqwest::Client::new();
        let n = rag_node(serde_json::json!({ "kb": "docs" }));
        let r = execute_rag(&n, &ctx(), &c, Some("http://localhost:9")).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn ingest_fails_without_ai_runtime_base_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "i1".into(),
            node_type: NodeType::RagIngest,
            config: Some(serde_json::json!({ "kb": "docs", "doc_id": "d", "text": "x" })),
        };
        let r = execute_rag_ingest(&n, &ctx(), &c, None).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("AI_RUNTIME_BASE_URL"));
    }

    #[tokio::test]
    async fn ingest_fails_without_doc_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "i2".into(),
            node_type: NodeType::RagIngest,
            config: Some(serde_json::json!({ "kb": "docs", "text": "x" })),
        };
        let r = execute_rag_ingest(&n, &ctx(), &c, Some("http://localhost:9")).await;
        assert!(r.error.as_deref().unwrap_or("").contains("doc_id"));
    }
}

#[derive(serde::Serialize)]
struct RagIngestRequest {
    tenant_id: String,
    kb: String,
    doc_id: String,
    text: String,
    chunk_size: u64,
    overlap: u64,
}

/// Ingest a document into a pgvector knowledge base through the AI runtime
/// (`POST /v1/rag/ingest`): the `text` is chunked, embedded, and stored under
/// `doc_id` (re-ingesting the same doc_id replaces it). Returns `{doc_id, chunks}`.
pub(super) async fn execute_rag_ingest(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
    let base_url = match ai_runtime_base_url {
        Some(url) => url,
        None => {
            return NodeExecutionResult::failed(
                "RAG Ingest node requires AI_RUNTIME_BASE_URL to be configured",
            )
        }
    };
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("RAG Ingest node requires config"),
    };
    let kb = match cfg.get("kb").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("RAG Ingest node missing 'kb'"),
    };
    let doc_id = match cfg.get("doc_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => resolve_template(s, context),
        _ => return NodeExecutionResult::failed("RAG Ingest node missing 'doc_id'"),
    };
    let text_tmpl = match cfg.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("RAG Ingest node missing 'text'"),
    };
    let text = resolve_template(text_tmpl, context);
    let tenant_id = cfg
        .get("tenant_id")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "tenant-1".to_string());
    let chunk_size = node_config_u64(node, "chunk_size")
        .unwrap_or(1000)
        .clamp(50, 8000);
    let overlap = node_config_u64(node, "overlap")
        .unwrap_or(150)
        .min(chunk_size - 1);

    let endpoint = format!("{}/v1/rag/ingest", base_url.trim_end_matches('/'));
    let request = RagIngestRequest {
        tenant_id,
        kb,
        doc_id,
        text,
        chunk_size,
        overlap,
    };

    match client.post(&endpoint).json(&request).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.text().await {
                    Ok(body) => NodeExecutionResult::succeeded(body),
                    Err(e) => NodeExecutionResult::failed(format!(
                        "Failed to read RAG ingest response: {e}"
                    )),
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                NodeExecutionResult::failed(format!("AI Runtime returned {status}: {body}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Failed to reach AI Runtime: {e}")),
    }
}
