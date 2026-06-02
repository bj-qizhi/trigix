// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::{collections::HashMap, future::Future, sync::Arc};

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    pub webhook_token: String,
    pub tenant_id: String,
    pub delivered_at: i64,
    pub status_code: Option<i32>,
    pub success: bool,
    pub error_message: Option<String>,
    pub execution_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRecord {
    pub token: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Optional condition expression. Syntax: `field.path == "value"` or `field.path != "value"`.
    /// Dot-separated path is extracted from the JSON payload. If set, webhooks that don't match are rejected 200 (no execution).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_expr: Option<String>,
    /// Maximum webhook calls allowed per minute (in-memory sliding window). None = unlimited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_calls_per_minute: Option<u32>,
    /// When true, the webhook rejects all incoming requests with 503.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
    /// Optional Rhai script to transform the incoming payload. `payload` variable holds the JSON object;
    /// script should return a new JSON string or object. On error the original payload is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_transform_script: Option<String>,
}

/// Evaluate a simple condition expression against a JSON payload.
/// Syntax: `a.b.c == "str"` | `a.b.c != "str"` | `a.b.c > 5` | `a.b.c < 5`
pub fn eval_condition(expr: &str, payload: &serde_json::Value) -> bool {
    let eq_idx = expr.find(" == ").or_else(|| expr.find(" != ")).or_else(|| expr.find(" > ")).or_else(|| expr.find(" < "));
    let (path_str, op, rhs_str) = if let Some(idx) = expr.find(" == ") {
        (&expr[..idx], "==", expr[idx + 4..].trim())
    } else if let Some(idx) = expr.find(" != ") {
        (&expr[..idx], "!=", expr[idx + 4..].trim())
    } else if let Some(idx) = expr.find(" > ") {
        (&expr[..idx], ">", expr[idx + 3..].trim())
    } else if let Some(idx) = expr.find(" < ") {
        (&expr[..idx], "<", expr[idx + 3..].trim())
    } else {
        return true; // unparseable → pass
    };
    let _ = eq_idx;
    let path_parts: Vec<&str> = path_str.trim().split('.').collect();
    let mut current = payload;
    for part in &path_parts {
        current = match current.get(part) {
            Some(v) => v,
            None => return false,
        };
    }
    let rhs_unquoted = rhs_str.trim_matches('"').trim_matches('\'');
    match op {
        "==" => {
            if let Some(s) = current.as_str() { s == rhs_unquoted }
            else if let (Some(n), Ok(r)) = (current.as_f64(), rhs_unquoted.parse::<f64>()) { (n - r).abs() < 1e-9 }
            else if let (Some(b), Ok(r)) = (current.as_bool(), rhs_unquoted.parse::<bool>()) { b == r }
            else { false }
        }
        "!=" => {
            if let Some(s) = current.as_str() { s != rhs_unquoted }
            else if let (Some(n), Ok(r)) = (current.as_f64(), rhs_unquoted.parse::<f64>()) { (n - r).abs() >= 1e-9 }
            else { true }
        }
        ">" => {
            if let (Some(n), Ok(r)) = (current.as_f64(), rhs_unquoted.parse::<f64>()) { n > r } else { false }
        }
        "<" => {
            if let (Some(n), Ok(r)) = (current.as_f64(), rhs_unquoted.parse::<f64>()) { n < r } else { false }
        }
        _ => true,
    }
}

/// Apply an optional Rhai transform script to a JSON payload string.
/// The script receives `payload` as a Dynamic (object map). It should return a new value.
/// On any error, returns the original payload unchanged.
pub fn apply_payload_transform(script: &str, payload_json: &str) -> String {
    use rhai::{Engine, Scope};
    let Ok(val) = serde_json::from_str::<serde_json::Value>(payload_json) else {
        return payload_json.to_string();
    };
    let engine = Engine::new();
    let mut scope = Scope::new();
    let dynamic_val = json_to_dynamic(&val);
    scope.push("payload", dynamic_val);
    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script) {
        Ok(result) => {
            if let Ok(s) = serde_json::to_string(&dynamic_to_json(&result)) {
                s
            } else {
                payload_json.to_string()
            }
        }
        Err(_) => payload_json.to_string(),
    }
}

