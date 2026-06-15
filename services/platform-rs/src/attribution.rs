// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! First-touch acquisition attribution: records which channel/campaign brought a
//! signup (captured at registration), so a later paid conversion can be credited
//! to its acquisition source and forwarded to PostHog server-side.
//!
//! First touch wins — once a tenant has an attribution row it is never
//! overwritten, mirroring how marketing attribution should treat the first
//! recorded acquisition channel.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;

/// A captured first-touch attribution for one tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttributionRecord {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
    pub referrer: Option<String>,
    pub landing_page: Option<String>,
    /// PostHog distinct id, so server-side conversion events stitch to the
    /// browser session that was first seen on the landing page.
    pub distinct_id: Option<String>,
    pub created_at: u64,
}

impl AttributionRecord {
    /// True when at least one acquisition signal is present — empty payloads
    /// (e.g. direct/no-UTM signups) are not worth persisting.
    pub fn has_signal(&self) -> bool {
        self.utm_source.is_some()
            || self.utm_medium.is_some()
            || self.utm_campaign.is_some()
            || self.utm_term.is_some()
            || self.utm_content.is_some()
            || self.referrer.is_some()
            || self.landing_page.is_some()
            || self.distinct_id.is_some()
    }
}

#[allow(async_fn_in_trait)]
pub trait AttributionStore: Clone + Send + Sync + 'static {
    /// Records first-touch attribution. No-op if the tenant already has a row.
    async fn record_first_touch(&self, rec: AttributionRecord);
    async fn get(&self, tenant_id: &str) -> Option<AttributionRecord>;
}

#[derive(Clone, Default)]
pub struct MemoryAttributionStore {
    rows: Arc<RwLock<HashMap<String, AttributionRecord>>>,
}

impl AttributionStore for MemoryAttributionStore {
    async fn record_first_touch(&self, rec: AttributionRecord) {
        if let Ok(mut rows) = self.rows.write() {
            rows.entry(rec.tenant_id.clone()).or_insert(rec);
        }
    }
    async fn get(&self, tenant_id: &str) -> Option<AttributionRecord> {
        self.rows
            .read()
            .ok()
            .and_then(|r| r.get(tenant_id).cloned())
    }
}

#[derive(Clone)]
pub struct PostgresAttributionStore {
    pool: PgPool,
}

impl PostgresAttributionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl AttributionStore for PostgresAttributionStore {
    async fn record_first_touch(&self, rec: AttributionRecord) {
        let _ = sqlx::query(
            r#"
            INSERT INTO af_attribution
              (tenant_id, user_id, utm_source, utm_medium, utm_campaign, utm_term,
               utm_content, referrer, landing_page, distinct_id, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (tenant_id) DO NOTHING
            "#,
        )
        .bind(&rec.tenant_id)
        .bind(&rec.user_id)
        .bind(&rec.utm_source)
        .bind(&rec.utm_medium)
        .bind(&rec.utm_campaign)
        .bind(&rec.utm_term)
        .bind(&rec.utm_content)
        .bind(&rec.referrer)
        .bind(&rec.landing_page)
        .bind(&rec.distinct_id)
        .bind(rec.created_at as i64)
        .execute(&self.pool)
        .await;
    }

    async fn get(&self, tenant_id: &str) -> Option<AttributionRecord> {
        #[derive(sqlx::FromRow)]
        struct Row {
            tenant_id: String,
            user_id: Option<String>,
            utm_source: Option<String>,
            utm_medium: Option<String>,
            utm_campaign: Option<String>,
            utm_term: Option<String>,
            utm_content: Option<String>,
            referrer: Option<String>,
            landing_page: Option<String>,
            distinct_id: Option<String>,
            created_at: i64,
        }
        let row = sqlx::query_as::<_, Row>(
            r#"
            SELECT tenant_id, user_id, utm_source, utm_medium, utm_campaign, utm_term,
                   utm_content, referrer, landing_page, distinct_id, created_at
            FROM af_attribution WHERE tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()?;
        Some(AttributionRecord {
            tenant_id: row.tenant_id,
            user_id: row.user_id,
            utm_source: row.utm_source,
            utm_medium: row.utm_medium,
            utm_campaign: row.utm_campaign,
            utm_term: row.utm_term,
            utm_content: row.utm_content,
            referrer: row.referrer,
            landing_page: row.landing_page,
            distinct_id: row.distinct_id,
            created_at: row.created_at as u64,
        })
    }
}

#[derive(Clone)]
pub enum PlatformAttributionStore {
    Memory(MemoryAttributionStore),
    Postgres(PostgresAttributionStore),
}

impl Default for PlatformAttributionStore {
    fn default() -> Self {
        Self::Memory(MemoryAttributionStore::default())
    }
}

impl PlatformAttributionStore {
    pub fn postgres(store: PostgresAttributionStore) -> Self {
        Self::Postgres(store)
    }
}

impl AttributionStore for PlatformAttributionStore {
    async fn record_first_touch(&self, rec: AttributionRecord) {
        match self {
            Self::Memory(s) => s.record_first_touch(rec).await,
            Self::Postgres(s) => s.record_first_touch(rec).await,
        }
    }
    async fn get(&self, tenant_id: &str) -> Option<AttributionRecord> {
        match self {
            Self::Memory(s) => s.get(tenant_id).await,
            Self::Postgres(s) => s.get(tenant_id).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(tenant: &str, source: &str) -> AttributionRecord {
        AttributionRecord {
            tenant_id: tenant.to_string(),
            utm_source: Some(source.to_string()),
            created_at: 1,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn first_touch_wins_and_is_not_overwritten() {
        let store = MemoryAttributionStore::default();
        store.record_first_touch(rec("t1", "google")).await;
        store.record_first_touch(rec("t1", "twitter")).await;
        let got = store.get("t1").await.unwrap();
        assert_eq!(got.utm_source.as_deref(), Some("google"));
    }

    #[test]
    fn has_signal_detects_empty_vs_attributed() {
        assert!(!AttributionRecord::default().has_signal());
        assert!(rec("t1", "google").has_signal());
    }
}
