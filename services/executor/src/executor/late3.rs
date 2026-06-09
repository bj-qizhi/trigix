// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Late-stage integration nodes (continued).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

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

// ── Slice 324: Qdrant ──────────────────────────────────────────────────────────
pub(super) async fn execute_qdrant(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Qdrant requires 'host' (e.g. https://xyz.qdrant.io or http://localhost:6333)",
            )
        }
    };
    let collection = match cfg.get("collection").and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => return NodeExecutionResult::failed("Qdrant requires 'collection'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("search")
        .to_string();
    let api_key = cfg
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut builder = |method: &str, url: &str| -> reqwest::RequestBuilder {
        let rb = match method {
            "POST" => http_client.post(url),
            "PUT" => http_client.put(url),
            "DELETE" => http_client.delete(url),
            _ => http_client.get(url),
        };
        if let Some(ref key) = api_key {
            rb.header("api-key", key)
                .header("Content-Type", "application/json")
        } else {
            rb.header("Content-Type", "application/json")
        }
    };

    match operation.as_str() {
        "search" => {
            let vector = match cfg.get("vector") {
                Some(v) => json_array_or_parse(v),
                None => return NodeExecutionResult::failed("Qdrant search requires 'vector'"),
            };
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);
            let mut body = serde_json::json!({ "vector": vector, "limit": limit });
            if let Some(filter) = cfg.get("filter") {
                body["filter"] = filter.clone();
            }
            if let Some(with_payload) = cfg.get("with_payload") {
                body["with_payload"] = with_payload.clone();
            }
            let url = format!("{}/collections/{}/points/search", host, collection);
            match builder("POST", &url).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant search error: {e}")),
            }
        }
        "upsert" => {
            let points = match cfg.get("points") {
                Some(p) => json_array_or_parse(p),
                None => {
                    return NodeExecutionResult::failed(
                        "Qdrant upsert requires 'points' (array of {id, vector, payload})",
                    )
                }
            };
            let body = serde_json::json!({ "points": points });
            let url = format!("{}/collections/{}/points", host, collection);
            match builder("PUT", &url).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant upsert error: {e}")),
            }
        }
        "delete" => {
            let ids = match cfg.get("ids") {
                Some(v) => json_array_or_parse(v),
                None => return NodeExecutionResult::failed("Qdrant delete requires 'ids'"),
            };
            let body = serde_json::json!({ "points": ids });
            let url = format!("{}/collections/{}/points/delete", host, collection);
            match builder("POST", &url).json(&body).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant delete error: {e}")),
            }
        }
        "get_collection" => {
            let url = format!("{}/collections/{}", host, collection);
            match builder("GET", &url).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Qdrant get_collection error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Qdrant unknown operation '{other}'")),
    }
}

// ── Slice 325: Cloudinary ──────────────────────────────────────────────────────
pub(super) async fn execute_cloudinary(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let cloud_name = match cfg.get("cloud_name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'cloud_name'"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'api_key'"),
    };
    let api_secret = match cfg.get("api_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'api_secret'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("{api_key}:{api_secret}").as_bytes());
    let auth = format!("Basic {encoded}");

    match operation.as_str() {
        "list" => {
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let url =
                format!("https://api.cloudinary.com/v1_1/{cloud_name}/resources/{resource_type}");
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
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary list error: {e}")),
            }
        }
        "upload" => {
            let file = match cfg.get("file").and_then(|v| v.as_str()) {
                Some(f) if !f.is_empty() => f.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Cloudinary upload requires 'file' (URL or base64 data URI)",
                    )
                }
            };
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let url =
                format!("https://api.cloudinary.com/v1_1/{cloud_name}/{resource_type}/upload");
            let mut form_data = std::collections::HashMap::new();
            form_data.insert("file", file.clone());
            form_data.insert("api_key", api_key.clone());
            // Timestamp-based signature would be needed for authenticated uploads
            // Using unsigned upload preset if configured
            if let Some(preset) = cfg.get("upload_preset").and_then(|v| v.as_str()) {
                form_data.insert("upload_preset", preset.to_string());
            }
            if let Some(folder) = cfg.get("folder").and_then(|v| v.as_str()) {
                form_data.insert("folder", folder.to_string());
            }
            match http_client.post(&url).form(&form_data).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary upload error: {e}")),
            }
        }
        "destroy" => {
            let public_id = match cfg.get("public_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Cloudinary destroy requires 'public_id'"),
            };
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let url =
                format!("https://api.cloudinary.com/v1_1/{cloud_name}/{resource_type}/destroy");
            let body = serde_json::json!({ "public_id": public_id });
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
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary destroy error: {e}")),
            }
        }
        "transform_url" => {
            let public_id = match cfg.get("public_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Cloudinary transform_url requires 'public_id'",
                    )
                }
            };
            let transformation = cfg
                .get("transformation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let format = cfg.get("format").and_then(|v| v.as_str()).unwrap_or("jpg");
            let url = if transformation.is_empty() {
                format!("https://res.cloudinary.com/{cloud_name}/{resource_type}/upload/{public_id}.{format}")
            } else {
                format!("https://res.cloudinary.com/{cloud_name}/{resource_type}/upload/{transformation}/{public_id}.{format}")
            };
            NodeExecutionResult::succeeded(
                serde_json::json!({ "url": url, "public_id": public_id }).to_string(),
            )
        }
        other => NodeExecutionResult::failed(format!("Cloudinary unknown operation '{other}'")),
    }
}