fn json_to_dynamic(val: &serde_json::Value) -> rhai::Dynamic {
    match val {
        serde_json::Value::Null => rhai::Dynamic::UNIT,
        serde_json::Value::Bool(b) => rhai::Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { rhai::Dynamic::from(i) }
            else { rhai::Dynamic::from(n.as_f64().unwrap_or(0.0)) }
        }
        serde_json::Value::String(s) => rhai::Dynamic::from(s.clone()),
        serde_json::Value::Array(arr) => {
            let v: rhai::Array = arr.iter().map(json_to_dynamic).collect();
            rhai::Dynamic::from(v)
        }
        serde_json::Value::Object(map) => {
            let mut m = rhai::Map::new();
            for (k, v) in map {
                m.insert(k.clone().into(), json_to_dynamic(v));
            }
            rhai::Dynamic::from(m)
        }
    }
}

fn dynamic_to_json(val: &rhai::Dynamic) -> serde_json::Value {
    if val.is_unit() { return serde_json::Value::Null; }
    if let Some(b) = val.as_bool().ok() { return serde_json::Value::Bool(b); }
    if let Some(i) = val.as_int().ok() { return serde_json::json!(i); }
    if let Some(f) = val.as_float().ok() { return serde_json::json!(f); }
    if let Some(s) = val.clone().try_cast::<String>() { return serde_json::Value::String(s); }
    if let Some(arr) = val.clone().try_cast::<rhai::Array>() {
        return serde_json::Value::Array(arr.iter().map(dynamic_to_json).collect());
    }
    if let Some(map) = val.clone().try_cast::<rhai::Map>() {
        let obj: serde_json::Map<_, _> = map.iter().map(|(k, v)| (k.to_string(), dynamic_to_json(v))).collect();
        return serde_json::Value::Object(obj);
    }
    serde_json::Value::Null
}

#[derive(Debug)]
pub enum WebhookError {
    NotFound,
    StoreUnavailable,
}

