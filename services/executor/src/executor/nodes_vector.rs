// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Vector-store nodes (Weaviate / Chroma) over their HTTP REST APIs.
//!
//! Both reuse the multi-operation HTTP pattern established by the Qdrant node:
//! resolve templated config, pick an `operation`, and return `{status, body}`.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── Weaviate ─────────────────────────────────────────────────────────────────
pub(super) async fn execute_weaviate(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed(
            "Weaviate requires 'host' (e.g. https://xyz.weaviate.network or http://localhost:8080)",
        ),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("query")
        .to_string();
    let api_key = cfg
        .get("api_key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let auth = |rb: reqwest::RequestBuilder| -> reqwest::RequestBuilder {
        let rb = rb.header("Content-Type", "application/json");
        match &api_key {
            Some(k) => rb.header("Authorization", format!("Bearer {k}")),
            None => rb,
        }
    };
    let send = |rb: reqwest::RequestBuilder, op: &'static str| async move {
        match rb.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body }).to_string(),
                )
            }
            Err(e) => NodeExecutionResult::failed(format!("Weaviate {op} error: {e}")),
        }
    };

    match operation.as_str() {
        // GraphQL search (nearVector / nearText / BM25 — caller supplies the query).
        "query" => {
            let query = match cfg.get("query").and_then(|v| v.as_str()) {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Weaviate query requires 'query' (a GraphQL query string)",
                    )
                }
            };
            let url = format!("{host}/v1/graphql");
            let body = serde_json::json!({ "query": query });
            send(auth(http_client.post(&url)).json(&body), "query").await
        }
        "create_object" => {
            let class = match cfg.get("class").and_then(|v| v.as_str()) {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => return NodeExecutionResult::failed("Weaviate create_object requires 'class'"),
            };
            let properties = cfg
                .get("properties")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let mut body = serde_json::json!({ "class": class, "properties": properties });
            if let Some(id) = cfg.get("id").and_then(|v| v.as_str()) {
                body["id"] = serde_json::Value::String(id.to_string());
            }
            if let Some(vector) = cfg.get("vector") {
                body["vector"] = json_array_or_parse(vector);
            }
            let url = format!("{host}/v1/objects");
            send(auth(http_client.post(&url)).json(&body), "create_object").await
        }
        "get_object" => {
            let class = match cfg.get("class").and_then(|v| v.as_str()) {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => return NodeExecutionResult::failed("Weaviate get_object requires 'class'"),
            };
            let id = match cfg.get("id").and_then(|v| v.as_str()) {
                Some(i) if !i.is_empty() => i.to_string(),
                _ => return NodeExecutionResult::failed("Weaviate get_object requires 'id'"),
            };
            let url = format!("{host}/v1/objects/{class}/{id}");
            send(auth(http_client.get(&url)), "get_object").await
        }
        "delete_object" => {
            let class = match cfg.get("class").and_then(|v| v.as_str()) {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => return NodeExecutionResult::failed("Weaviate delete_object requires 'class'"),
            };
            let id = match cfg.get("id").and_then(|v| v.as_str()) {
                Some(i) if !i.is_empty() => i.to_string(),
                _ => return NodeExecutionResult::failed("Weaviate delete_object requires 'id'"),
            };
            let url = format!("{host}/v1/objects/{class}/{id}");
            send(auth(http_client.delete(&url)), "delete_object").await
        }
        other => NodeExecutionResult::failed(format!("Weaviate unknown operation '{other}'")),
    }
}

