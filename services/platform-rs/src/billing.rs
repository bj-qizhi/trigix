// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn epoch_to_year_month(secs: u64) -> String {
    let mut days = secs / 86400;
    let mut y = 1970u32;
    loop {
        let dy = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            366u64
        } else {
            365u64
        };
        if days < dy {
            break;
        }
        days -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        m += 1;
    }
    format!("{:04}{:02}", y, m + 1)
}

fn current_year_month() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    epoch_to_year_month(secs)
}

/// Returns the list of N most-recent year_month strings (newest first), including the current month.
pub fn recent_year_months(n: usize) -> Vec<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut days = now / 86400;
    let mut y = 1970u32;
    loop {
        let dy: u64 = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            366
        } else {
            365
        };
        if days < dy {
            break;
        }
        days -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        m += 1;
    }
    // y, m now = current year/month (1-based)
    let mut result = Vec::with_capacity(n);
    let mut cy = y;
    let mut cm = m;
    for _ in 0..n {
        result.push(format!("{:04}{:02}", cy, cm));
        if cm == 1 {
            cm = 12;
            cy -= 1;
        } else {
            cm -= 1;
        }
    }
    result
}

/// Returns how many seconds until the start of next month (quota reset).
pub fn secs_until_quota_reset() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // compute next month start
    let mut days_remaining = now / 86400;
    let now_secs_in_day = now % 86400;
    let mut y = 1970u32;
    loop {
        let dy: u64 = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            366
        } else {
            365
        };
        if days_remaining < dy {
            break;
        }
        days_remaining -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0usize;
    let mut day_of_month = days_remaining;
    for (i, &md) in month_days.iter().enumerate() {
        if day_of_month < md {
            m = i;
            break;
        }
        day_of_month -= md;
    }
    // days_in_current_month - day_of_month = remaining days in month
    let days_left_in_month = month_days[m] - day_of_month;
    let secs_left_in_day = 86400 - now_secs_in_day;
    (days_left_in_month - 1) * 86400 + secs_left_in_day
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantQuota {
    pub tenant_id: String,
    pub tier: String,
    pub max_executions_per_month: i64,
    pub max_concurrent_executions: i64,
    pub max_workflows: i64,
}