pub trait WebhookStore: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        record: WebhookRecord,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;
    fn get_by_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;
    fn get_by_version(
        &self,
        workflow_version_id: &str,
    ) -> impl Future<Output = Result<Option<WebhookRecord>, WebhookError>> + Send;
    fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> impl Future<Output = Result<Vec<WebhookRecord>, WebhookError>> + Send;
    fn delete_by_token(
        &self,
        tenant_id: &str,
        token: &str,
    ) -> impl Future<Output = Result<(), WebhookError>> + Send;
    fn record_delivery(
        &self,
        delivery: WebhookDelivery,
    ) -> impl Future<Output = ()> + Send;
    fn list_deliveries(
        &self,
        webhook_token: &str,
        limit: i64,
    ) -> impl Future<Output = Vec<WebhookDelivery>> + Send;
    fn set_condition(
        &self,
        tenant_id: &str,
        token: &str,
        condition_expr: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;

    fn set_rate_limit(
        &self,
        tenant_id: &str,
        token: &str,
        max_calls_per_minute: Option<u32>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;

    fn set_paused(
        &self,
        tenant_id: &str,
        token: &str,
        paused: bool,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;

    fn rotate_secret(
        &self,
        tenant_id: &str,
        token: &str,
        new_secret: String,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;

    fn set_payload_transform(
        &self,
        tenant_id: &str,
        token: &str,
        script: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;
}

#[derive(Clone, Default)]
pub struct MemoryWebhookStore {
    by_token: Arc<Mutex<HashMap<String, WebhookRecord>>>,
    by_version: Arc<Mutex<HashMap<String, String>>>,
    deliveries: Arc<Mutex<Vec<WebhookDelivery>>>,
}

impl WebhookStore for MemoryWebhookStore {
    fn upsert(
        &self,
        record: WebhookRecord,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let by_version = self.by_version.clone();
        async move {
            by_token
                .lock()
                .await
                .insert(record.token.clone(), record.clone());
            by_version
                .lock()
                .await
                .insert(record.workflow_version_id.clone(), record.token.clone());
            Ok(record)
        }
    }

    fn get_by_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let token = token.to_string();
        async move {
            by_token
                .lock()
                .await
                .get(&token)
                .cloned()
                .ok_or(WebhookError::NotFound)
        }
    }

    fn get_by_version(
        &self,
        workflow_version_id: &str,
    ) -> impl Future<Output = Result<Option<WebhookRecord>, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let by_version = self.by_version.clone();
        let version_id = workflow_version_id.to_string();
        async move {
            let versions = by_version.lock().await;
            let Some(token) = versions.get(&version_id).cloned() else {
                return Ok(None);
            };
            drop(versions);
            Ok(by_token.lock().await.get(&token).cloned())
        }
    }

    fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> impl Future<Output = Result<Vec<WebhookRecord>, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let tenant_id = tenant_id.to_string();
        async move {
            let mut records: Vec<WebhookRecord> = by_token.lock().await.values()
                .filter(|r| r.tenant_id == tenant_id)
                .cloned()
                .collect();
            records.sort_by(|a, b| a.workflow_version_id.cmp(&b.workflow_version_id));
            Ok(records)
        }
    }

    fn delete_by_token(
        &self,
        tenant_id: &str,
        token: &str,
    ) -> impl Future<Output = Result<(), WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let by_version = self.by_version.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let mut bt = by_token.lock().await;
            let record = bt.get(&token).cloned().ok_or(WebhookError::NotFound)?;
            if record.tenant_id != tenant_id {
                return Err(WebhookError::NotFound);
            }
            bt.remove(&token);
            drop(bt);
            by_version.lock().await.remove(&record.workflow_version_id);
            Ok(())
        }
    }

    fn record_delivery(
        &self,
        delivery: WebhookDelivery,
    ) -> impl Future<Output = ()> + Send {
        let deliveries = self.deliveries.clone();
        async move {
            deliveries.lock().await.push(delivery);
        }
    }

    fn list_deliveries(
        &self,
        webhook_token: &str,
        limit: i64,
    ) -> impl Future<Output = Vec<WebhookDelivery>> + Send {
        let deliveries = self.deliveries.clone();
        let token = webhook_token.to_string();
        async move {
            let all = deliveries.lock().await;
            let mut result: Vec<WebhookDelivery> = all.iter()
                .filter(|d| d.webhook_token == token)
                .cloned()
                .collect();
            result.sort_by(|a, b| b.delivered_at.cmp(&a.delivered_at));
            result.truncate(limit as usize);
            result
        }
    }

    fn set_condition(
        &self,
        tenant_id: &str,
        token: &str,
        condition_expr: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let mut map = by_token.lock().await;
            let record = map.get_mut(&token).filter(|r| r.tenant_id == tenant_id)
                .ok_or(WebhookError::NotFound)?;
            record.condition_expr = condition_expr;
            Ok(record.clone())
        }
    }

    fn set_rate_limit(
        &self,
        tenant_id: &str,
        token: &str,
        max_calls_per_minute: Option<u32>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let mut map = by_token.lock().await;
            let record = map.get_mut(&token).filter(|r| r.tenant_id == tenant_id)
                .ok_or(WebhookError::NotFound)?;
            record.max_calls_per_minute = max_calls_per_minute;
            Ok(record.clone())
        }
    }

    fn set_paused(
        &self,
        tenant_id: &str,
        token: &str,
        paused: bool,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let mut map = by_token.lock().await;
            let record = map.get_mut(&token).filter(|r| r.tenant_id == tenant_id)
                .ok_or(WebhookError::NotFound)?;
            record.paused = paused;
            Ok(record.clone())
        }
    }

    fn rotate_secret(
        &self,
        tenant_id: &str,
        token: &str,
        new_secret: String,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let mut map = by_token.lock().await;
            let record = map.get_mut(&token).filter(|r| r.tenant_id == tenant_id)
                .ok_or(WebhookError::NotFound)?;
            record.secret = Some(new_secret);
            Ok(record.clone())
        }
    }

    fn set_payload_transform(
        &self,
        tenant_id: &str,
        token: &str,
        script: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let mut map = by_token.lock().await;
            let record = map.get_mut(&token).filter(|r| r.tenant_id == tenant_id)
                .ok_or(WebhookError::NotFound)?;
            record.payload_transform_script = script;
            Ok(record.clone())
        }
    }
}