#[cfg(test)]
mod tests_322_325 {
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

    // ── Groq ──────────────────────────────────────────────────────────────────

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

    #[tokio::test]
    async fn qdrant_fails_without_host() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "q1".into(),
            node_type: NodeType::Qdrant,
            config: Some(
                serde_json::json!({"collection":"test","operation":"search","vector":[0.1]}),
            ),
        };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn qdrant_fails_without_collection() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "q2".into(),
            node_type: NodeType::Qdrant,
            config: Some(serde_json::json!({"host":"http://localhost:6333","operation":"search"})),
        };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("collection"));
    }

    #[tokio::test]
    async fn qdrant_search_fails_without_vector() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "q3".into(),
            node_type: NodeType::Qdrant,
            config: Some(
                serde_json::json!({"host":"http://localhost:6333","collection":"test","operation":"search"}),
            ),
        };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vector"));
    }

    #[tokio::test]
    async fn qdrant_upsert_fails_without_points() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "q4".into(),
            node_type: NodeType::Qdrant,
            config: Some(
                serde_json::json!({"host":"http://localhost:6333","collection":"test","operation":"upsert"}),
            ),
        };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("points"));
    }

    #[tokio::test]
    async fn qdrant_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "q5".into(),
            node_type: NodeType::Qdrant,
            config: Some(
                serde_json::json!({"host":"http://localhost:6333","collection":"test","operation":"bad"}),
            ),
        };
        let r = execute_qdrant(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Cloudinary ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cloudinary_fails_without_cloud_name() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl1".into(),
            node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"api_key":"k","api_secret":"s"})),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("cloud_name"));
    }

    #[tokio::test]
    async fn cloudinary_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl2".into(),
            node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"cloud_name":"mycloud","api_secret":"s"})),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn cloudinary_destroy_fails_without_public_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl3".into(),
            node_type: NodeType::Cloudinary,
            config: Some(
                serde_json::json!({"cloud_name":"c","api_key":"k","api_secret":"s","operation":"destroy"}),
            ),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("public_id"));
    }

    #[tokio::test]
    async fn cloudinary_transform_url_succeeds_without_network() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl4".into(),
            node_type: NodeType::Cloudinary,
            config: Some(
                serde_json::json!({"cloud_name":"mycloud","api_key":"k","api_secret":"s","operation":"transform_url","public_id":"sample","transformation":"w_300,h_200"}),
            ),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        // transform_url is local — no network needed
        assert!(r
            .output_json
            .as_deref()
            .unwrap_or("")
            .contains("res.cloudinary.com"));
    }
}

