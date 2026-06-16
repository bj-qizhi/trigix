// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Database / warehouse nodes reachable over HTTP: MongoDB (Atlas Data API)
//! and ClickHouse (HTTP interface). Both return `{status, body}`.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── MongoDB (Atlas Data API) ──────────────────────────────────────────────────
pub(super) async fn execute_mongodb(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let base = match cfg.get("data_api_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "MongoDB requires 'data_api_url' (Atlas Data API endpoint base, e.g. https://<region>.data.mongodb-api.com/app/<app-id>/endpoint/data/v1)",
            )
        }
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("MongoDB requires 'api_key'"),
    };
    let data_source = match cfg.get("data_source").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "MongoDB requires 'data_source' (Atlas cluster name)",
            )
        }
    };
    let database = match cfg.get("database").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("MongoDB requires 'database'"),
    };
    let collection = match cfg.get("collection").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("MongoDB requires 'collection'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("find")
        .to_string();

    const ACTIONS: &[&str] = &[
        "find",
        "findOne",
        "insertOne",
        "insertMany",
        "updateOne",
        "updateMany",
        "deleteOne",
        "deleteMany",
        "aggregate",
    ];
    if !ACTIONS.contains(&operation.as_str()) {
        return NodeExecutionResult::failed(format!(
            "MongoDB unknown operation '{operation}' (expected one of {ACTIONS:?})"
        ));
    }

    let mut body = serde_json::json!({
        "dataSource": data_source,
        "database": database,
        "collection": collection,
    });
    // Operation-specific fields, all optional pass-throughs of caller JSON.
    let copy = |body: &mut serde_json::Value, key: &str| {
        if let Some(v) = cfg.get(key) {
            body[key] = json_array_or_parse(v);
        }
    };
    match operation.as_str() {
        "find" => {
            copy(&mut body, "filter");
            copy(&mut body, "projection");
            copy(&mut body, "sort");
            if let Some(limit) = cfg.get("limit").and_then(|v| v.as_u64()) {
                body["limit"] = serde_json::json!(limit);
            }
            if let Some(skip) = cfg.get("skip").and_then(|v| v.as_u64()) {
                body["skip"] = serde_json::json!(skip);
            }
        }
        "findOne" => {
            copy(&mut body, "filter");
            copy(&mut body, "projection");
        }
        "insertOne" => copy(&mut body, "document"),
        "insertMany" => copy(&mut body, "documents"),
        "updateOne" | "updateMany" => {
            copy(&mut body, "filter");
            copy(&mut body, "update");
            if let Some(upsert) = cfg.get("upsert").and_then(|v| v.as_bool()) {
                body["upsert"] = serde_json::json!(upsert);
            }
        }
        "deleteOne" | "deleteMany" => copy(&mut body, "filter"),
        "aggregate" => copy(&mut body, "pipeline"),
        _ => {}
    }

    let url = format!("{base}/action/{operation}");
    match http_client
        .post(&url)
        .header("api-key", &api_key)
        .header("Content-Type", "application/json")
        .header("Access-Control-Request-Headers", "*")
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
        Err(e) => NodeExecutionResult::failed(format!("MongoDB {operation} error: {e}")),
    }
}

// ── ClickHouse (HTTP interface) ───────────────────────────────────────────────
pub(super) async fn execute_clickhouse(
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
                "ClickHouse requires 'host' (e.g. https://abc.clickhouse.cloud:8443)",
            )
        }
    };
    let query = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.trim().is_empty() => q.trim().to_string(),
        _ => return NodeExecutionResult::failed("ClickHouse requires 'query' (SQL)"),
    };
    let user = cfg
        .get("user")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    let password = cfg
        .get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let format = cfg
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("JSON")
        .to_string();

    // Append a FORMAT clause for SELECT-style queries unless the caller already
    // specified one. DDL/DML (INSERT/CREATE/…) are sent verbatim.
    let upper = query.to_uppercase();
    let sql = if upper.contains("FORMAT ")
        || upper.starts_with("INSERT")
        || upper.starts_with("CREATE")
        || upper.starts_with("ALTER")
        || upper.starts_with("DROP")
    {
        query.clone()
    } else {
        format!("{query} FORMAT {format}")
    };

    let mut req = http_client
        .post(&host)
        .header("X-ClickHouse-User", &user)
        .body(sql);
    if !password.is_empty() {
        req = req.header("X-ClickHouse-Key", &password);
    }
    if let Some(db) = cfg.get("database").and_then(|v| v.as_str()) {
        if !db.is_empty() {
            req = req.query(&[("database", db)]);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            // ClickHouse JSON/JSONEachRow responses parse cleanly; fall back to raw text.
            let body = serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or(serde_json::Value::String(text));
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("ClickHouse query error: {e}")),
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
    async fn mongodb_requires_data_api_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m1".into(),
            node_type: NodeType::Mongodb,
            config: Some(serde_json::json!({"operation":"find"})),
        };
        let r = execute_mongodb(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("data_api_url"));
    }

    #[tokio::test]
    async fn mongodb_requires_collection() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m2".into(),
            node_type: NodeType::Mongodb,
            config: Some(serde_json::json!({
                "data_api_url":"https://x.data.mongodb-api.com/app/a/endpoint/data/v1",
                "api_key":"k","data_source":"Cluster0","database":"db"
            })),
        };
        let r = execute_mongodb(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("collection"));
    }

    #[tokio::test]
    async fn mongodb_rejects_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "m3".into(),
            node_type: NodeType::Mongodb,
            config: Some(serde_json::json!({
                "data_api_url":"https://x.data.mongodb-api.com/app/a/endpoint/data/v1",
                "api_key":"k","data_source":"Cluster0","database":"db","collection":"c",
                "operation":"nope"
            })),
        };
        let r = execute_mongodb(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    #[tokio::test]
    async fn clickhouse_requires_host() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ch1".into(),
            node_type: NodeType::Clickhouse,
            config: Some(serde_json::json!({"query":"SELECT 1"})),
        };
        let r = execute_clickhouse(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn clickhouse_requires_query() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "ch2".into(),
            node_type: NodeType::Clickhouse,
            config: Some(serde_json::json!({"host":"https://x.clickhouse.cloud:8443"})),
        };
        let r = execute_clickhouse(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }
}