/// Verify an HMAC-SHA256 webhook signature.
/// The expected header format is `sha256=<hex_digest>`.
pub fn verify_signature(secret: &str, body: &[u8], header: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let expected_hex = header.strip_prefix("sha256=").unwrap_or(header);
    let Ok(expected_bytes) = hex::decode(expected_hex) else {
        return false;
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(body);
    let result = mac.finalize().into_bytes();
    // Constant-time comparison
    result.as_slice() == expected_bytes.as_slice()
}

// ── Postgres implementation ──────────────────────────────────────────────────

#[derive(Clone)]
pub struct PostgresWebhookStore {
    pool: PgPool,
}

impl PostgresWebhookStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl WebhookStore for PostgresWebhookStore {
    fn upsert(
        &self,
        record: WebhookRecord,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let pool = self.pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO af_webhooks (token, tenant_id, workflow_id, workflow_version_id, secret)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (workflow_version_id)
                 DO UPDATE SET token = EXCLUDED.token, secret = EXCLUDED.secret",
            )
            .bind(&record.token)
            .bind(&record.tenant_id)
            .bind(&record.workflow_id)
            .bind(&record.workflow_version_id)
            .bind(&record.secret)
            .execute(&pool)
            .await
            .map_err(|_| WebhookError::StoreUnavailable)?;
            Ok(record)
        }
    }

    fn get_by_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let pool = self.pool.clone();
        let token = token.to_string();
        async move {
            sqlx::query_as::<_, WebhookRow>(
                "SELECT token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script
                 FROM af_webhooks WHERE token = $1",
            )
            .bind(&token)
            .fetch_optional(&pool)
            .await
            .map_err(|_| WebhookError::StoreUnavailable)?
            .map(WebhookRow::into_record)
            .ok_or(WebhookError::NotFound)
        }
    }

    fn get_by_version(
        &self,
        workflow_version_id: &str,
    ) -> impl Future<Output = Result<Option<WebhookRecord>, WebhookError>> + Send {
        let pool = self.pool.clone();
        let version_id = workflow_version_id.to_string();
        async move {
            let row = sqlx::query_as::<_, WebhookRow>(
                "SELECT token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script
                 FROM af_webhooks WHERE workflow_version_id = $1",
            )
            .bind(&version_id)
            .fetch_optional(&pool)
            .await
            .map_err(|_| WebhookError::StoreUnavailable)?;
            Ok(row.map(WebhookRow::into_record))
        }
    }

    fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> impl Future<Output = Result<Vec<WebhookRecord>, WebhookError>> + Send {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_string();
        async move {
            let rows = sqlx::query_as::<_, WebhookRow>(
                "SELECT token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script
                 FROM af_webhooks WHERE tenant_id = $1
                 ORDER BY workflow_version_id",
            )
            .bind(&tenant_id)
            .fetch_all(&pool)
            .await
            .map_err(|_| WebhookError::StoreUnavailable)?;
            Ok(rows.into_iter().map(WebhookRow::into_record).collect())
        }
    }

    fn delete_by_token(
        &self,
        tenant_id: &str,
        token: &str,
    ) -> impl Future<Output = Result<(), WebhookError>> + Send {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let rows = sqlx::query(
                "DELETE FROM af_webhooks WHERE token = $1 AND tenant_id = $2",
            )
            .bind(&token)
            .bind(&tenant_id)
            .execute(&pool)
            .await
            .map_err(|_| WebhookError::StoreUnavailable)?;
            if rows.rows_affected() == 0 { Err(WebhookError::NotFound) } else { Ok(()) }
        }
    }

    fn record_delivery(
        &self,
        delivery: WebhookDelivery,
    ) -> impl Future<Output = ()> + Send {
        let pool = self.pool.clone();
        async move {
            let _ = sqlx::query(
                "INSERT INTO af_webhook_deliveries
                 (id, webhook_token, tenant_id, delivered_at, status_code, success, error_message, execution_id)
                 VALUES ($1, $2, $3, to_timestamp($4), $5, $6, $7, $8)",
            )
            .bind(&delivery.id)
            .bind(&delivery.webhook_token)
            .bind(&delivery.tenant_id)
            .bind(delivery.delivered_at)
            .bind(delivery.status_code)
            .bind(delivery.success)
            .bind(&delivery.error_message)
            .bind(&delivery.execution_id)
            .execute(&pool)
            .await;
        }
    }

    fn list_deliveries(
        &self,
        webhook_token: &str,
        limit: i64,
    ) -> impl Future<Output = Vec<WebhookDelivery>> + Send {
        let pool = self.pool.clone();
        let token = webhook_token.to_string();
        async move {
            sqlx::query_as::<_, DeliveryRow>(
                "SELECT id, webhook_token, tenant_id,
                        EXTRACT(EPOCH FROM delivered_at)::BIGINT AS delivered_at,
                        status_code, success, error_message, execution_id
                 FROM af_webhook_deliveries
                 WHERE webhook_token = $1
                 ORDER BY delivered_at DESC
                 LIMIT $2",
            )
            .bind(&token)
            .bind(limit)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(DeliveryRow::into_delivery)
            .collect()
        }
    }

    fn set_condition(
        &self,
        tenant_id: &str,
        token: &str,
        condition_expr: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let rows = sqlx::query_as::<_, WebhookRow>(
                "UPDATE af_webhooks SET condition_expr=$1 WHERE token=$2 AND tenant_id=$3
                 RETURNING token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script"
            )
            .bind(&condition_expr)
            .bind(&token)
            .bind(&tenant_id)
            .fetch_all(&pool).await.map_err(|_| WebhookError::StoreUnavailable)?;
            rows.into_iter().next().map(WebhookRow::into_record).ok_or(WebhookError::NotFound)
        }
    }

    fn set_rate_limit(
        &self,
        tenant_id: &str,
        token: &str,
        max_calls_per_minute: Option<u32>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let rows = sqlx::query_as::<_, WebhookRow>(
                "UPDATE af_webhooks SET max_calls_per_minute=$1 WHERE token=$2 AND tenant_id=$3
                 RETURNING token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script"
            )
            .bind(max_calls_per_minute.map(|v| v as i32))
            .bind(&token)
            .bind(&tenant_id)
            .fetch_all(&pool).await.map_err(|_| WebhookError::StoreUnavailable)?;
            rows.into_iter().next().map(WebhookRow::into_record).ok_or(WebhookError::NotFound)
        }
    }

    fn set_paused(
        &self,
        tenant_id: &str,
        token: &str,
        paused: bool,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let rows = sqlx::query_as::<_, WebhookRow>(
                "UPDATE af_webhooks SET paused=$1 WHERE token=$2 AND tenant_id=$3
                 RETURNING token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script"
            )
            .bind(paused)
            .bind(&token)
            .bind(&tenant_id)
            .fetch_all(&pool).await.map_err(|_| WebhookError::StoreUnavailable)?;
            rows.into_iter().next().map(WebhookRow::into_record).ok_or(WebhookError::NotFound)
        }
    }

    fn rotate_secret(
        &self,
        tenant_id: &str,
        token: &str,
        new_secret: String,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let rows = sqlx::query_as::<_, WebhookRow>(
                "UPDATE af_webhooks SET secret=$1 WHERE token=$2 AND tenant_id=$3
                 RETURNING token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script"
            )
            .bind(&new_secret)
            .bind(&token)
            .bind(&tenant_id)
            .fetch_all(&pool).await.map_err(|_| WebhookError::StoreUnavailable)?;
            rows.into_iter().next().map(WebhookRow::into_record).ok_or(WebhookError::NotFound)
        }
    }

    fn set_payload_transform(
        &self,
        tenant_id: &str,
        token: &str,
        script: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let pool = self.pool.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            let rows = sqlx::query_as::<_, WebhookRow>(
                "UPDATE af_webhooks SET payload_transform_script=$1 WHERE token=$2 AND tenant_id=$3
                 RETURNING token, tenant_id, workflow_id, workflow_version_id, secret, condition_expr, max_calls_per_minute, paused, payload_transform_script"
            )
            .bind(script)
            .bind(&token)
            .bind(&tenant_id)
            .fetch_all(&pool).await.map_err(|_| WebhookError::StoreUnavailable)?;
            rows.into_iter().next().map(WebhookRow::into_record).ok_or(WebhookError::NotFound)
        }
    }
}

