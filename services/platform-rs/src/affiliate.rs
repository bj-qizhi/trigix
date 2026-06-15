// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Affiliate / referral program with a double-entry general ledger.
//!
//! Each tenant has a shareable referral [`code`](AffiliateStore::get_or_create_code).
//! A new signup can supply a referrer's code, creating a first-touch referral
//! link. Money events post **balanced** GL transactions (debit-positive; every
//! transaction's postings sum to zero):
//!
//! - commission: Dr `commission_expense`, Cr `affiliate_payable[affiliate]`
//! - clawback:   the reverse
//! - payout:     Dr `affiliate_payable[affiliate]`, Cr `cash`
//!
//! An affiliate's payable account carries a credit (negative) balance; the amount
//! owed to them is its negation, exposed by [`AffiliateStore::balance_cents`].

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPool;

/// General-ledger account names.
pub mod account {
    pub const AFFILIATE_PAYABLE: &str = "affiliate_payable";
    pub const COMMISSION_EXPENSE: &str = "commission_expense";
    pub const CASH: &str = "cash";
}

/// Business-event kind recorded on a posting (for display/filtering).
pub mod kind {
    pub const COMMISSION: &str = "commission";
    pub const CLAWBACK: &str = "clawback";
    pub const PAYOUT: &str = "payout";
}

/// A single general-ledger posting (one leg of a balanced transaction).
#[derive(Debug, Clone)]
pub struct Posting {
    pub id: String,
    pub txn_id: String,
    pub account: String,
    /// Owner of the account (the affiliate, for `affiliate_payable`); `None` for
    /// platform accounts.
    pub tenant_id: Option<String>,
    /// The referred tenant this posting relates to, for display context.
    pub referee_tenant: Option<String>,
    /// Debit-positive signed amount (minor currency unit).
    pub amount_cents: i64,
    pub kind: String,
    pub source_ref: Option<String>,
    pub created_at: u64,
}

/// An affiliate-facing ledger line: amounts are shown in the affiliate's favour
/// (commission positive, clawback/payout negative).
#[derive(Debug, Clone, Serialize)]
pub struct LedgerEntry {
    pub id: String,
    pub referee_tenant: Option<String>,
    pub amount_cents: i64,
    pub kind: String,
    pub source_ref: Option<String>,
    pub created_at: u64,
}

/// A GL account's balance (debit-positive), for the operator's books view.
#[derive(Debug, Clone, Serialize)]
pub struct AccountBalance {
    pub account: String,
    pub balance_cents: i64,
}

/// Payout-request statuses.
pub mod payout_status {
    pub const REQUESTED: &str = "requested";
    pub const PAID: &str = "paid";
    pub const REJECTED: &str = "rejected";
}

/// An affiliate's request to cash out their balance to an address (e.g. USDT).
#[derive(Debug, Clone, Serialize)]
pub struct PayoutRequest {
    pub id: String,
    pub tenant_id: String,
    /// Payout method, e.g. `usdt`.
    pub method: String,
    pub address: String,
    pub amount_cents: i64,
    pub status: String,
    pub note: Option<String>,
    pub created_at: u64,
    pub processed_at: Option<u64>,
}

