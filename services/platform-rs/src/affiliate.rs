// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Affiliate / referral program.
//!
//! Each tenant has a shareable referral [`code`](AffiliateStore::get_or_create_code).
//! A new signup can supply a referrer's code, creating a first-touch referral
//! link. When a referred tenant pays an invoice, the referrer accrues a
//! commission in a signed ledger; refunds claw it back, and operator payouts
//! debit it. A referrer's balance is the sum of their ledger entries.
//!
//! The ledger is single-entry with signed amounts (the practical model most
//! affiliate programs use), not strict double-entry accounting.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPool;

/// Ledger entry kinds.
pub mod kind {
    /// Commission accrued from a referred tenant's paid invoice (positive).
    pub const COMMISSION: &str = "commission";
    /// Commission reversed when the referred tenant's payment is refunded (negative).
    pub const CLAWBACK: &str = "clawback";
    /// Operator payout of accrued balance to the affiliate (negative).
    pub const PAYOUT: &str = "payout";
}

#[derive(Debug, Clone, Serialize)]
pub struct LedgerEntry {
    pub id: String,
    pub referrer_tenant: String,
    pub referee_tenant: Option<String>,
    /// Signed minor-currency amount: commissions are positive, clawbacks and
    /// payouts are negative.
    pub amount_cents: i64,
    pub kind: String,
    pub source_ref: Option<String>,
    pub created_at: u64,
}

/// Deterministic 8-char uppercase referral code derived from the tenant id, so a
/// tenant always maps to the same code without a generation round-trip.
pub fn code_for(tenant_id: &str) -> String {
    let digest = Sha256::digest(tenant_id.as_bytes());
    hex::encode(digest)[..8].to_uppercase()
}

#[allow(async_fn_in_trait)]
pub trait AffiliateStore: Clone + Send + Sync + 'static {
    /// Returns the tenant's referral code, creating (persisting) it on first use.
    async fn get_or_create_code(&self, tenant_id: &str) -> String;
    /// Resolves a referral code to its owning (referrer) tenant.
    async fn resolve_code(&self, code: &str) -> Option<String>;
    /// Records a first-touch referral. No-op if the referee already has one or
    /// the referrer is the referee.
    async fn record_referral(&self, referee_tenant: &str, referrer_tenant: &str, code: &str);
    /// The referrer that brought in `referee_tenant`, if any.
    async fn get_referrer(&self, referee_tenant: &str) -> Option<String>;
    async fn add_entry(&self, entry: LedgerEntry);
    /// Sum of the referrer's ledger entries (their payable balance).
    async fn balance_cents(&self, referrer_tenant: &str) -> i64;
    async fn referral_count(&self, referrer_tenant: &str) -> i64;
    async fn list_entries(&self, referrer_tenant: &str, limit: i64) -> Vec<LedgerEntry>;
}

// ── Memory implementation ──────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct MemoryAffiliateStore {
    /// tenant -> code
    codes: Arc<RwLock<HashMap<String, String>>>,
    /// referee -> (referrer, code)
    referrals: Arc<RwLock<HashMap<String, (String, String)>>>,
    ledger: Arc<RwLock<Vec<LedgerEntry>>>,
}

impl AffiliateStore for MemoryAffiliateStore {
    async fn get_or_create_code(&self, tenant_id: &str) -> String {
        let code = code_for(tenant_id);
        if let Ok(mut codes) = self.codes.write() {
            codes.entry(tenant_id.to_string()).or_insert(code.clone());
        }
        code
    }
    async fn resolve_code(&self, code: &str) -> Option<String> {
        let codes = self.codes.read().ok()?;
        codes
            .iter()
            .find(|(_, c)| c.as_str() == code)
            .map(|(t, _)| t.clone())
    }
    async fn record_referral(&self, referee_tenant: &str, referrer_tenant: &str, code: &str) {
        if referee_tenant == referrer_tenant {
            return;
        }
        if let Ok(mut refs) = self.referrals.write() {
            refs.entry(referee_tenant.to_string())
                .or_insert((referrer_tenant.to_string(), code.to_string()));
        }
    }
    async fn get_referrer(&self, referee_tenant: &str) -> Option<String> {
        self.referrals
            .read()
            .ok()?
            .get(referee_tenant)
            .map(|(r, _)| r.clone())
    }
    async fn add_entry(&self, entry: LedgerEntry) {
        if let Ok(mut l) = self.ledger.write() {
            l.push(entry);
        }
    }
    async fn balance_cents(&self, referrer_tenant: &str) -> i64 {
        self.ledger
            .read()
            .map(|l| {
                l.iter()
                    .filter(|e| e.referrer_tenant == referrer_tenant)
                    .map(|e| e.amount_cents)
                    .sum()
            })
            .unwrap_or(0)
    }
    async fn referral_count(&self, referrer_tenant: &str) -> i64 {
        self.referrals
            .read()
            .map(|r| r.values().filter(|(t, _)| t == referrer_tenant).count() as i64)
            .unwrap_or(0)
    }
    async fn list_entries(&self, referrer_tenant: &str, limit: i64) -> Vec<LedgerEntry> {
        let Ok(l) = self.ledger.read() else {
            return Vec::new();
        };
        let mut entries: Vec<LedgerEntry> = l
            .iter()
            .filter(|e| e.referrer_tenant == referrer_tenant)
            .cloned()
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.created_at));
        entries.truncate(limit.max(0) as usize);
        entries
    }
}

