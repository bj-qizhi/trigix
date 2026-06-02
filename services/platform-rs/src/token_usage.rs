// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::Serialize;
use sqlx::postgres::PgPool;

#[derive(Debug, Clone, Serialize)]
pub struct TokenUsageRecord {
    pub id: String,
    pub tenant_id: String,
    pub execution_id: String,
    pub node_id: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenUsageSummary {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub by_model: HashMap<String, ModelUsage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[allow(async_fn_in_trait)]
pub trait TokenUsageStore: Clone + Send + Sync + 'static {
    async fn record(&self, rec: TokenUsageRecord);
    async fn summarize(&self, tenant_id: &str, since_unix: u64) -> TokenUsageSummary;
}

#[derive(Clone, Default)]
pub struct MemoryTokenUsageStore {
    records: Arc<RwLock<Vec<TokenUsageRecord>>>,
}

impl TokenUsageStore for MemoryTokenUsageStore {
    async fn record(&self, rec: TokenUsageRecord) {
        if let Ok(mut recs) = self.records.write() {
            recs.push(rec);
        }
    }

    async fn summarize(&self, tenant_id: &str, since_unix: u64) -> TokenUsageSummary {
        let recs = self.records.read().unwrap_or_else(|e| e.into_inner());
        let relevant: Vec<_> = recs
            .iter()
            .filter(|r| r.tenant_id == tenant_id && r.created_at >= since_unix)
            .collect();
        build_summary(relevant.into_iter().map(|r| r as &TokenUsageRecord))
    }
}

#[derive(Clone)]
pub struct PostgresTokenUsageStore {
    pool: PgPool,
}

impl PostgresTokenUsageStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl TokenUsageStore for PostgresTokenUsageStore {
    async fn record(&self, rec: TokenUsageRecord) {
        let _ = sqlx::query(
            r#"
            INSERT INTO af_token_usage (id, tenant_id, execution_id, node_id, model, prompt_tokens, completion_tokens, total_tokens, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&rec.id)
        .bind(&rec.tenant_id)
        .bind(&rec.execution_id)
        .bind(&rec.node_id)
        .bind(&rec.model)
        .bind(rec.prompt_tokens)
        .bind(rec.completion_tokens)
        .bind(rec.total_tokens)
        .bind(rec.created_at as i64)
        .execute(&self.pool)
        .await;
    }

    async fn summarize(&self, tenant_id: &str, since_unix: u64) -> TokenUsageSummary {
        #[derive(sqlx::FromRow)]
        struct Row {
            model: String,
            prompt_tokens: i64,
            completion_tokens: i64,
            total_tokens: i64,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT model,
                   SUM(prompt_tokens)::bigint AS prompt_tokens,
                   SUM(completion_tokens)::bigint AS completion_tokens,
                   SUM(total_tokens)::bigint AS total_tokens
            FROM af_token_usage
            WHERE tenant_id = $1 AND created_at >= $2
            GROUP BY model
            "#,
        )
        .bind(tenant_id)
        .bind(since_unix as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let mut by_model: HashMap<String, ModelUsage> = HashMap::new();
        let mut total_prompt = 0i64;
        let mut total_completion = 0i64;
        let mut total = 0i64;

        for row in rows {
            total_prompt += row.prompt_tokens;
            total_completion += row.completion_tokens;
            total += row.total_tokens;
            by_model.insert(row.model, ModelUsage {
                prompt_tokens: row.prompt_tokens,
                completion_tokens: row.completion_tokens,
                total_tokens: row.total_tokens,
            });
        }

        TokenUsageSummary {
            prompt_tokens: total_prompt,
            completion_tokens: total_completion,
            total_tokens: total,
            by_model,
        }
    }
}

#[derive(Clone)]
pub enum PlatformTokenUsageStore {
    Memory(MemoryTokenUsageStore),
    Postgres(PostgresTokenUsageStore),
}

impl Default for PlatformTokenUsageStore {
    fn default() -> Self {
        Self::Memory(MemoryTokenUsageStore::default())
    }
}

impl PlatformTokenUsageStore {
    pub fn postgres(store: PostgresTokenUsageStore) -> Self {
        Self::Postgres(store)
    }
}

impl TokenUsageStore for PlatformTokenUsageStore {
    async fn record(&self, rec: TokenUsageRecord) {
        match self {
            Self::Memory(s) => s.record(rec).await,
            Self::Postgres(s) => s.record(rec).await,
        }
    }

    async fn summarize(&self, tenant_id: &str, since_unix: u64) -> TokenUsageSummary {
        match self {
            Self::Memory(s) => s.summarize(tenant_id, since_unix).await,
            Self::Postgres(s) => s.summarize(tenant_id, since_unix).await,
        }
    }
}

fn build_summary<'a>(records: impl Iterator<Item = &'a TokenUsageRecord>) -> TokenUsageSummary {
    let mut by_model: HashMap<String, ModelUsage> = HashMap::new();
    let mut total_prompt = 0i64;
    let mut total_completion = 0i64;
    let mut total = 0i64;

    for r in records {
        total_prompt += r.prompt_tokens;
        total_completion += r.completion_tokens;
        total += r.total_tokens;
        let entry = by_model.entry(r.model.clone()).or_insert(ModelUsage {
            prompt_tokens: 0, completion_tokens: 0, total_tokens: 0,
        });
        entry.prompt_tokens += r.prompt_tokens;
        entry.completion_tokens += r.completion_tokens;
        entry.total_tokens += r.total_tokens;
    }

    TokenUsageSummary {
        prompt_tokens: total_prompt,
        completion_tokens: total_completion,
        total_tokens: total,
        by_model,
    }
}

/// Extract token usage records from a completed execution's node results.
/// Looks for nodes whose output_json contains `usage.prompt_tokens` and `usage.completion_tokens`
/// (OpenAI and Gemini both return this structure).
pub fn extract_token_usage(
    tenant_id: &str,
    execution_id: &str,
    node_results: &[crate::execution::NodeExecutionRecord],
    now_unix: u64,
) -> Vec<TokenUsageRecord> {
    let mut out = Vec::new();
    for nr in node_results {
        if nr.node_type != "openai" && nr.node_type != "gemini" && nr.node_type != "claude" {
            continue;
        }
        let Some(output) = &nr.output_json else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(output) else { continue };
        let usage = v.get("usage").or_else(|| v.get("usageMetadata"));
        let Some(usage) = usage else { continue };
        let prompt = usage.get("prompt_tokens")
            .or_else(|| usage.get("promptTokenCount"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let completion = usage.get("completion_tokens")
            .or_else(|| usage.get("candidatesTokenCount"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let total = usage.get("total_tokens")
            .or_else(|| usage.get("totalTokenCount"))
            .and_then(|v| v.as_i64())
            .unwrap_or(prompt + completion);
        let model = v.get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(&nr.node_type)
            .to_string();
        out.push(TokenUsageRecord {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.to_string(),
            execution_id: execution_id.to_string(),
            node_id: nr.node_id.clone(),
            model,
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            created_at: now_unix,
        });
    }
    out
}