impl TenantQuota {
    pub fn free(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            tier: "free".to_string(),
            max_executions_per_month: 1000,
            max_concurrent_executions: 10,
            max_workflows: 50,
        }
    }
    pub fn pro(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            tier: "pro".to_string(),
            max_executions_per_month: 50_000,
            max_concurrent_executions: 50,
            max_workflows: 500,
        }
    }
    pub fn business(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            tier: "business".to_string(),
            max_executions_per_month: 500_000,
            max_concurrent_executions: 200,
            max_workflows: 5000,
        }
    }
    pub fn unlimited(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            tier: "enterprise".to_string(),
            max_executions_per_month: i64::MAX,
            max_concurrent_executions: i64::MAX,
            max_workflows: i64::MAX,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageSummary {
    pub tenant_id: String,
    pub year_month: String,
    pub executions_used: i64,
    pub tokens_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingStatus {
    pub quota: TenantQuota,
    pub usage: UsageSummary,
    pub executions_remaining: i64,
    pub usage_pct: f64,
}

pub trait BillingStore: Send + Sync {
    fn get_quota(&self, tenant_id: &str) -> TenantQuota;
    fn set_quota(&self, quota: TenantQuota);
    fn get_usage(&self, tenant_id: &str, year_month: &str) -> UsageSummary;
    fn increment_execution(&self, tenant_id: &str);
    fn increment_tokens(&self, tenant_id: &str, tokens: i64);
    /// Returns `(stripe_customer_id, stripe_subscription_id)` for the tenant.
    fn get_stripe_ids(&self, tenant_id: &str) -> (Option<String>, Option<String>);
    /// Persists Stripe customer/subscription IDs.  `None` values leave existing data unchanged.
    fn set_stripe_ids(
        &self,
        tenant_id: &str,
        customer_id: Option<&str>,
        subscription_id: Option<&str>,
    );
    /// Looks up which tenant owns the given Stripe customer ID.
    fn get_tenant_by_stripe_customer(&self, customer_id: &str) -> Option<String>;
    /// Returns usage records for the last `months` months (newest first).
    fn get_usage_history(&self, tenant_id: &str, months: usize) -> Vec<UsageSummary>;
    fn billing_status(&self, tenant_id: &str) -> BillingStatus {
        let quota = self.get_quota(tenant_id);
        let ym = current_year_month();
        let usage = self.get_usage(tenant_id, &ym);
        let remaining = (quota.max_executions_per_month - usage.executions_used).max(0);
        let pct =
            if quota.max_executions_per_month == 0 || quota.max_executions_per_month == i64::MAX {
                0.0
            } else {
                (usage.executions_used as f64 / quota.max_executions_per_month as f64 * 100.0)
                    .min(100.0)
            };
        BillingStatus {
            quota,
            usage,
            executions_remaining: remaining,
            usage_pct: pct,
        }
    }
    fn check_execution_quota(&self, tenant_id: &str) -> Result<(), String> {
        let quota = self.get_quota(tenant_id);
        if quota.max_executions_per_month == i64::MAX {
            return Ok(());
        }
        let ym = current_year_month();
        let usage = self.get_usage(tenant_id, &ym);
        if usage.executions_used >= quota.max_executions_per_month {
            Err(format!(
                "Monthly execution quota exceeded ({}/{} used, tier: {}). Upgrade your plan at /billing.",
                usage.executions_used, quota.max_executions_per_month, quota.tier
            ))
        } else {
            Ok(())
        }
    }
}

// ── Memory implementation ──────────────────────────────────────────────────

#[derive(Default)]
pub struct MemoryBillingStore {
    quotas: RwLock<HashMap<String, TenantQuota>>,
    usage: RwLock<HashMap<(String, String), UsageSummary>>,
    stripe_ids: RwLock<HashMap<String, (Option<String>, Option<String>)>>,
}

impl BillingStore for MemoryBillingStore {
    fn get_quota(&self, tenant_id: &str) -> TenantQuota {
        self.quotas
            .read()
            .unwrap()
            .get(tenant_id)
            .cloned()
            .unwrap_or_else(|| TenantQuota::free(tenant_id))
    }
    fn set_quota(&self, quota: TenantQuota) {
        self.quotas
            .write()
            .unwrap()
            .insert(quota.tenant_id.clone(), quota);
    }
    fn get_usage(&self, tenant_id: &str, year_month: &str) -> UsageSummary {
        self.usage
            .read()
            .unwrap()
            .get(&(tenant_id.to_string(), year_month.to_string()))
            .cloned()
            .unwrap_or_else(|| UsageSummary {
                tenant_id: tenant_id.to_string(),
                year_month: year_month.to_string(),
                ..Default::default()
            })
    }
    fn increment_execution(&self, tenant_id: &str) {
        let ym = current_year_month();
        let key = (tenant_id.to_string(), ym.clone());
        let mut map = self.usage.write().unwrap();
        let entry = map.entry(key).or_insert_with(|| UsageSummary {
            tenant_id: tenant_id.to_string(),
            year_month: ym,
            ..Default::default()
        });
        entry.executions_used += 1;
    }
    fn increment_tokens(&self, tenant_id: &str, tokens: i64) {
        let ym = current_year_month();
        let key = (tenant_id.to_string(), ym.clone());
        let mut map = self.usage.write().unwrap();
        let entry = map.entry(key).or_insert_with(|| UsageSummary {
            tenant_id: tenant_id.to_string(),
            year_month: ym,
            ..Default::default()
        });
        entry.tokens_used += tokens;
    }
    fn get_stripe_ids(&self, tenant_id: &str) -> (Option<String>, Option<String>) {
        self.stripe_ids
            .read()
            .unwrap()
            .get(tenant_id)
            .cloned()
            .unwrap_or((None, None))
    }
    fn set_stripe_ids(
        &self,
        tenant_id: &str,
        customer_id: Option<&str>,
        subscription_id: Option<&str>,
    ) {
        let mut map = self.stripe_ids.write().unwrap();
        let entry = map.entry(tenant_id.to_string()).or_insert((None, None));
        if let Some(cid) = customer_id {
            entry.0 = Some(cid.to_string());
        }
        if let Some(sid) = subscription_id {
            entry.1 = Some(sid.to_string());
        }
    }
    fn get_tenant_by_stripe_customer(&self, customer_id: &str) -> Option<String> {
        self.stripe_ids
            .read()
            .unwrap()
            .iter()
            .find(|(_, (cid, _))| cid.as_deref() == Some(customer_id))
            .map(|(tid, _)| tid.clone())
    }
    fn get_usage_history(&self, tenant_id: &str, months: usize) -> Vec<UsageSummary> {
        let yms = recent_year_months(months);
        let map = self.usage.read().unwrap();
        yms.into_iter()
            .map(|ym| {
                map.get(&(tenant_id.to_string(), ym.clone()))
                    .cloned()
                    .unwrap_or_else(|| UsageSummary {
                        tenant_id: tenant_id.to_string(),
                        year_month: ym,
                        ..Default::default()
                    })
            })
            .collect()
    }
}

// ── Postgres implementation ────────────────────────────────────────────────

pub struct PostgresBillingStore {
    pool: sqlx::PgPool,
}

impl PostgresBillingStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

impl BillingStore for PostgresBillingStore {
    fn get_quota(&self, tenant_id: &str) -> TenantQuota {
        let pool = self.pool.clone();
        let tid = tenant_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, (String, i64, i64, i64)>(
                    "SELECT tier, max_executions_per_month, max_concurrent_executions, max_workflows \
                     FROM af_tenant_quotas WHERE tenant_id = $1"
                )
                .bind(&tid)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .map(|(tier, max_exec, max_conc, max_wf)| TenantQuota {
                    tenant_id: tid.clone(),
                    tier,
                    max_executions_per_month: max_exec,
                    max_concurrent_executions: max_conc,
                    max_workflows: max_wf,
                })
                .unwrap_or_else(|| TenantQuota::free(&tid))
            })
        })
    }
    fn set_quota(&self, quota: TenantQuota) {
        let pool = self.pool.clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO af_tenant_quotas (tenant_id, tier, max_executions_per_month, max_concurrent_executions, max_workflows, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (tenant_id) DO UPDATE SET \
                   tier = EXCLUDED.tier, \
                   max_executions_per_month = EXCLUDED.max_executions_per_month, \
                   max_concurrent_executions = EXCLUDED.max_concurrent_executions, \
                   max_workflows = EXCLUDED.max_workflows, \
                   updated_at = EXCLUDED.updated_at"
            )
            .bind(&quota.tenant_id)
            .bind(&quota.tier)
            .bind(quota.max_executions_per_month)
            .bind(quota.max_concurrent_executions)
            .bind(quota.max_workflows)
            .bind(now)
            .execute(&pool)
            .await;
        });
    }
    fn get_usage(&self, tenant_id: &str, year_month: &str) -> UsageSummary {
        let pool = self.pool.clone();
        let tid = tenant_id.to_string();
        let ym = year_month.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, (i64, i64)>(
                    "SELECT executions_used, tokens_used FROM af_usage_ledger \
                     WHERE tenant_id = $1 AND year_month = $2",
                )
                .bind(&tid)
                .bind(&ym)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .map(|(exec, tok)| UsageSummary {
                    tenant_id: tid.clone(),
                    year_month: ym.clone(),
                    executions_used: exec,
                    tokens_used: tok,
                })
                .unwrap_or_else(|| UsageSummary {
                    tenant_id: tid,
                    year_month: ym,
                    ..Default::default()
                })
            })
        })
    }
    fn increment_execution(&self, tenant_id: &str) {
        let pool = self.pool.clone();
        let tid = tenant_id.to_string();
        let ym = current_year_month();
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO af_usage_ledger (tenant_id, year_month, executions_used, tokens_used) \
                 VALUES ($1, $2, 1, 0) \
                 ON CONFLICT (tenant_id, year_month) DO UPDATE SET \
                   executions_used = af_usage_ledger.executions_used + 1"
            )
            .bind(&tid)
            .bind(&ym)
            .execute(&pool)
            .await;
        });
    }
    fn increment_tokens(&self, tenant_id: &str, tokens: i64) {
        let pool = self.pool.clone();
        let tid = tenant_id.to_string();
        let ym = current_year_month();
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO af_usage_ledger (tenant_id, year_month, executions_used, tokens_used) \
                 VALUES ($1, $2, 0, $3) \
                 ON CONFLICT (tenant_id, year_month) DO UPDATE SET \
                   tokens_used = af_usage_ledger.tokens_used + EXCLUDED.tokens_used"
            )
            .bind(&tid)
            .bind(&ym)
            .bind(tokens)
            .execute(&pool)
            .await;
        });
    }
    fn get_stripe_ids(&self, tenant_id: &str) -> (Option<String>, Option<String>) {
        let pool = self.pool.clone();
        let tid = tenant_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, (Option<String>, Option<String>)>(
                    "SELECT stripe_customer_id, stripe_subscription_id \
                     FROM af_tenant_quotas WHERE tenant_id = $1",
                )
                .bind(&tid)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .unwrap_or((None, None))
            })
        })
    }
    fn set_stripe_ids(
        &self,
        tenant_id: &str,
        customer_id: Option<&str>,
        subscription_id: Option<&str>,
    ) {
        let pool = self.pool.clone();
        let tid = tenant_id.to_string();
        let cid = customer_id.map(|s| s.to_string());
        let sid = subscription_id.map(|s| s.to_string());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO af_tenant_quotas \
                   (tenant_id, tier, max_executions_per_month, max_concurrent_executions, \
                    max_workflows, stripe_customer_id, stripe_subscription_id, updated_at) \
                 VALUES ($1, 'free', 1000, 10, 50, $2, $3, $4) \
                 ON CONFLICT (tenant_id) DO UPDATE SET \
                   stripe_customer_id    = COALESCE(EXCLUDED.stripe_customer_id,    af_tenant_quotas.stripe_customer_id), \
                   stripe_subscription_id = COALESCE(EXCLUDED.stripe_subscription_id, af_tenant_quotas.stripe_subscription_id), \
                   updated_at            = EXCLUDED.updated_at"
            )
            .bind(&tid)
            .bind(&cid)
            .bind(&sid)
            .bind(now)
            .execute(&pool)
            .await;
        });
    }
    fn get_tenant_by_stripe_customer(&self, customer_id: &str) -> Option<String> {
        let pool = self.pool.clone();
        let cid = customer_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query_as::<_, (String,)>(
                    "SELECT tenant_id FROM af_tenant_quotas WHERE stripe_customer_id = $1 LIMIT 1",
                )
                .bind(&cid)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .map(|(tid,)| tid)
            })
        })
    }
    fn get_usage_history(&self, tenant_id: &str, months: usize) -> Vec<UsageSummary> {
        let pool = self.pool.clone();
        let tid = tenant_id.to_string();
        let yms = recent_year_months(months);
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let rows: Vec<(String, i64, i64)> = sqlx::query_as(
                    "SELECT year_month, executions_used, tokens_used \
                     FROM af_usage_ledger WHERE tenant_id = $1 \
                     ORDER BY year_month DESC LIMIT 24",
                )
                .bind(&tid)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
                let map: std::collections::HashMap<String, (i64, i64)> =
                    rows.into_iter().map(|(ym, e, t)| (ym, (e, t))).collect();
                yms.into_iter()
                    .map(|ym| {
                        let (exec, tok) = map.get(&ym).copied().unwrap_or((0, 0));
                        UsageSummary {
                            tenant_id: tid.clone(),
                            year_month: ym,
                            executions_used: exec,
                            tokens_used: tok,
                        }
                    })
                    .collect()
            })
        })
    }
}