// ── Postgres implementation ────────────────────────────────────────────────

#[derive(Clone)]
pub struct PostgresAffiliateStore {
    pool: PgPool,
}

impl PostgresAffiliateStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl AffiliateStore for PostgresAffiliateStore {
    async fn get_or_create_code(&self, tenant_id: &str) -> String {
        let code = code_for(tenant_id);
        let _ = sqlx::query(
            "INSERT INTO af_affiliate_codes (tenant_id, code, created_at) VALUES ($1, $2, $3) \
             ON CONFLICT (tenant_id) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(&code)
        .bind(now_secs())
        .execute(&self.pool)
        .await;
        // Return the stored code (in case one already existed).
        sqlx::query_scalar::<_, String>("SELECT code FROM af_affiliate_codes WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .unwrap_or(code)
    }
    async fn resolve_code(&self, code: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>("SELECT tenant_id FROM af_affiliate_codes WHERE code = $1")
            .bind(code)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
    }
    async fn record_referral(&self, referee_tenant: &str, referrer_tenant: &str, code: &str) {
        if referee_tenant == referrer_tenant {
            return;
        }
        let _ = sqlx::query(
            "INSERT INTO af_referrals (referee_tenant, referrer_tenant, code, created_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (referee_tenant) DO NOTHING",
        )
        .bind(referee_tenant)
        .bind(referrer_tenant)
        .bind(code)
        .bind(now_secs())
        .execute(&self.pool)
        .await;
    }
    async fn get_referrer(&self, referee_tenant: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT referrer_tenant FROM af_referrals WHERE referee_tenant = $1",
        )
        .bind(referee_tenant)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
    }
    async fn add_entry(&self, entry: LedgerEntry) {
        let _ = sqlx::query(
            "INSERT INTO af_affiliate_ledger \
               (id, referrer_tenant, referee_tenant, amount_cents, kind, source_ref, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&entry.id)
        .bind(&entry.referrer_tenant)
        .bind(&entry.referee_tenant)
        .bind(entry.amount_cents)
        .bind(&entry.kind)
        .bind(&entry.source_ref)
        .bind(entry.created_at as i64)
        .execute(&self.pool)
        .await;
    }
    async fn balance_cents(&self, referrer_tenant: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(amount_cents), 0)::bigint FROM af_affiliate_ledger \
             WHERE referrer_tenant = $1",
        )
        .bind(referrer_tenant)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0)
    }
    async fn referral_count(&self, referrer_tenant: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::bigint FROM af_referrals WHERE referrer_tenant = $1",
        )
        .bind(referrer_tenant)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0)
    }
    async fn list_entries(&self, referrer_tenant: &str, limit: i64) -> Vec<LedgerEntry> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            referrer_tenant: String,
            referee_tenant: Option<String>,
            amount_cents: i64,
            kind: String,
            source_ref: Option<String>,
            created_at: i64,
        }
        sqlx::query_as::<_, Row>(
            "SELECT id, referrer_tenant, referee_tenant, amount_cents, kind, source_ref, created_at \
             FROM af_affiliate_ledger WHERE referrer_tenant = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(referrer_tenant)
        .bind(limit.max(0))
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| LedgerEntry {
            id: r.id,
            referrer_tenant: r.referrer_tenant,
            referee_tenant: r.referee_tenant,
            amount_cents: r.amount_cents,
            kind: r.kind,
            source_ref: r.source_ref,
            created_at: r.created_at as u64,
        })
        .collect()
    }
}

// ── Platform enum ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformAffiliateStore {
    Memory(MemoryAffiliateStore),
    Postgres(PostgresAffiliateStore),
}

impl Default for PlatformAffiliateStore {
    fn default() -> Self {
        Self::Memory(MemoryAffiliateStore::default())
    }
}

impl PlatformAffiliateStore {
    pub fn postgres(store: PostgresAffiliateStore) -> Self {
        Self::Postgres(store)
    }
}