pub(super) async fn execute_gcal(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Calendar requires 'access_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_events")
        .to_string();
    let auth = format!("Bearer {access_token}");
    let base = "https://www.googleapis.com/calendar/v3";

    match operation.as_str() {
        "list_calendars" => {
            let url = format!("{base}/users/me/calendarList");
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
                Err(e) => NodeExecutionResult::failed(format!(
                    "Google Calendar list_calendars error: {e}"
                )),
            }
        }
        "list_events" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let mut url = format!("{base}/calendars/{calendar_id}/events");
            if let Some(q) = cfg.get("query").and_then(|v| v.as_str()) {
                if !q.is_empty() {
                    url.push_str(&format!("?q={}", urlencoding_simple(q)));
                }
            }
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
                    NodeExecutionResult::failed(format!("Google Calendar list_events error: {e}"))
                }
            }
        }
        "get_event" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let event_id = match cfg.get("event_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Google Calendar get_event requires 'event_id'",
                    )
                }
            };
            let url = format!("{base}/calendars/{calendar_id}/events/{event_id}");
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
                    NodeExecutionResult::failed(format!("Google Calendar get_event error: {e}"))
                }
            }
        }
        "create_event" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let summary = cfg
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("New Event");
            let start_time = match cfg.get("start_time").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Google Calendar create_event requires 'start_time'",
                    )
                }
            };
            let end_time = cfg
                .get("end_time")
                .and_then(|v| v.as_str())
                .unwrap_or(&start_time)
                .to_string();
            let timezone = cfg
                .get("timezone")
                .and_then(|v| v.as_str())
                .unwrap_or("UTC");
            let description = cfg
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let body = serde_json::json!({
                "summary": summary,
                "description": description,
                "start": { "dateTime": start_time, "timeZone": timezone },
                "end":   { "dateTime": end_time,   "timeZone": timezone }
            });
            let url = format!("{base}/calendars/{calendar_id}/events");
            match http_client
                .post(&url)
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
                Err(e) => {
                    NodeExecutionResult::failed(format!("Google Calendar create_event error: {e}"))
                }
            }
        }
        "delete_event" => {
            let calendar_id = cfg
                .get("calendar_id")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            let event_id = match cfg.get("event_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Google Calendar delete_event requires 'event_id'",
                    )
                }
            };
            let url = format!("{base}/calendars/{calendar_id}/events/{event_id}");
            match http_client
                .delete(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "deleted": true }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("Google Calendar delete_event error: {e}"))
                }
            }
        }
        other => {
            NodeExecutionResult::failed(format!("Google Calendar unknown operation '{other}'"))
        }
    }
}

pub(super) async fn execute_docusign(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("DocuSign requires 'access_token'"),
    };
    let account_id = match cfg.get("account_id").and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return NodeExecutionResult::failed("DocuSign requires 'account_id'"),
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://demo.docusign.net/restapi");
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_envelopes")
        .to_string();
    let auth = format!("Bearer {access_token}");

    match operation.as_str() {
        "list_envelopes" => {
            let from_date = cfg
                .get("from_date")
                .and_then(|v| v.as_str())
                .unwrap_or("2024-01-01");
            let url =
                format!("{base_url}/v2.1/accounts/{account_id}/envelopes?from_date={from_date}");
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
                    NodeExecutionResult::failed(format!("DocuSign list_envelopes error: {e}"))
                }
            }
        }
        "get_envelope" => {
            let envelope_id = match cfg.get("envelope_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "DocuSign get_envelope requires 'envelope_id'",
                    )
                }
            };
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes/{envelope_id}");
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
                Err(e) => NodeExecutionResult::failed(format!("DocuSign get_envelope error: {e}")),
            }
        }
        "create_envelope" => {
            let body = cfg
                .get("body")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes");
            match http_client
                .post(&url)
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
                Err(e) => {
                    NodeExecutionResult::failed(format!("DocuSign create_envelope error: {e}"))
                }
            }
        }
        "void_envelope" => {
            let envelope_id = match cfg.get("envelope_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "DocuSign void_envelope requires 'envelope_id'",
                    )
                }
            };
            let reason = cfg
                .get("void_reason")
                .and_then(|v| v.as_str())
                .unwrap_or("Voided via workflow");
            let body = serde_json::json!({ "status": "voided", "voidedReason": reason });
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes/{envelope_id}");
            match http_client
                .put(&url)
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
                Err(e) => NodeExecutionResult::failed(format!("DocuSign void_envelope error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("DocuSign unknown operation '{other}'")),
    }
}

pub(super) async fn execute_xero(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Xero requires 'access_token'"),
    };
    let tenant_id = match cfg.get("tenant_id").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Xero requires 'tenant_id'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/Contacts");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://api.xero.com/api.xro/2.0{endpoint}");
    let auth = format!("Bearer {access_token}");

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        _ => http_client.get(&url),
    };
    req = req
        .header("Authorization", &auth)
        .header("Xero-Tenant-Id", &tenant_id)
        .header("Accept", "application/json");
    if let Some(body) = cfg.get("body") {
        if !matches!(method.as_str(), "GET" | "DELETE") {
            req = req.json(body);
        }
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Xero error: {e}")),
    }
}

