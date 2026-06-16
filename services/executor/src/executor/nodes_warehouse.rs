// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Data-warehouse / additional-database nodes: Snowflake and BigQuery over
//! their HTTP SQL APIs (caller-supplied bearer token, like the GCS/Vertex
//! nodes), plus MySQL via sqlx (mirrors the built-in Postgres `database` node).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
// sqlx's Column/Row traits are already in scope via `use super::*`.
use std::time::Duration;
use workflow_core::Node;

// ── MySQL (sqlx) ──────────────────────────────────────────────────────────────
pub(super) async fn execute_mysql(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let url = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => {
            return NodeExecutionResult::failed("MySQL requires 'url' (mysql://user:pass@host/db)")
        }
    };
    let query_str = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => return NodeExecutionResult::failed("MySQL requires 'query'"),
    };

    let pool = match sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => return NodeExecutionResult::failed(format!("MySQL connect error: {e}")),
    };

    let trimmed = query_str.trim().to_ascii_uppercase();
    let is_select = trimmed.starts_with("SELECT") || trimmed.starts_with("WITH");
    if is_select {
        match sqlx::query(&query_str).fetch_all(&pool).await {
            Ok(rows) => {
                let json_rows: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|row| {
                        let mut obj = serde_json::Map::new();
                        for (i, col) in row.columns().iter().enumerate() {
                            let val: serde_json::Value = row
                                .try_get::<i64, _>(i)
                                .map(|v| serde_json::json!(v))
                                .or_else(|_| row.try_get::<f64, _>(i).map(|v| serde_json::json!(v)))
                                .or_else(|_| {
                                    row.try_get::<bool, _>(i).map(|v| serde_json::json!(v))
                                })
                                .or_else(|_| {
                                    row.try_get::<String, _>(i).map(|v| serde_json::json!(v))
                                })
                                .unwrap_or(serde_json::Value::Null);
                            obj.insert(col.name().to_string(), val);
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                let count = json_rows.len();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "rows": json_rows, "count": count }).to_string(),
                )
            }
            Err(e) => NodeExecutionResult::failed(format!("MySQL query error: {e}")),
        }
    } else {
        match sqlx::query(&query_str).execute(&pool).await {
            Ok(result) => NodeExecutionResult::succeeded(
                serde_json::json!({ "rows_affected": result.rows_affected() }).to_string(),
            ),
            Err(e) => NodeExecutionResult::failed(format!("MySQL execute error: {e}")),
        }
    }
}

// ── Snowflake (SQL API v2) ────────────────────────────────────────────────────
pub(super) async fn execute_snowflake(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let account = match cfg.get("account").and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => {
            return NodeExecutionResult::failed("Snowflake requires 'account' (e.g. myorg-myacct)")
        }
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Snowflake requires 'token' (OAuth or key-pair JWT bearer)",
            )
        }
    };
    let statement = match cfg.get("statement").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Snowflake requires 'statement' (SQL)"),
    };
    let token_type = cfg
        .get("token_type")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("OAUTH")
        .to_string();

    let mut body = serde_json::json!({ "statement": statement, "timeout": 60 });
    for key in ["warehouse", "database", "schema", "role"] {
        if let Some(v) = cfg.get(key).and_then(|v| v.as_str()) {
            if !v.is_empty() {
                body[key] = serde_json::json!(v);
            }
        }
    }

    let url = format!("https://{account}.snowflakecomputing.com/api/v2/statements");
    match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("X-Snowflake-Authorization-Token-Type", token_type)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
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
        Err(e) => NodeExecutionResult::failed(format!("Snowflake request error: {e}")),
    }
}

// ── BigQuery (jobs.query REST) ────────────────────────────────────────────────
pub(super) async fn execute_bigquery(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let project = match cfg.get("project").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("BigQuery requires 'project'"),
    };
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "BigQuery requires 'access_token' (OAuth2 bearer for the bigquery scope)",
            )
        }
    };
    let query = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => return NodeExecutionResult::failed("BigQuery requires 'query'"),
    };
    let use_legacy = cfg
        .get("use_legacy_sql")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut body = serde_json::json!({ "query": query, "useLegacySql": use_legacy });
    if let Some(max) = cfg.get("max_results").and_then(|v| v.as_u64()) {
        body["maxResults"] = serde_json::json!(max);
    }
    if let Some(loc) = cfg.get("location").and_then(|v| v.as_str()) {
        if !loc.is_empty() {
            body["location"] = serde_json::json!(loc);
        }
    }

    let url = format!("https://bigquery.googleapis.com/bigquery/v2/projects/{project}/queries");
    match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {access_token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("BigQuery request error: {e}")),
    }
}