impl AffiliateStore for PlatformAffiliateStore {
    async fn get_or_create_code(&self, tenant_id: &str) -> String {
        match self {
            Self::Memory(s) => s.get_or_create_code(tenant_id).await,
            Self::Postgres(s) => s.get_or_create_code(tenant_id).await,
        }
    }
    async fn resolve_code(&self, code: &str) -> Option<String> {
        match self {
            Self::Memory(s) => s.resolve_code(code).await,
            Self::Postgres(s) => s.resolve_code(code).await,
        }
    }
    async fn record_referral(&self, referee_tenant: &str, referrer_tenant: &str, code: &str) {
        match self {
            Self::Memory(s) => {
                s.record_referral(referee_tenant, referrer_tenant, code)
                    .await
            }
            Self::Postgres(s) => {
                s.record_referral(referee_tenant, referrer_tenant, code)
                    .await
            }
        }
    }
    async fn get_referrer(&self, referee_tenant: &str) -> Option<String> {
        match self {
            Self::Memory(s) => s.get_referrer(referee_tenant).await,
            Self::Postgres(s) => s.get_referrer(referee_tenant).await,
        }
    }
    async fn add_entry(&self, entry: LedgerEntry) {
        match self {
            Self::Memory(s) => s.add_entry(entry).await,
            Self::Postgres(s) => s.add_entry(entry).await,
        }
    }
    async fn balance_cents(&self, referrer_tenant: &str) -> i64 {
        match self {
            Self::Memory(s) => s.balance_cents(referrer_tenant).await,
            Self::Postgres(s) => s.balance_cents(referrer_tenant).await,
        }
    }
    async fn referral_count(&self, referrer_tenant: &str) -> i64 {
        match self {
            Self::Memory(s) => s.referral_count(referrer_tenant).await,
            Self::Postgres(s) => s.referral_count(referrer_tenant).await,
        }
    }
    async fn list_entries(&self, referrer_tenant: &str, limit: i64) -> Vec<LedgerEntry> {
        match self {
            Self::Memory(s) => s.list_entries(referrer_tenant, limit).await,
            Self::Postgres(s) => s.list_entries(referrer_tenant, limit).await,
        }
    }
}

/// Commission rate (percent of a referred tenant's paid invoice) from
/// `AFFILIATE_COMMISSION_PCT`. Defaults to 0 — affiliates are tracked but no
/// commission accrues until the operator opts in by setting a rate.
pub fn commission_pct() -> f64 {
    std::env::var("AFFILIATE_COMMISSION_PCT")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|p| *p >= 0.0)
        .unwrap_or(0.0)
}

/// Commission for a payment of `amount_cents` at the configured rate.
pub fn commission_for(amount_cents: i64) -> i64 {
    ((amount_cents as f64) * commission_pct() / 100.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn code_roundtrip_and_referral_first_touch() {
        let store = MemoryAffiliateStore::default();
        let code = store.get_or_create_code("referrer").await;
        assert_eq!(code, code_for("referrer"));
        assert_eq!(store.resolve_code(&code).await.as_deref(), Some("referrer"));

        // First-touch referral; a later attempt does not overwrite.
        store.record_referral("referee", "referrer", &code).await;
        store
            .record_referral("referee", "someone-else", &code)
            .await;
        assert_eq!(
            store.get_referrer("referee").await.as_deref(),
            Some("referrer")
        );
        // Self-referral is ignored.
        store.record_referral("solo", "solo", "X").await;
        assert_eq!(store.get_referrer("solo").await, None);
        assert_eq!(store.referral_count("referrer").await, 1);
    }

    #[tokio::test]
    async fn ledger_balance_sums_signed_entries() {
        let store = MemoryAffiliateStore::default();
        let entry = |amount, k: &str| LedgerEntry {
            id: uuid::Uuid::new_v4().to_string(),
            referrer_tenant: "r".into(),
            referee_tenant: Some("e".into()),
            amount_cents: amount,
            kind: k.into(),
            source_ref: None,
            created_at: 1,
        };
        store.add_entry(entry(1000, kind::COMMISSION)).await;
        store.add_entry(entry(-300, kind::CLAWBACK)).await;
        store.add_entry(entry(-200, kind::PAYOUT)).await;
        assert_eq!(store.balance_cents("r").await, 500);
        assert_eq!(store.list_entries("r", 10).await.len(), 3);
        assert_eq!(store.balance_cents("other").await, 0);
    }

    #[test]
    fn commission_for_uses_configured_rate() {
        // Default (unset) → no commission.
        std::env::remove_var("AFFILIATE_COMMISSION_PCT");
        assert_eq!(commission_for(10_000), 0);
    }
}