pub(super) async fn execute_calendly(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Calendly requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("get_current_user")
        .to_string();
    let auth = format!("Bearer {api_key}");
    let base = "https://api.calendly.com";

    match operation.as_str() {
        "get_current_user" => {
            match http_client
                .get(&format!("{base}/users/me"))
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
                    NodeExecutionResult::failed(format!("Calendly get_current_user error: {e}"))
                }
            }
        }
        "list_event_types" => {
            let user_uri = match cfg.get("user_uri").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly list_event_types requires 'user_uri'",
                    )
                }
            };
            let url = format!("{base}/event_types?user={}", urlencoding_simple(&user_uri));
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
                    NodeExecutionResult::failed(format!("Calendly list_event_types error: {e}"))
                }
            }
        }
        "list_scheduled_events" => {
            let user_uri = match cfg.get("user_uri").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly list_scheduled_events requires 'user_uri'",
                    )
                }
            };
            let status_filter = cfg
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("active");
            let url = format!(
                "{base}/scheduled_events?user={}&status={}",
                urlencoding_simple(&user_uri),
                status_filter
            );
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
                Err(e) => NodeExecutionResult::failed(format!(
                    "Calendly list_scheduled_events error: {e}"
                )),
            }
        }
        "get_scheduled_event" => {
            let event_uuid = match cfg.get("event_uuid").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly get_scheduled_event requires 'event_uuid'",
                    )
                }
            };
            let url = format!("{base}/scheduled_events/{event_uuid}");
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
                    NodeExecutionResult::failed(format!("Calendly get_scheduled_event error: {e}"))
                }
            }
        }
        "cancel_event" => {
            let event_uuid = match cfg.get("event_uuid").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Calendly cancel_event requires 'event_uuid'",
                    )
                }
            };
            let reason = cfg.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            let body = serde_json::json!({ "reason": reason });
            let url = format!("{base}/scheduled_events/{event_uuid}/cancellation");
            match http_client
                .post(&url)
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
                Err(e) => NodeExecutionResult::failed(format!("Calendly cancel_event error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Calendly unknown operation '{other}'")),
    }
}

#[cfg(test)]
mod tests_326_329 {
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

    // ── Google Calendar ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn gcal_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g1".into(),
            node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"operation":"list_events"})),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn gcal_create_event_fails_without_start_time() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g2".into(),
            node_type: NodeType::Gcal,
            config: Some(
                serde_json::json!({"access_token":"tok","operation":"create_event","summary":"Meeting"}),
            ),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("start_time"));
    }

    #[tokio::test]
    async fn gcal_delete_event_fails_without_event_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g3".into(),
            node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"access_token":"tok","operation":"delete_event"})),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("event_id"));
    }

    #[tokio::test]
    async fn gcal_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g4".into(),
            node_type: NodeType::Gcal,
            config: Some(serde_json::json!({"access_token":"tok","operation":"bad"})),
        };
        let r = execute_gcal(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── DocuSign ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn docusign_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d1".into(),
            node_type: NodeType::Docusign,
            config: Some(serde_json::json!({"account_id":"abc"})),
        };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn docusign_fails_without_account_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d2".into(),
            node_type: NodeType::Docusign,
            config: Some(serde_json::json!({"access_token":"tok"})),
        };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("account_id"));
    }

    #[tokio::test]
    async fn docusign_get_envelope_fails_without_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d3".into(),
            node_type: NodeType::Docusign,
            config: Some(
                serde_json::json!({"access_token":"tok","account_id":"acc","operation":"get_envelope"}),
            ),
        };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("envelope_id"));
    }

    // ── Xero ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn xero_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "x1".into(),
            node_type: NodeType::Xero,
            config: Some(serde_json::json!({"tenant_id":"tid"})),
        };
        let r = execute_xero(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn xero_fails_without_tenant_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "x2".into(),
            node_type: NodeType::Xero,
            config: Some(serde_json::json!({"access_token":"tok"})),
        };
        let r = execute_xero(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("tenant_id"));
    }

    // ── Calendly ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn calendly_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca1".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"operation":"get_current_user"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn calendly_list_event_types_fails_without_user_uri() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca2".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"list_event_types"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("user_uri"));
    }

    #[tokio::test]
    async fn calendly_cancel_event_fails_without_uuid() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca3".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"cancel_event"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("event_uuid"));
    }

    #[tokio::test]
    async fn calendly_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ca4".into(),
            node_type: NodeType::Calendly,
            config: Some(serde_json::json!({"api_key":"key","operation":"bad"})),
        };
        let r = execute_calendly(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }
}

