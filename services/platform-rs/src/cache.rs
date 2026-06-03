// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::time::Duration;

use redis::aio::ConnectionManager;
use redis::AsyncCommands;

/// Optional Redis cache. When REDIS_URL is not set, all operations are no-ops.
#[derive(Clone)]
pub enum CacheClient {
    Redis(ConnectionManager),
    Noop,
}

impl CacheClient {
    /// Connect using REDIS_URL env var. Falls back to Noop on missing/invalid URL.
    pub async fn from_env() -> Self {
        match std::env::var("REDIS_URL") {
            Ok(url) => match Self::connect(&url).await {
                Ok(client) => {
                    tracing::info!(url = %url, "Redis cache connected");
                    client
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Redis connection failed, running without cache");
                    Self::Noop
                }
            },
            Err(_) => Self::Noop,
        }
    }

    async fn connect(url: &str) -> redis::RedisResult<Self> {
        let client = redis::Client::open(url)?;
        let mgr = ConnectionManager::new(client).await?;
        Ok(Self::Redis(mgr))
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Redis(_))
    }

    /// Get a string value. Returns None on cache miss or Noop.
    pub async fn get(&self, key: &str) -> Option<String> {
        match self {
            Self::Noop => None,
            Self::Redis(conn) => {
                let mut c = conn.clone();
                c.get::<_, Option<String>>(key).await.ok().flatten()
            }
        }
    }

    /// Set a string value with an optional TTL.
    pub async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) {
        match self {
            Self::Noop => {}
            Self::Redis(conn) => {
                let mut c = conn.clone();
                match ttl {
                    Some(d) => {
                        let _ = c.set_ex::<_, _, ()>(key, value, d.as_secs()).await;
                    }
                    None => {
                        let _ = c.set::<_, _, ()>(key, value).await;
                    }
                }
            }
        }
    }

    /// Delete a key.
    pub async fn del(&self, key: &str) {
        match self {
            Self::Noop => {}
            Self::Redis(conn) => {
                let mut c = conn.clone();
                let _ = c.del::<_, ()>(key).await;
            }
        }
    }

    /// Publish an event to a Redis channel (for multi-instance SSE fanout).
    pub async fn publish(&self, channel: &str, message: &str) {
        match self {
            Self::Noop => {}
            Self::Redis(conn) => {
                let mut c = conn.clone();
                let _ = c.publish::<_, _, ()>(channel, message).await;
            }
        }
    }

    // ── Redis Streams (for distributed execution queue) ───────────────────────

    /// XADD — append a message to a stream. Returns the message ID or None on error/Noop.
    pub async fn xadd(&self, stream: &str, fields: &[(&str, &str)]) -> Option<String> {
        match self {
            Self::Noop => None,
            Self::Redis(conn) => {
                let mut c = conn.clone();
                let mut cmd = redis::cmd("XADD");
                cmd.arg(stream).arg("*");
                for (k, v) in fields {
                    cmd.arg(k).arg(v);
                }
                cmd.query_async::<String>(&mut c).await.ok()
            }
        }
    }

    /// XGROUP CREATE stream group $ MKSTREAM — idempotent (ignores BUSYGROUP error).
    pub async fn xgroup_create_mkstream(&self, stream: &str, group: &str) {
        if let Self::Redis(conn) = self {
            let mut c = conn.clone();
            let _: redis::RedisResult<()> = redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(stream)
                .arg(group)
                .arg("$")
                .arg("MKSTREAM")
                .query_async(&mut c)
                .await;
        }
    }

    /// XREADGROUP GROUP group consumer COUNT n BLOCK timeout_ms STREAMS stream >
    /// Returns a list of (message_id, fields) pairs.
    pub async fn xreadgroup(
        &self,
        stream: &str,
        group: &str,
        consumer: &str,
        count: usize,
        block_ms: u64,
    ) -> Vec<(String, Vec<(String, String)>)> {
        match self {
            Self::Noop => vec![],
            Self::Redis(conn) => {
                let mut c = conn.clone();
                let result: redis::RedisResult<redis::Value> = redis::cmd("XREADGROUP")
                    .arg("GROUP")
                    .arg(group)
                    .arg(consumer)
                    .arg("COUNT")
                    .arg(count)
                    .arg("BLOCK")
                    .arg(block_ms)
                    .arg("STREAMS")
                    .arg(stream)
                    .arg(">")
                    .query_async(&mut c)
                    .await;

                parse_xreadgroup_response(result)
            }
        }
    }

    /// XLEN stream — return number of messages in stream. Returns None on Noop/error.
    pub async fn xlen(&self, stream: &str) -> Option<u64> {
        match self {
            Self::Noop => None,
            Self::Redis(conn) => {
                let mut c = conn.clone();
                redis::cmd("XLEN")
                    .arg(stream)
                    .query_async::<u64>(&mut c)
                    .await
                    .ok()
            }
        }
    }

    /// XACK stream group id — acknowledge a processed message.
    pub async fn xack(&self, stream: &str, group: &str, msg_id: &str) {
        if let Self::Redis(conn) = self {
            let mut c = conn.clone();
            let _: redis::RedisResult<()> = redis::cmd("XACK")
                .arg(stream)
                .arg(group)
                .arg(msg_id)
                .query_async(&mut c)
                .await;
        }
    }
}