#[derive(sqlx::FromRow)]
struct WebhookRow {
    token: String,
    tenant_id: String,
    workflow_id: String,
    workflow_version_id: String,
    secret: Option<String>,
    #[sqlx(default)]
    condition_expr: Option<String>,
    #[sqlx(default)]
    max_calls_per_minute: Option<i32>,
    #[sqlx(default)]
    paused: bool,
    #[sqlx(default)]
    payload_transform_script: Option<String>,
}

impl WebhookRow {
    fn into_record(self) -> WebhookRecord {
        WebhookRecord {
            token: self.token,
            tenant_id: self.tenant_id,
            workflow_id: self.workflow_id,
            workflow_version_id: self.workflow_version_id,
            secret: self.secret,
            condition_expr: self.condition_expr,
            max_calls_per_minute: self.max_calls_per_minute.map(|v| v as u32),
            paused: self.paused,
            payload_transform_script: self.payload_transform_script,
        }
    }
}

#[derive(sqlx::FromRow)]
struct DeliveryRow {
    id: String,
    webhook_token: String,
    tenant_id: String,
    delivered_at: i64,
    status_code: Option<i32>,
    success: bool,
    error_message: Option<String>,
    execution_id: Option<String>,
}

impl DeliveryRow {
    fn into_delivery(self) -> WebhookDelivery {
        WebhookDelivery {
            id: self.id,
            webhook_token: self.webhook_token,
            tenant_id: self.tenant_id,
            delivered_at: self.delivered_at,
            status_code: self.status_code,
            success: self.success,
            error_message: self.error_message,
            execution_id: self.execution_id,
        }
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformWebhookStore {
    Memory(MemoryWebhookStore),
    Postgres(PostgresWebhookStore),
}

impl Default for PlatformWebhookStore {
    fn default() -> Self {
        Self::Memory(MemoryWebhookStore::default())
    }
}

impl PlatformWebhookStore {
    pub fn postgres(store: PostgresWebhookStore) -> Self {
        Self::Postgres(store)
    }
}

impl WebhookStore for PlatformWebhookStore {
    fn upsert(
        &self,
        record: WebhookRecord,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Memory(s) => s.upsert(record).await,
                Self::Postgres(s) => s.upsert(record).await,
            }
        }
    }