// ── Chroma ───────────────────────────────────────────────────────────────────
pub(super) async fn execute_chroma(
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
                "Chroma requires 'host' (e.g. http://localhost:8000)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("query")
        .to_string();
    let api_key = cfg
        .get("api_key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let auth = |rb: reqwest::RequestBuilder| -> reqwest::RequestBuilder {
        let rb = rb.header("Content-Type", "application/json");
        match &api_key {
            Some(k) => rb.header("Authorization", format!("Bearer {k}")),
            None => rb,
        }
    };
    let send = |rb: reqwest::RequestBuilder, op: &'static str| async move {
        match rb.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body }).to_string(),
                )
            }
            Err(e) => NodeExecutionResult::failed(format!("Chroma {op} error: {e}")),
        }
    };

    // Data-plane ops address a collection by its id; resolve it via get_collection first.
    let collection_id = || -> Result<String, NodeExecutionResult> {
        match cfg.get("collection_id").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => Ok(c.to_string()),
            _ => Err(NodeExecutionResult::failed(
                "Chroma requires 'collection_id' (use the get_collection op to resolve a name)",
            )),
        }
    };

    match operation.as_str() {
        // Resolve a collection name to its id/metadata.
        "get_collection" => {
            let name = match cfg.get("collection").and_then(|v| v.as_str()) {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Chroma get_collection requires 'collection' (name)",
                    )
                }
            };
            let url = format!("{host}/api/v1/collections/{name}");
            send(auth(http_client.get(&url)), "get_collection").await
        }
        "add" => {
            let cid = match collection_id() {
                Ok(c) => c,
                Err(e) => return e,
            };
            let ids = match cfg.get("ids") {
                Some(v) => json_array_or_parse(v),
                None => return NodeExecutionResult::failed("Chroma add requires 'ids'"),
            };
            let mut body = serde_json::json!({ "ids": ids });
            if let Some(emb) = cfg.get("embeddings") {
                body["embeddings"] = json_array_or_parse(emb);
            }
            if let Some(md) = cfg.get("metadatas") {
                body["metadatas"] = json_array_or_parse(md);
            }
            if let Some(docs) = cfg.get("documents") {
                body["documents"] = json_array_or_parse(docs);
            }
            let url = format!("{host}/api/v1/collections/{cid}/add");
            send(auth(http_client.post(&url)).json(&body), "add").await
        }
        "query" => {
            let cid = match collection_id() {
                Ok(c) => c,
                Err(e) => return e,
            };
            let query_embeddings = match cfg.get("query_embeddings") {
                Some(v) => json_array_or_parse(v),
                None => {
                    return NodeExecutionResult::failed("Chroma query requires 'query_embeddings'")
                }
            };
            let n_results = cfg.get("n_results").and_then(|v| v.as_u64()).unwrap_or(10);
            let mut body =
                serde_json::json!({ "query_embeddings": query_embeddings, "n_results": n_results });
            if let Some(w) = cfg.get("where") {
                body["where"] = w.clone();
            }
            let url = format!("{host}/api/v1/collections/{cid}/query");
            send(auth(http_client.post(&url)).json(&body), "query").await
        }
        "delete" => {
            let cid = match collection_id() {
                Ok(c) => c,
                Err(e) => return e,
            };
            let ids = match cfg.get("ids") {
                Some(v) => json_array_or_parse(v),
                None => return NodeExecutionResult::failed("Chroma delete requires 'ids'"),
            };
            let body = serde_json::json!({ "ids": ids });
            let url = format!("{host}/api/v1/collections/{cid}/delete");
            send(auth(http_client.post(&url)).json(&body), "delete").await
        }
        other => NodeExecutionResult::failed(format!("Chroma unknown operation '{other}'")),
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
    async fn weaviate_requires_host() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w1".into(),
            node_type: NodeType::Weaviate,
            config: Some(serde_json::json!({"operation":"query"})),
        };
        let r = execute_weaviate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn weaviate_query_requires_query() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w2".into(),
            node_type: NodeType::Weaviate,
            config: Some(serde_json::json!({"host":"http://localhost:8080","operation":"query"})),
        };
        let r = execute_weaviate(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn weaviate_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w3".into(),
            node_type: NodeType::Weaviate,
            config: Some(serde_json::json!({"host":"http://localhost:8080","operation":"nope"})),
        };
        let r = execute_weaviate(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    #[tokio::test]
    async fn chroma_requires_host() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c1".into(),
            node_type: NodeType::Chroma,
            config: Some(serde_json::json!({"operation":"query"})),
        };
        let r = execute_chroma(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn chroma_query_requires_collection_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c2".into(),
            node_type: NodeType::Chroma,
            config: Some(serde_json::json!({"host":"http://localhost:8000","operation":"query"})),
        };
        let r = execute_chroma(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("collection_id"));
    }

    #[tokio::test]
    async fn chroma_query_requires_embeddings() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c3".into(),
            node_type: NodeType::Chroma,
            config: Some(serde_json::json!({
                "host":"http://localhost:8000",
                "operation":"query",
                "collection_id":"abc"
            })),
        };
        let r = execute_chroma(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("query_embeddings"));
    }
}