/// Parse the nested XREADGROUP response into flat (id, fields) pairs.
fn parse_xreadgroup_response(
    result: redis::RedisResult<redis::Value>,
) -> Vec<(String, Vec<(String, String)>)> {
    let val = match result {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    // Response shape: [[stream_name, [[msg_id, [f1,v1,f2,v2,...]], ...]]]
    let streams = match val {
        redis::Value::Array(arr) => arr,
        _ => return vec![],
    };
    let mut out = vec![];
    for stream_entry in streams {
        let parts = match stream_entry {
            redis::Value::Array(p) => p,
            _ => continue,
        };
        if parts.len() < 2 {
            continue;
        }
        let messages = match &parts[1] {
            redis::Value::Array(msgs) => msgs,
            _ => continue,
        };
        for msg in messages {
            let msg_parts = match msg {
                redis::Value::Array(p) => p,
                _ => continue,
            };
            if msg_parts.len() < 2 {
                continue;
            }
            let id = match &msg_parts[0] {
                redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
                redis::Value::SimpleString(s) => s.clone(),
                _ => continue,
            };
            let fields_val = match &msg_parts[1] {
                redis::Value::Array(f) => f,
                _ => continue,
            };
            let mut fields = vec![];
            let mut it = fields_val.iter();
            while let (Some(k), Some(v)) = (it.next(), it.next()) {
                let key = match k {
                    redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
                    redis::Value::SimpleString(s) => s.clone(),
                    _ => continue,
                };
                let val = match v {
                    redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
                    redis::Value::SimpleString(s) => s.clone(),
                    _ => String::new(),
                };
                fields.push((key, val));
            }
            out.push((id, fields));
        }
    }
    out
}

impl Default for CacheClient {
    fn default() -> Self {
        Self::Noop
    }
}

/// Cache key helpers — centralize key construction to avoid collisions.
pub mod keys {
    pub fn execution(execution_id: &str) -> String {
        format!("af:execution:{execution_id}")
    }

    pub fn execution_list(tenant_id: &str) -> String {
        format!("af:executions:{tenant_id}")
    }

    pub fn workflow(tenant_id: &str, workflow_id: &str) -> String {
        format!("af:workflow:{tenant_id}:{workflow_id}")
    }

    pub fn workflow_list(tenant_id: &str) -> String {
        format!("af:workflows:{tenant_id}")
    }

    pub fn execution_events_channel(execution_id: &str) -> String {
        format!("af:events:{execution_id}")
    }

    pub fn exec_queue_stream() -> &'static str {
        "af:exec:queue"
    }

    pub fn exec_queue_group() -> &'static str {
        "af:exec:workers"
    }
}