// ── Platform enum ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformBillingStore {
    Memory(Arc<MemoryBillingStore>),
    Postgres(Arc<PostgresBillingStore>),
}

impl PlatformBillingStore {
    pub fn memory() -> Self {
        Self::Memory(Arc::new(MemoryBillingStore::default()))
    }
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self::Postgres(Arc::new(PostgresBillingStore::new(pool)))
    }
}

impl BillingStore for PlatformBillingStore {
    fn get_quota(&self, t: &str) -> TenantQuota {
        match self {
            Self::Memory(s) => s.get_quota(t),
            Self::Postgres(s) => s.get_quota(t),
        }
    }
    fn set_quota(&self, q: TenantQuota) {
        match self {
            Self::Memory(s) => s.set_quota(q),
            Self::Postgres(s) => s.set_quota(q),
        }
    }
    fn get_usage(&self, t: &str, ym: &str) -> UsageSummary {
        match self {
            Self::Memory(s) => s.get_usage(t, ym),
            Self::Postgres(s) => s.get_usage(t, ym),
        }
    }
    fn increment_execution(&self, t: &str) {
        match self {
            Self::Memory(s) => s.increment_execution(t),
            Self::Postgres(s) => s.increment_execution(t),
        }
    }
    fn increment_tokens(&self, t: &str, tokens: i64) {
        match self {
            Self::Memory(s) => s.increment_tokens(t, tokens),
            Self::Postgres(s) => s.increment_tokens(t, tokens),
        }
    }
    fn get_stripe_ids(&self, t: &str) -> (Option<String>, Option<String>) {
        match self {
            Self::Memory(s) => s.get_stripe_ids(t),
            Self::Postgres(s) => s.get_stripe_ids(t),
        }
    }
    fn set_stripe_ids(&self, t: &str, cid: Option<&str>, sid: Option<&str>) {
        match self {
            Self::Memory(s) => s.set_stripe_ids(t, cid, sid),
            Self::Postgres(s) => s.set_stripe_ids(t, cid, sid),
        }
    }
    fn get_tenant_by_stripe_customer(&self, cid: &str) -> Option<String> {
        match self {
            Self::Memory(s) => s.get_tenant_by_stripe_customer(cid),
            Self::Postgres(s) => s.get_tenant_by_stripe_customer(cid),
        }
    }
    fn get_usage_history(&self, t: &str, months: usize) -> Vec<UsageSummary> {
        match self {
            Self::Memory(s) => s.get_usage_history(t, months),
            Self::Postgres(s) => s.get_usage_history(t, months),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_to_year_month_unix_epoch() {
        assert_eq!(epoch_to_year_month(0), "197001");
    }

    #[test]
    fn epoch_to_year_month_feb_1970() {
        assert_eq!(epoch_to_year_month(86400 * 31), "197002"); // 1970-02-01
    }

    #[test]
    fn epoch_to_year_month_jan_2000() {
        assert_eq!(epoch_to_year_month(946684800), "200001"); // 2000-01-01 (well-known timestamp)
    }

    #[test]
    fn epoch_to_year_month_may_2026() {
        assert_eq!(epoch_to_year_month(1777593600), "202605"); // 2026-05-01
        assert_eq!(epoch_to_year_month(1780185600), "202605"); // 2026-05-31
    }

    #[test]
    fn epoch_to_year_month_jun_2026() {
        assert_eq!(epoch_to_year_month(1780272000), "202606"); // 2026-06-01
    }

    #[test]
    fn current_year_month_format() {
        let ym = current_year_month();
        assert_eq!(ym.len(), 6, "expected YYYYMM, got {ym}");
        let year: u32 = ym[..4].parse().expect("year not numeric");
        let month: u32 = ym[4..].parse().expect("month not numeric");
        assert!((2024..=2050).contains(&year), "year out of range: {year}");
        assert!((1..=12).contains(&month), "month out of range: {month}");
    }

    #[test]
    fn memory_billing_store_increments_and_checks_quota() {
        let store = MemoryBillingStore::default();
        store.set_quota(TenantQuota {
            tenant_id: "t1".into(),
            tier: "free".into(),
            max_executions_per_month: 2,
            max_concurrent_executions: 10,
            max_workflows: 50,
        });
        assert!(store.check_execution_quota("t1").is_ok());
        store.increment_execution("t1");
        store.increment_execution("t1");
        assert!(
            store.check_execution_quota("t1").is_err(),
            "quota should be exhausted after 2 increments"
        );
    }
}