pub(super) async fn execute_apify(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_token = match cfg.get("api_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Apify requires 'api_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("run_actor")
        .to_string();
    let base = "https://api.apify.com/v2";
    let auth = format!("Bearer {api_token}");

    match operation.as_str() {
        "run_actor" => {
            let actor_id = match cfg.get("actor_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Apify run_actor requires 'actor_id'"),
            };
            let input = cfg
                .get("input")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let url = format!("{base}/acts/{actor_id}/runs");
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .json(&input)
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
                Err(e) => NodeExecutionResult::failed(format!("Apify run_actor error: {e}")),
            }
        }
        "get_run" => {
            let run_id = match cfg.get("run_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Apify get_run requires 'run_id'"),
            };
            let url = format!("{base}/actor-runs/{run_id}");
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
                Err(e) => NodeExecutionResult::failed(format!("Apify get_run error: {e}")),
            }
        }
        "get_dataset_items" => {
            let dataset_id = match cfg.get("dataset_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Apify get_dataset_items requires 'dataset_id'",
                    )
                }
            };
            let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(100);
            let url = format!("{base}/datasets/{dataset_id}/items?limit={limit}");
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
                    NodeExecutionResult::failed(format!("Apify get_dataset_items error: {e}"))
                }
            }
        }
        "list_actors" => {
            let url = format!("{base}/acts");
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
                Err(e) => NodeExecutionResult::failed(format!("Apify list_actors error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Apify unknown operation '{other}'")),
    }
}

pub(super) async fn execute_ganalytics(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Analytics requires 'access_token'"),
    };
    let property_id = match cfg.get("property_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return NodeExecutionResult::failed("Google Analytics requires 'property_id'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("run_report")
        .to_string();
    let auth = format!("Bearer {access_token}");
    let base = "https://analyticsdata.googleapis.com/v1beta";

    match operation.as_str() {
        "run_report" => {
            let date_ranges = cfg
                .get("date_ranges")
                .cloned()
                .unwrap_or(serde_json::json!([{"startDate":"7daysAgo","endDate":"today"}]));
            let dimensions = cfg
                .get("dimensions")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"date"}]));
            let metrics = cfg
                .get("metrics")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"sessions"}]));
            let body = serde_json::json!({
                "dateRanges": date_ranges,
                "dimensions": dimensions,
                "metrics": metrics
            });
            let url = format!("{base}/properties/{property_id}:runReport");
            match http_client
                .post(&url)
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
                Err(e) => {
                    NodeExecutionResult::failed(format!("Google Analytics run_report error: {e}"))
                }
            }
        }
        "run_realtime_report" => {
            let dimensions = cfg
                .get("dimensions")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"country"}]));
            let metrics = cfg
                .get("metrics")
                .cloned()
                .unwrap_or(serde_json::json!([{"name":"activeUsers"}]));
            let body = serde_json::json!({ "dimensions": dimensions, "metrics": metrics });
            let url = format!("{base}/properties/{property_id}:runRealtimeReport");
            match http_client
                .post(&url)
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
                Err(e) => NodeExecutionResult::failed(format!(
                    "Google Analytics run_realtime_report error: {e}"
                )),
            }
        }
        "get_metadata" => {
            let url = format!("{base}/properties/{property_id}/metadata");
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
                    NodeExecutionResult::failed(format!("Google Analytics get_metadata error: {e}"))
                }
            }
        }
        other => {
            NodeExecutionResult::failed(format!("Google Analytics unknown operation '{other}'"))
        }
    }
}

pub(super) async fn execute_neon(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Neon requires 'api_key'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_projects")
        .to_string();
    let auth = format!("Bearer {api_key}");
    let base = "https://console.neon.tech/api/v2";

    match operation.as_str() {
        "list_projects" => {
            match http_client
                .get(&format!("{base}/projects"))
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
                Err(e) => NodeExecutionResult::failed(format!("Neon list_projects error: {e}")),
            }
        }
        "get_project" => {
            let project_id = match cfg.get("project_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Neon get_project requires 'project_id'"),
            };
            let url = format!("{base}/projects/{project_id}");
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
                Err(e) => NodeExecutionResult::failed(format!("Neon get_project error: {e}")),
            }
        }
        "create_project" => {
            let name = cfg
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("new-project");
            let body = serde_json::json!({ "project": { "name": name } });
            match http_client
                .post(&format!("{base}/projects"))
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
                Err(e) => NodeExecutionResult::failed(format!("Neon create_project error: {e}")),
            }
        }
        "list_branches" => {
            let project_id = match cfg.get("project_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed("Neon list_branches requires 'project_id'")
                }
            };
            let url = format!("{base}/projects/{project_id}/branches");
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
                Err(e) => NodeExecutionResult::failed(format!("Neon list_branches error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Neon unknown operation '{other}'")),
    }
}