/// Deterministic 8-char uppercase referral code derived from the tenant id.
pub fn code_for(tenant_id: &str) -> String {
    let digest = Sha256::digest(tenant_id.as_bytes());
    hex::encode(digest)[..8].to_uppercase()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Builds a balanced commission transaction (Dr expense, Cr payable).
fn commission_postings(
    affiliate: &str,
    referee: &str,
    amount_cents: i64,
    source_ref: Option<&str>,
) -> Vec<Posting> {
    balanced(
        account::COMMISSION_EXPENSE,
        None,
        account::AFFILIATE_PAYABLE,
        Some(affiliate),
        Some(referee),
        amount_cents,
        kind::COMMISSION,
        source_ref,
    )
}

/// Builds a balanced clawback transaction (Dr payable, Cr expense).
fn clawback_postings(
    affiliate: &str,
    referee: &str,
    amount_cents: i64,
    source_ref: Option<&str>,
) -> Vec<Posting> {
    balanced(
        account::AFFILIATE_PAYABLE,
        Some(affiliate),
        account::COMMISSION_EXPENSE,
        None,
        Some(referee),
        amount_cents,
        kind::CLAWBACK,
        source_ref,
    )
}

/// Builds a balanced payout transaction (Dr payable, Cr cash).
fn payout_postings(affiliate: &str, amount_cents: i64, source_ref: Option<&str>) -> Vec<Posting> {
    balanced(
        account::AFFILIATE_PAYABLE,
        Some(affiliate),
        account::CASH,
        None,
        None,
        amount_cents,
        kind::PAYOUT,
        source_ref,
    )
}

/// Two-leg balanced transaction: debit `amount` to `debit_acct`, credit it from
/// `credit_acct`. The postings sum to zero.
#[allow(clippy::too_many_arguments)]
fn balanced(
    debit_acct: &str,
    debit_tenant: Option<&str>,
    credit_acct: &str,
    credit_tenant: Option<&str>,
    referee: Option<&str>,
    amount_cents: i64,
    kind: &str,
    source_ref: Option<&str>,
) -> Vec<Posting> {
    let txn_id = uuid::Uuid::new_v4().to_string();
    let created_at = now_secs();
    let mk = |account: &str, tenant: Option<&str>, amount: i64| Posting {
        id: uuid::Uuid::new_v4().to_string(),
        txn_id: txn_id.clone(),
        account: account.to_string(),
        tenant_id: tenant.map(str::to_string),
        referee_tenant: referee.map(str::to_string),
        amount_cents: amount,
        kind: kind.to_string(),
        source_ref: source_ref.map(str::to_string),
        created_at,
    };
    vec![
        mk(debit_acct, debit_tenant, amount_cents),
        mk(credit_acct, credit_tenant, -amount_cents),
    ]
}

#[allow(async_fn_in_trait)]
pub trait AffiliateStore: Clone + Send + Sync + 'static {
    async fn get_or_create_code(&self, tenant_id: &str) -> String;
    async fn resolve_code(&self, code: &str) -> Option<String>;
    async fn record_referral(&self, referee_tenant: &str, referrer_tenant: &str, code: &str);
    async fn get_referrer(&self, referee_tenant: &str) -> Option<String>;
    async fn referral_count(&self, referrer_tenant: &str) -> i64;

    /// Posts a balanced GL transaction.
    async fn post(&self, postings: Vec<Posting>);

    async fn accrue_commission(
        &self,
        affiliate: &str,
        referee: &str,
        amount_cents: i64,
        source_ref: Option<&str>,
    ) {
        if amount_cents == 0 {
            return;
        }
        self.post(commission_postings(
            affiliate,
            referee,
            amount_cents,
            source_ref,
        ))
        .await;
    }
    async fn clawback_commission(
        &self,
        affiliate: &str,
        referee: &str,
        amount_cents: i64,
        source_ref: Option<&str>,
    ) {
        if amount_cents == 0 {
            return;
        }
        self.post(clawback_postings(
            affiliate,
            referee,
            amount_cents,
            source_ref,
        ))
        .await;
    }
    async fn record_payout(&self, affiliate: &str, amount_cents: i64, source_ref: Option<&str>) {
        if amount_cents <= 0 {
            return;
        }
        self.post(payout_postings(affiliate, amount_cents, source_ref))
            .await;
    }

    /// Amount currently owed to the affiliate (negation of their payable balance).
    async fn balance_cents(&self, affiliate: &str) -> i64;
    /// Affiliate-facing ledger lines (commission +, clawback/payout −).
    async fn list_entries(&self, affiliate: &str, limit: i64) -> Vec<LedgerEntry>;
    /// Operator books: every GL account's debit-positive balance (these sum to 0).
    async fn account_balances(&self) -> Vec<AccountBalance>;

    /// Records an affiliate's payout request (status `requested`).
    async fn request_payout(
        &self,
        tenant_id: &str,
        method: &str,
        address: &str,
        amount_cents: i64,
    ) -> PayoutRequest;
    /// The affiliate's own payout requests, newest first.
    async fn list_payout_requests(&self, tenant_id: &str) -> Vec<PayoutRequest>;
    /// All pending (`requested`) payout requests, for the operator queue.
    async fn list_pending_payouts(&self) -> Vec<PayoutRequest>;
    /// Approves or rejects a pending request. Approval transitions it to `paid`
    /// and books the payout transaction; both are idempotent (a request already
    /// processed is left unchanged). Returns the updated request.
    async fn process_payout_request(
        &self,
        id: &str,
        approve: bool,
        note: Option<&str>,
    ) -> Option<PayoutRequest>;
}

