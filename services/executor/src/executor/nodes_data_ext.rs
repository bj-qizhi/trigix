// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! Additional vector store / database nodes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

pub(super) async fn execute_firebase(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let project_id = match cfg.get("project_id").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("Firebase requires 'project_id'"),
    };
    let id_token = match cfg.get("id_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Firebase requires 'id_token'"),
    };
    let service = cfg
        .get("service")
        .and_then(|v| v.as_str())
        .unwrap_or("firestore")
        .to_string();
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Firebase requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };

    let url = match service.as_str() {
        "firestore" => format!("https://firestore.googleapis.com/v1/projects/{project_id}/databases/(default)/documents{ep}"),
        "rtdb" => {
            let db_url = cfg.get("database_url").and_then(|v| v.as_str())
                .unwrap_or("https://PROJECT.firebaseio.com");
            format!("{db_url}{ep}.json?auth={id_token}")
        }
        "storage" => format!("https://firebasestorage.googleapis.com/v0/b/{project_id}.appspot.com/o{ep}"),
        _ => return NodeExecutionResult::failed(format!("Firebase unknown service '{service}'")),
    };

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Content-Type", "application/json");

    // RTDB embeds auth in URL; Firestore/Storage use Bearer header
    if service != "rtdb" {
        req = req.header("Authorization", format!("Bearer {id_token}"));
    }

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
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
        Err(e) => NodeExecutionResult::failed(format!("Firebase request error: {e}")),
    }
}

// ── Slice 297: Supabase ────────────────────────────────────────────────────────

pub(super) async fn execute_supabase(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let project_url = match cfg.get("project_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("Supabase requires 'project_url'"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Supabase requires 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Supabase requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let base = project_url.trim_end_matches('/');
    let url = format!("{base}{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("apikey", &api_key)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json");

    // PostgREST Prefer header for upsert/returning
    if let Some(prefer) = cfg.get("prefer").and_then(|v| v.as_str()) {
        req = req.header("Prefer", prefer);
    }

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
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
        Err(e) => NodeExecutionResult::failed(format!("Supabase request error: {e}")),
    }
}