pub(super) async fn execute_copper(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Copper requires 'api_key'"),
    };
    let user_email = match cfg.get("user_email").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Copper requires 'user_email'"),
    };
    let resource = cfg
        .get("resource")
        .and_then(|v| v.as_str())
        .unwrap_or("people");
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();
    let base = "https://api.copper.com/developer_api/v1";

    let mut req_builder;
    match operation.as_str() {
        "list" => {
            let body = cfg.get("filter").cloned().unwrap_or(serde_json::json!({}));
            req_builder = http_client
                .post(&format!("{base}/{resource}/search"))
                .json(&body);
        }
        "get" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper get requires 'record_id'"),
            };
            req_builder = http_client.get(&format!("{base}/{resource}/{id}"));
        }
        "create" => {
            let body = cfg
                .get("body")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            req_builder = http_client.post(&format!("{base}/{resource}")).json(&body);
        }
        "update" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper update requires 'record_id'"),
            };
            let body = cfg
                .get("body")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            req_builder = http_client
                .put(&format!("{base}/{resource}/{id}"))
                .json(&body);
        }
        "delete" => {
            let id = match cfg.get("record_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Copper delete requires 'record_id'"),
            };
            req_builder = http_client.delete(&format!("{base}/{resource}/{id}"));
        }
        other => return NodeExecutionResult::failed(format!("Copper unknown operation '{other}'")),
    };
    req_builder = req_builder
        .header("X-PW-AccessToken", &api_key)
        .header("X-PW-Application", "developer_api")
        .header("X-PW-UserEmail", &user_email)
        .header("Content-Type", "application/json");
    match req_builder.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Copper error: {e}")),
    }
}

#[cfg(test)]
mod tests_330_333 {
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

    // ── Apify ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn apify_fails_without_api_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a1".into(),
            node_type: NodeType::Apify,
            config: Some(serde_json::json!({"operation":"list_actors"})),
        };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_token"));
    }

    #[tokio::test]
    async fn apify_run_actor_fails_without_actor_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a2".into(),
            node_type: NodeType::Apify,
            config: Some(serde_json::json!({"api_token":"tok","operation":"run_actor"})),
        };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("actor_id"));
    }

    #[tokio::test]
    async fn apify_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a3".into(),
            node_type: NodeType::Apify,
            config: Some(serde_json::json!({"api_token":"tok","operation":"bad"})),
        };
        let r = execute_apify(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Google Analytics ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn ganalytics_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ga1".into(),
            node_type: NodeType::Ganalytics,
            config: Some(serde_json::json!({"property_id":"123456789"})),
        };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn ganalytics_fails_without_property_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ga2".into(),
            node_type: NodeType::Ganalytics,
            config: Some(serde_json::json!({"access_token":"tok"})),
        };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("property_id"));
    }

    #[tokio::test]
    async fn ganalytics_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ga3".into(),
            node_type: NodeType::Ganalytics,
            config: Some(
                serde_json::json!({"access_token":"tok","property_id":"123","operation":"bad"}),
            ),
        };
        let r = execute_ganalytics(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Neon ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn neon_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "n1".into(),
            node_type: NodeType::Neon,
            config: Some(serde_json::json!({"operation":"list_projects"})),
        };
        let r = execute_neon(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn neon_get_project_fails_without_project_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "n2".into(),
            node_type: NodeType::Neon,
            config: Some(serde_json::json!({"api_key":"key","operation":"get_project"})),
        };
        let r = execute_neon(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_id"));
    }

    // ── Copper ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn copper_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "co1".into(),
            node_type: NodeType::Copper,
            config: Some(serde_json::json!({"user_email":"a@b.com"})),
        };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn copper_fails_without_user_email() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "co2".into(),
            node_type: NodeType::Copper,
            config: Some(serde_json::json!({"api_key":"key"})),
        };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("user_email"));
    }

    #[tokio::test]
    async fn copper_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "co3".into(),
            node_type: NodeType::Copper,
            config: Some(
                serde_json::json!({"api_key":"key","user_email":"a@b.com","operation":"bad"}),
            ),
        };
        let r = execute_copper(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }
}