/// Maps a payable-account posting to an affiliate-facing entry (favour sign).
fn entry_from_payable(p: &Posting) -> LedgerEntry {
    LedgerEntry {
        id: p.id.clone(),
        referee_tenant: p.referee_tenant.clone(),
        amount_cents: -p.amount_cents,
        kind: p.kind.clone(),
        source_ref: p.source_ref.clone(),
        created_at: p.created_at,
    }
}

// ── Memory implementation ──────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct MemoryAffiliateStore {
    codes: Arc<RwLock<HashMap<String, String>>>,
    referrals: Arc<RwLock<HashMap<String, (String, String)>>>,
    postings: Arc<RwLock<Vec<Posting>>>,
    payouts: Arc<RwLock<Vec<PayoutRequest>>>,
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
    async fn referral_count(&self, referrer_tenant: &str) -> i64 {
        self.referrals
            .read()
            .map(|r| r.values().filter(|(t, _)| t == referrer_tenant).count() as i64)
            .unwrap_or(0)
    }
    async fn post(&self, postings: Vec<Posting>) {
        if let Ok(mut l) = self.postings.write() {
            l.extend(postings);
        }
    }
    async fn balance_cents(&self, affiliate: &str) -> i64 {
        let owed: i64 = self
            .postings
            .read()
            .map(|l| {
                l.iter()
                    .filter(|p| {
                        p.account == account::AFFILIATE_PAYABLE
                            && p.tenant_id.as_deref() == Some(affiliate)
                    })
                    .map(|p| p.amount_cents)
                    .sum()
            })
            .unwrap_or(0);
        -owed
    }
    async fn list_entries(&self, affiliate: &str, limit: i64) -> Vec<LedgerEntry> {
        let Ok(l) = self.postings.read() else {
            return Vec::new();
        };
        let mut entries: Vec<LedgerEntry> = l
            .iter()
            .filter(|p| {
                p.account == account::AFFILIATE_PAYABLE && p.tenant_id.as_deref() == Some(affiliate)
            })
            .map(entry_from_payable)
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.created_at));
        entries.truncate(limit.max(0) as usize);
        entries
    }
    async fn account_balances(&self) -> Vec<AccountBalance> {
        let Ok(l) = self.postings.read() else {
            return Vec::new();
        };
        let mut by_acct: HashMap<String, i64> = HashMap::new();
        for p in l.iter() {
            *by_acct.entry(p.account.clone()).or_insert(0) += p.amount_cents;
        }
        let mut out: Vec<AccountBalance> = by_acct
            .into_iter()
            .map(|(account, balance_cents)| AccountBalance {
                account,
                balance_cents,
            })
            .collect();
        out.sort_by(|a, b| a.account.cmp(&b.account));
        out
    }
    async fn request_payout(
        &self,
        tenant_id: &str,
        method: &str,
        address: &str,
        amount_cents: i64,
    ) -> PayoutRequest {
        let req = PayoutRequest {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.to_string(),
            method: method.to_string(),
            address: address.to_string(),
            amount_cents,
            status: payout_status::REQUESTED.to_string(),
            note: None,
            created_at: now_secs(),
            processed_at: None,
        };
        if let Ok(mut p) = self.payouts.write() {
            p.push(req.clone());
        }
        req
    }
    async fn list_payout_requests(&self, tenant_id: &str) -> Vec<PayoutRequest> {
        let Ok(p) = self.payouts.read() else {
            return Vec::new();
        };
        let mut out: Vec<PayoutRequest> = p
            .iter()
            .filter(|r| r.tenant_id == tenant_id)
            .cloned()
            .collect();
        out.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        out
    }
    async fn list_pending_payouts(&self) -> Vec<PayoutRequest> {
        let Ok(p) = self.payouts.read() else {
            return Vec::new();
        };
        let mut out: Vec<PayoutRequest> = p
            .iter()
            .filter(|r| r.status == payout_status::REQUESTED)
            .cloned()
            .collect();
        out.sort_by_key(|r| r.created_at);
        out
    }
    async fn process_payout_request(
        &self,
        id: &str,
        approve: bool,
        note: Option<&str>,
    ) -> Option<PayoutRequest> {
        let (claimed, was_requested) = {
            let mut p = self.payouts.write().ok()?;
            let r = p.iter_mut().find(|r| r.id == id)?;
            let was_requested = r.status == payout_status::REQUESTED;
            if was_requested {
                r.status = if approve {
                    payout_status::PAID
                } else {
                    payout_status::REJECTED
                }
                .to_string();
                r.processed_at = Some(now_secs());
                r.note = note.map(str::to_string);
            }
            (r.clone(), was_requested)
        };
        if was_requested && approve {
            self.record_payout(&claimed.tenant_id, claimed.amount_cents, Some(&claimed.id))
                .await;
        }
        Some(claimed)
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

impl AffiliateStore for PostgresAffiliateStore {
    async fn get_or_create_code(&self, tenant_id: &str) -> String {
        let code = code_for(tenant_id);
        let _ = sqlx::query(
            "INSERT INTO af_affiliate_codes (tenant_id, code, created_at) VALUES ($1, $2, $3) \
             ON CONFLICT (tenant_id) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(&code)
        .bind(now_secs() as i64)
        .execute(&self.pool)
        .await;
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
        .bind(now_secs() as i64)
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
    async fn referral_count(&self, referrer_tenant: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::bigint FROM af_referrals WHERE referrer_tenant = $1",
        )
        .bind(referrer_tenant)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0)
    }
    async fn post(&self, postings: Vec<Posting>) {
        for p in postings {
            let _ = sqlx::query(
                "INSERT INTO af_ledger_postings \
                   (id, txn_id, account, tenant_id, referee_tenant, amount_cents, kind, source_ref, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(&p.id)
            .bind(&p.txn_id)
            .bind(&p.account)
            .bind(&p.tenant_id)
            .bind(&p.referee_tenant)
            .bind(p.amount_cents)
            .bind(&p.kind)
            .bind(&p.source_ref)
            .bind(p.created_at as i64)
            .execute(&self.pool)
            .await;
        }
    }
    async fn balance_cents(&self, affiliate: &str) -> i64 {
        let owed = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(amount_cents), 0)::bigint FROM af_ledger_postings \
             WHERE account = $1 AND tenant_id = $2",
        )
        .bind(account::AFFILIATE_PAYABLE)
        .bind(affiliate)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);
        -owed
    }
    async fn list_entries(&self, affiliate: &str, limit: i64) -> Vec<LedgerEntry> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            referee_tenant: Option<String>,
            amount_cents: i64,
            kind: String,
            source_ref: Option<String>,
            created_at: i64,
        }
        sqlx::query_as::<_, Row>(
            "SELECT id, referee_tenant, amount_cents, kind, source_ref, created_at \
             FROM af_ledger_postings WHERE account = $1 AND tenant_id = $2 \
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(account::AFFILIATE_PAYABLE)
        .bind(affiliate)
        .bind(limit.max(0))
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| LedgerEntry {
            id: r.id,
            referee_tenant: r.referee_tenant,
            amount_cents: -r.amount_cents,
            kind: r.kind,
            source_ref: r.source_ref,
            created_at: r.created_at as u64,
        })
        .collect()
    }
    async fn account_balances(&self) -> Vec<AccountBalance> {
        sqlx::query_as::<_, (String, i64)>(
            "SELECT account, COALESCE(SUM(amount_cents), 0)::bigint \
             FROM af_ledger_postings GROUP BY account ORDER BY account",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(account, balance_cents)| AccountBalance {
            account,
            balance_cents,
        })
        .collect()
    }
    async fn request_payout(
        &self,
        tenant_id: &str,
        method: &str,
        address: &str,
        amount_cents: i64,
    ) -> PayoutRequest {
        let req = PayoutRequest {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.to_string(),
            method: method.to_string(),
            address: address.to_string(),
            amount_cents,
            status: payout_status::REQUESTED.to_string(),
            note: None,
            created_at: now_secs(),
            processed_at: None,
        };
        let _ = sqlx::query(
            "INSERT INTO af_payout_requests \
               (id, tenant_id, method, address, amount_cents, status, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&req.id)
        .bind(&req.tenant_id)
        .bind(&req.method)
        .bind(&req.address)
        .bind(req.amount_cents)
        .bind(&req.status)
        .bind(req.created_at as i64)
        .execute(&self.pool)
        .await;
        req
    }
    async fn list_payout_requests(&self, tenant_id: &str) -> Vec<PayoutRequest> {
        payout_rows(
            sqlx::query_as::<_, PayoutRow>(
                "SELECT id, tenant_id, method, address, amount_cents, status, note, created_at, processed_at \
                 FROM af_payout_requests WHERE tenant_id = $1 ORDER BY created_at DESC",
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default(),
        )
    }
    async fn list_pending_payouts(&self) -> Vec<PayoutRequest> {
        payout_rows(
            sqlx::query_as::<_, PayoutRow>(
                "SELECT id, tenant_id, method, address, amount_cents, status, note, created_at, processed_at \
                 FROM af_payout_requests WHERE status = 'requested' ORDER BY created_at ASC",
            )
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default(),
        )
    }
    async fn process_payout_request(
        &self,
        id: &str,
        approve: bool,
        note: Option<&str>,
    ) -> Option<PayoutRequest> {
        let new_status = if approve {
            payout_status::PAID
        } else {
            payout_status::REJECTED
        };
        // Atomic claim: only a still-`requested` row transitions and returns.
        let claimed = sqlx::query_as::<_, (String, i64)>(
            "UPDATE af_payout_requests SET status = $2, processed_at = $3, note = $4 \
             WHERE id = $1 AND status = 'requested' RETURNING tenant_id, amount_cents",
        )
        .bind(id)
        .bind(new_status)
        .bind(now_secs() as i64)
        .bind(note)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();
        if let Some((tenant, amount)) = claimed {
            if approve {
                self.record_payout(&tenant, amount, Some(id)).await;
            }
        }
        payout_rows(
            sqlx::query_as::<_, PayoutRow>(
                "SELECT id, tenant_id, method, address, amount_cents, status, note, created_at, processed_at \
                 FROM af_payout_requests WHERE id = $1",
            )
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default(),
        )
        .into_iter()
        .next()
    }
}

#[derive(sqlx::FromRow)]
struct PayoutRow {
    id: String,
    tenant_id: String,
    method: String,
    address: String,
    amount_cents: i64,
    status: String,
    note: Option<String>,
    created_at: i64,
    processed_at: Option<i64>,
}

fn payout_rows(rows: Vec<PayoutRow>) -> Vec<PayoutRequest> {
    rows.into_iter()
        .map(|r| PayoutRequest {
            id: r.id,
            tenant_id: r.tenant_id,
            method: r.method,
            address: r.address,
            amount_cents: r.amount_cents,
            status: r.status,
            note: r.note,
            created_at: r.created_at as u64,
            processed_at: r.processed_at.map(|v| v as u64),
        })
        .collect()
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
    async fn referral_count(&self, referrer_tenant: &str) -> i64 {
        match self {
            Self::Memory(s) => s.referral_count(referrer_tenant).await,
            Self::Postgres(s) => s.referral_count(referrer_tenant).await,
        }
    }
    async fn post(&self, postings: Vec<Posting>) {
        match self {
            Self::Memory(s) => s.post(postings).await,
            Self::Postgres(s) => s.post(postings).await,
        }
    }
    async fn balance_cents(&self, affiliate: &str) -> i64 {
        match self {
            Self::Memory(s) => s.balance_cents(affiliate).await,
            Self::Postgres(s) => s.balance_cents(affiliate).await,
        }
    }
    async fn list_entries(&self, affiliate: &str, limit: i64) -> Vec<LedgerEntry> {
        match self {
            Self::Memory(s) => s.list_entries(affiliate, limit).await,
            Self::Postgres(s) => s.list_entries(affiliate, limit).await,
        }
    }
    async fn account_balances(&self) -> Vec<AccountBalance> {
        match self {
            Self::Memory(s) => s.account_balances().await,
            Self::Postgres(s) => s.account_balances().await,
        }
    }
    async fn request_payout(
        &self,
        tenant_id: &str,
        method: &str,
        address: &str,
        amount_cents: i64,
    ) -> PayoutRequest {
        match self {
            Self::Memory(s) => {
                s.request_payout(tenant_id, method, address, amount_cents)
                    .await
            }
            Self::Postgres(s) => {
                s.request_payout(tenant_id, method, address, amount_cents)
                    .await
            }
        }
    }
    async fn list_payout_requests(&self, tenant_id: &str) -> Vec<PayoutRequest> {
        match self {
            Self::Memory(s) => s.list_payout_requests(tenant_id).await,
            Self::Postgres(s) => s.list_payout_requests(tenant_id).await,
        }
    }
    async fn list_pending_payouts(&self) -> Vec<PayoutRequest> {
        match self {
            Self::Memory(s) => s.list_pending_payouts().await,
            Self::Postgres(s) => s.list_pending_payouts().await,
        }
    }
    async fn process_payout_request(
        &self,
        id: &str,
        approve: bool,
        note: Option<&str>,
    ) -> Option<PayoutRequest> {
        match self {
            Self::Memory(s) => s.process_payout_request(id, approve, note).await,
            Self::Postgres(s) => s.process_payout_request(id, approve, note).await,
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

        store.record_referral("referee", "referrer", &code).await;
        store
            .record_referral("referee", "someone-else", &code)
            .await;
        assert_eq!(
            store.get_referrer("referee").await.as_deref(),
            Some("referrer")
        );
        store.record_referral("solo", "solo", "X").await;
        assert_eq!(store.get_referrer("solo").await, None);
        assert_eq!(store.referral_count("referrer").await, 1);
    }

    #[tokio::test]
    async fn double_entry_balance_and_books_stay_balanced() {
        let store = MemoryAffiliateStore::default();
        store.accrue_commission("r", "e", 1000, Some("evt1")).await;
        store.clawback_commission("r", "e", 300, Some("evt2")).await;
        store.record_payout("r", 200, None).await;

        // Amount owed = 1000 − 300 − 200 = 500.
        assert_eq!(store.balance_cents("r").await, 500);
        // Affiliate-facing entries: +1000, −300, −200.
        let entries = store.list_entries("r", 10).await;
        assert_eq!(entries.len(), 3);
        let sum: i64 = entries.iter().map(|e| e.amount_cents).sum();
        assert_eq!(sum, 500);

        // The books balance: every account's postings sum to zero overall.
        let total: i64 = store
            .account_balances()
            .await
            .iter()
            .map(|a| a.balance_cents)
            .sum();
        assert_eq!(total, 0, "double-entry postings must sum to zero");

        assert_eq!(store.balance_cents("other").await, 0);
    }

    #[tokio::test]
    async fn payout_request_approve_books_payout_and_is_idempotent() {
        let store = MemoryAffiliateStore::default();
        store.accrue_commission("r", "e", 1000, Some("evt1")).await;
        assert_eq!(store.balance_cents("r").await, 1000);

        let req = store.request_payout("r", "usdt", "TUSDTaddr", 400).await;
        assert_eq!(req.status, payout_status::REQUESTED);
        assert_eq!(store.list_pending_payouts().await.len(), 1);
        assert_eq!(store.list_payout_requests("r").await.len(), 1);

        let done = store
            .process_payout_request(&req.id, true, Some("sent"))
            .await
            .unwrap();
        assert_eq!(done.status, payout_status::PAID);
        assert_eq!(store.balance_cents("r").await, 600); // 1000 − 400
        assert!(store.list_pending_payouts().await.is_empty());

        // Re-processing does not double-book.
        let again = store
            .process_payout_request(&req.id, true, None)
            .await
            .unwrap();
        assert_eq!(again.status, payout_status::PAID);
        assert_eq!(store.balance_cents("r").await, 600);
    }

    #[tokio::test]
    async fn payout_request_reject_leaves_balance() {
        let store = MemoryAffiliateStore::default();
        store.accrue_commission("r", "e", 1000, None).await;
        let req = store.request_payout("r", "usdt", "addr", 400).await;
        let done = store
            .process_payout_request(&req.id, false, Some("invalid address"))
            .await
            .unwrap();
        assert_eq!(done.status, payout_status::REJECTED);
        assert_eq!(store.balance_cents("r").await, 1000);
    }

    #[test]
    fn commission_for_uses_configured_rate() {
        std::env::remove_var("AFFILIATE_COMMISSION_PCT");
        assert_eq!(commission_for(10_000), 0);
    }
}