// ── Microsoft SQL Server (tiberius, pure Rust) ────────────────────────────────
pub(super) async fn execute_sqlserver(
    node: &Node,
    context: &ExecutionContext,
) -> NodeExecutionResult {
    use tiberius::{AuthMethod, Client, Config};
    use tokio::net::TcpStream;
    use tokio_util::compat::TokioAsyncWriteCompatExt;

    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let host = match cfg.get("host").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return NodeExecutionResult::failed("SQL Server requires 'host'"),
    };
    let user = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("SQL Server requires 'username'"),
    };
    let query_str = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => return NodeExecutionResult::failed("SQL Server requires 'query'"),
    };
    let port = cfg.get("port").and_then(|v| v.as_u64()).unwrap_or(1433) as u16;
    let pass = cfg
        .get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut config = Config::new();
    config.host(&host);
    config.port(port);
    config.authentication(AuthMethod::sql_server(&user, &pass));
    if let Some(db) = cfg.get("database").and_then(|v| v.as_str()) {
        if !db.is_empty() {
            config.database(db);
        }
    }
    // Accept self-signed certs (common for internal SQL Server instances).
    config.trust_cert();

    let tcp = match TcpStream::connect(config.get_addr()).await {
        Ok(t) => t,
        Err(e) => return NodeExecutionResult::failed(format!("SQL Server connect error: {e}")),
    };
    let _ = tcp.set_nodelay(true);
    let mut client = match Client::connect(config, tcp.compat_write()).await {
        Ok(c) => c,
        Err(e) => return NodeExecutionResult::failed(format!("SQL Server handshake error: {e}")),
    };

    let trimmed = query_str.trim().to_ascii_uppercase();
    let is_select = trimmed.starts_with("SELECT") || trimmed.starts_with("WITH");
    if is_select {
        let stream = match client.query(query_str.as_str(), &[]).await {
            Ok(s) => s,
            Err(e) => return NodeExecutionResult::failed(format!("SQL Server query error: {e}")),
        };
        let rows = match stream.into_first_result().await {
            Ok(r) => r,
            Err(e) => return NodeExecutionResult::failed(format!("SQL Server result error: {e}")),
        };
        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                let cols: Vec<String> =
                    row.columns().iter().map(|c| c.name().to_string()).collect();
                for (i, name) in cols.iter().enumerate() {
                    // Mismatched types return Err → treated as absent; first match wins.
                    let val: serde_json::Value = row
                        .try_get::<&str, _>(i)
                        .ok()
                        .flatten()
                        .map(|s| serde_json::json!(s))
                        .or_else(|| {
                            row.try_get::<i32, _>(i)
                                .ok()
                                .flatten()
                                .map(|n| serde_json::json!(n))
                        })
                        .or_else(|| {
                            row.try_get::<i64, _>(i)
                                .ok()
                                .flatten()
                                .map(|n| serde_json::json!(n))
                        })
                        .or_else(|| {
                            row.try_get::<f64, _>(i)
                                .ok()
                                .flatten()
                                .map(|n| serde_json::json!(n))
                        })
                        .or_else(|| {
                            row.try_get::<bool, _>(i)
                                .ok()
                                .flatten()
                                .map(|b| serde_json::json!(b))
                        })
                        .unwrap_or(serde_json::Value::Null);
                    obj.insert(name.clone(), val);
                }
                serde_json::Value::Object(obj)
            })
            .collect();
        let count = json_rows.len();
        NodeExecutionResult::succeeded(
            serde_json::json!({ "rows": json_rows, "count": count }).to_string(),
        )
    } else {
        match client.execute(query_str.as_str(), &[]).await {
            Ok(res) => NodeExecutionResult::succeeded(
                serde_json::json!({ "rows_affected": res.total() }).to_string(),
            ),
            Err(e) => NodeExecutionResult::failed(format!("SQL Server execute error: {e}")),
        }
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
    async fn mysql_requires_url() {
        let n = Node {
            id: "m1".into(),
            node_type: NodeType::Mysql,
            config: Some(serde_json::json!({"query":"SELECT 1"})),
        };
        let r = execute_mysql(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("url"));
    }

    #[tokio::test]
    async fn snowflake_requires_account() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s1".into(),
            node_type: NodeType::Snowflake,
            config: Some(serde_json::json!({"token":"t","statement":"select 1"})),
        };
        let r = execute_snowflake(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("account"));
    }

    #[tokio::test]
    async fn snowflake_requires_statement() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s2".into(),
            node_type: NodeType::Snowflake,
            config: Some(serde_json::json!({"account":"acct","token":"t"})),
        };
        let r = execute_snowflake(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("statement"));
    }

    #[tokio::test]
    async fn sqlserver_requires_host() {
        let n = Node {
            id: "ms1".into(),
            node_type: NodeType::Sqlserver,
            config: Some(serde_json::json!({"username":"sa","query":"SELECT 1"})),
        };
        let r = execute_sqlserver(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("host"));
    }

    #[tokio::test]
    async fn sqlserver_requires_query() {
        let n = Node {
            id: "ms2".into(),
            node_type: NodeType::Sqlserver,
            config: Some(serde_json::json!({"host":"db","username":"sa"})),
        };
        let r = execute_sqlserver(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn bigquery_requires_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "b1".into(),
            node_type: NodeType::Bigquery,
            config: Some(serde_json::json!({"project":"p","query":"SELECT 1"})),
        };
        let r = execute_bigquery(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }
}