    fn get_by_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let this = self.clone();
        let token = token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.get_by_token(&token).await,
                Self::Postgres(s) => s.get_by_token(&token).await,
            }
        }
    }

    fn get_by_version(
        &self,
        workflow_version_id: &str,
    ) -> impl Future<Output = Result<Option<WebhookRecord>, WebhookError>> + Send {
        let this = self.clone();
        let version_id = workflow_version_id.to_string();
        async move {
            match this {
                Self::Memory(s) => s.get_by_version(&version_id).await,
                Self::Postgres(s) => s.get_by_version(&version_id).await,
            }
        }
    }

    fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> impl Future<Output = Result<Vec<WebhookRecord>, WebhookError>> + Send {
        let this = self.clone();
        let tenant_id = tenant_id.to_string();
        async move {
            match this {
                Self::Memory(s) => s.list_by_tenant(&tenant_id).await,
                Self::Postgres(s) => s.list_by_tenant(&tenant_id).await,
            }
        }
    }

    fn delete_by_token(
        &self,
        tenant_id: &str,
        token: &str,
    ) -> impl Future<Output = Result<(), WebhookError>> + Send {
        let this = self.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.delete_by_token(&tenant_id, &token).await,
                Self::Postgres(s) => s.delete_by_token(&tenant_id, &token).await,
            }
        }
    }

    fn record_delivery(
        &self,
        delivery: WebhookDelivery,
    ) -> impl Future<Output = ()> + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Memory(s) => s.record_delivery(delivery).await,
                Self::Postgres(s) => s.record_delivery(delivery).await,
            }
        }
    }

    fn list_deliveries(
        &self,
        webhook_token: &str,
        limit: i64,
    ) -> impl Future<Output = Vec<WebhookDelivery>> + Send {
        let this = self.clone();
        let token = webhook_token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.list_deliveries(&token, limit).await,
                Self::Postgres(s) => s.list_deliveries(&token, limit).await,
            }
        }
    }

    fn set_condition(
        &self,
        tenant_id: &str,
        token: &str,
        condition_expr: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let this = self.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.set_condition(&tenant_id, &token, condition_expr).await,
                Self::Postgres(s) => s.set_condition(&tenant_id, &token, condition_expr).await,
            }
        }
    }

    fn set_rate_limit(
        &self,
        tenant_id: &str,
        token: &str,
        max_calls_per_minute: Option<u32>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let this = self.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.set_rate_limit(&tenant_id, &token, max_calls_per_minute).await,
                Self::Postgres(s) => s.set_rate_limit(&tenant_id, &token, max_calls_per_minute).await,
            }
        }
    }

    fn set_paused(
        &self,
        tenant_id: &str,
        token: &str,
        paused: bool,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let this = self.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.set_paused(&tenant_id, &token, paused).await,
                Self::Postgres(s) => s.set_paused(&tenant_id, &token, paused).await,
            }
        }
    }

    fn rotate_secret(
        &self,
        tenant_id: &str,
        token: &str,
        new_secret: String,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let this = self.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.rotate_secret(&tenant_id, &token, new_secret).await,
                Self::Postgres(s) => s.rotate_secret(&tenant_id, &token, new_secret).await,
            }
        }
    }

    fn set_payload_transform(
        &self,
        tenant_id: &str,
        token: &str,
        script: Option<String>,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let this = self.clone();
        let tenant_id = tenant_id.to_string();
        let token = token.to_string();
        async move {
            match this {
                Self::Memory(s) => s.set_payload_transform(&tenant_id, &token, script).await,
                Self::Postgres(s) => s.set_payload_transform(&tenant_id, &token, script).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_signature_accepts_valid_hmac() {
        // Pre-computed: echo -n "hello" | openssl dgst -sha256 -hmac "mysecret"
        let secret = "mysecret";
        let body = b"hello";
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize().into_bytes();
        let sig = format!("sha256={}", hex::encode(result));
        assert!(verify_signature(secret, body, &sig));
    }

    #[test]
    fn verify_signature_rejects_wrong_secret() {
        let body = b"hello";
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(b"mysecret").unwrap();
        mac.update(body);
        let result = mac.finalize().into_bytes();
        let sig = format!("sha256={}", hex::encode(result));
        assert!(!verify_signature("wrongsecret", body, &sig));
    }

    #[test]
    fn verify_signature_rejects_invalid_header() {
        assert!(!verify_signature("secret", b"body", "not-a-signature"));
    }
}