// ── Slice 318: Pinecone ────────────────────────────────────────────────────────
pub(super) async fn execute_pinecone(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Pinecone requires 'api_key'"),
    };
    let index_host = match cfg.get("index_host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Pinecone requires 'index_host' (e.g. https://my-index-abc.svc.pinecone.io)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("query")
        .to_string();

    match operation.as_str() {
        "query" => {
            let vector = match cfg.get("vector") {
                Some(v) => json_array_or_parse(v),
                None => {
                    return NodeExecutionResult::failed(
                        "Pinecone query requires 'vector' (float array)",
                    )
                }
            };
            let top_k = cfg
                .get("top_k")
                .or_else(|| cfg.get("top"))
                .and_then(|v| v.as_u64())
                .unwrap_or(10);
            let mut body = serde_json::json!({ "vector": vector, "topK": top_k });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                body["namespace"] = serde_json::json!(ns);
            }
            if let Some(filter) = cfg.get("filter") {
                body["filter"] = filter.clone();
            }
            if let Some(imd) = cfg.get("include_metadata").and_then(|v| v.as_bool()) {
                body["includeMetadata"] = serde_json::json!(imd);
            }
            let url = format!("{}/query", index_host);
            match http_client
                .post(&url)
                .header("Api-Key", &api_key)
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone query error: {e}")),
            }
        }
        "upsert" => {
            let vectors = match cfg.get("vectors") {
                Some(v) => json_array_or_parse(v),
                None => {
                    return NodeExecutionResult::failed(
                        "Pinecone upsert requires 'vectors' (array of {id, values, metadata})",
                    )
                }
            };
            let mut body = serde_json::json!({ "vectors": vectors });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                body["namespace"] = serde_json::json!(ns);
            }
            let url = format!("{}/vectors/upsert", index_host);
            match http_client
                .post(&url)
                .header("Api-Key", &api_key)
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone upsert error: {e}")),
            }
        }
        "delete" => {
            let ids = match cfg.get("ids") {
                Some(v) => json_array_or_parse(v),
                None => {
                    return NodeExecutionResult::failed(
                        "Pinecone delete requires 'ids' (array of vector IDs)",
                    )
                }
            };
            let mut body = serde_json::json!({ "ids": ids });
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                body["namespace"] = serde_json::json!(ns);
            }
            let url = format!("{}/vectors/delete", index_host);
            match http_client
                .post(&url)
                .header("Api-Key", &api_key)
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone delete error: {e}")),
            }
        }
        "fetch" => {
            let ids = match cfg.get("ids").and_then(|v| v.as_array()) {
                Some(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                None => {
                    return NodeExecutionResult::failed("Pinecone fetch requires 'ids' (array)")
                }
            };
            let mut url = format!("{}/vectors/fetch?ids={}", index_host, ids);
            if let Some(ns) = cfg.get("namespace").and_then(|v| v.as_str()) {
                url.push_str(&format!("&namespace={ns}"));
            }
            match http_client
                .get(&url)
                .header("Api-Key", &api_key)
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
                Err(e) => NodeExecutionResult::failed(format!("Pinecone fetch error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Pinecone unknown operation '{other}'")),
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

    let builder = |method: &str, url: &str| -> reqwest::RequestBuilder {
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
                .get(format!("{base}/projects"))
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
                .post(format!("{base}/projects"))
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
    async fn firebase_fails_without_project_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fb1".into(),
            node_type: NodeType::Firebase,
            config: Some(serde_json::json!({ "id_token": "tok", "endpoint": "/users" })),
        };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_id"));
    }

    #[tokio::test]
    async fn firebase_fails_without_id_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fb2".into(),
            node_type: NodeType::Firebase,
            config: Some(serde_json::json!({ "project_id": "my-proj", "endpoint": "/users" })),
        };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("id_token"));
    }

    #[tokio::test]
    async fn firebase_unknown_service() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "fb3".into(),
            node_type: NodeType::Firebase,
            config: Some(
                serde_json::json!({ "project_id": "proj", "id_token": "tok", "endpoint": "/doc", "service": "bogus" }),
            ),
        };
        let r = execute_firebase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bogus"));
    }

    // ── Supabase ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn supabase_fails_without_project_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sb1".into(),
            node_type: NodeType::Supabase,
            config: Some(serde_json::json!({ "api_key": "eyJ…", "endpoint": "/rest/v1/users" })),
        };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("project_url"));
    }

    #[tokio::test]
    async fn supabase_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sb2".into(),
            node_type: NodeType::Supabase,
            config: Some(
                serde_json::json!({ "project_url": "https://xyz.supabase.co", "endpoint": "/rest/v1/users" }),
            ),
        };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn supabase_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sb3".into(),
            node_type: NodeType::Supabase,
            config: Some(
                serde_json::json!({ "project_url": "https://xyz.supabase.co", "api_key": "eyJ…" }),
            ),
        };
        let r = execute_supabase(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[tokio::test]
    async fn pinecone_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p1".into(),
            node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"index_host":"https://idx.svc.pinecone.io"})),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn pinecone_fails_without_index_host() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p2".into(),
            node_type: NodeType::Pinecone,
            config: Some(serde_json::json!({"api_key":"test"})),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("index_host"));
    }

    #[tokio::test]
    async fn pinecone_query_fails_without_vector() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p3".into(),
            node_type: NodeType::Pinecone,
            config: Some(
                serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"query"}),
            ),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vector"));
    }

    #[tokio::test]
    async fn pinecone_upsert_fails_without_vectors() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p4".into(),
            node_type: NodeType::Pinecone,
            config: Some(
                serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"upsert"}),
            ),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("vectors"));
    }

    #[tokio::test]
    async fn pinecone_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "p5".into(),
            node_type: NodeType::Pinecone,
            config: Some(
                serde_json::json!({"api_key":"test","index_host":"https://idx.svc.pinecone.io","operation":"bad"}),
            ),
        };
        let r = execute_pinecone(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── Together AI ───────────────────────────────────────────────────────────

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
}
