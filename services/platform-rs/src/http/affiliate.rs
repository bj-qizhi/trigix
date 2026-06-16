// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;
use crate::affiliate::{
    AccountBalance, AffiliateStore, CurrencyAmount, LedgerEntry, PayoutRequest,
};

#[derive(serde::Serialize)]
struct AffiliateInfo {
    /// The tenant's shareable referral code.
    code: String,
    /// How many tenants this affiliate has referred.
    referral_count: i64,
    /// Owed balance per currency (commissions − clawbacks − payouts).
    balances: Vec<CurrencyAmount>,
    /// Configured commission rate (percent of a referral's paid invoices).
    commission_pct: f64,
    entries: Vec<LedgerEntry>,
    payout_requests: Vec<PayoutRequest>,
}

/// The caller's own affiliate dashboard: code, referrals, balances, ledger, payouts.
async fn affiliate_me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Json<AffiliateInfo> {
    let tenant_id = effective_tenant_id(&claims, "");
    let store = &state.affiliate_store;
    Json(AffiliateInfo {
        code: store.get_or_create_code(&tenant_id).await,
        referral_count: store.referral_count(&tenant_id).await,
        balances: store.balances(&tenant_id).await,
        commission_pct: crate::affiliate::commission_pct(),
        entries: store.list_entries(&tenant_id, 50).await,
        payout_requests: store.list_payout_requests(&tenant_id).await,
    })
}

#[derive(serde::Deserialize)]
struct PayoutRequestBody {
    /// Payout method; defaults to `usdt`.
    method: Option<String>,
    /// Destination address (e.g. a USDT wallet).
    address: String,
    /// Currency to cash out; defaults to `usd`.
    currency: Option<String>,
    amount_cents: i64,
}

/// An affiliate requests a cashout of (part of) their balance to an address.
async fn affiliate_request_payout_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<PayoutRequestBody>,
) -> Result<Json<PayoutRequest>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    if body.amount_cents <= 0 {
        return Err(ApiError::bad_request("amount_cents must be positive"));
    }
    if body.address.trim().is_empty() {
        return Err(ApiError::bad_request("address is required"));
    }
    let currency = body.currency.unwrap_or_else(|| "usd".to_string());
    let balance = state
        .affiliate_store
        .balance_for(&tenant_id, &currency)
        .await;
    if body.amount_cents > balance {
        return Err(ApiError::bad_request("amount exceeds available balance"));
    }
    let method = body.method.unwrap_or_else(|| "usdt".to_string());
    Ok(Json(
        state
            .affiliate_store
            .request_payout(
                &tenant_id,
                &method,
                body.address.trim(),
                &currency,
                body.amount_cents,
            )
            .await,
    ))
}

/// Operator queue of pending payout requests. Admin-only.
async fn affiliate_admin_payouts_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<PayoutRequest>>, ApiError> {
    require_admin(&claims)?;
    Ok(Json(state.affiliate_store.list_pending_payouts().await))
}

#[derive(serde::Deserialize)]
struct ProcessPayoutBody {
    id: String,
    approve: bool,
    note: Option<String>,
}

/// Operator approves (books the payout) or rejects a pending request. Admin-only.
async fn affiliate_admin_process_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<ProcessPayoutBody>,
) -> Result<Json<PayoutRequest>, ApiError> {
    require_admin(&claims)?;
    state
        .affiliate_store
        .process_payout_request(&body.id, body.approve, body.note.as_deref())
        .await
        .map(Json)
        .ok_or_else(|| ApiError::not_found("payout request not found"))
}

#[derive(serde::Deserialize)]
struct PayoutBody {
    /// The affiliate (referrer) tenant being paid out.
    tenant_id: String,
    /// Currency being paid out; defaults to `usd`.
    currency: Option<String>,
    /// Positive amount to disburse (minor currency unit); recorded as a debit.
    amount_cents: i64,
}

/// Records an operator payout that debits an affiliate's accrued balance. The
/// actual money movement is out-of-band; this only books the ledger entry.
async fn affiliate_payout_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<PayoutBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&claims)?;
    if body.amount_cents <= 0 {
        return Err(ApiError::bad_request("amount_cents must be positive"));
    }
    let currency = body.currency.unwrap_or_else(|| "usd".to_string());
    state
        .affiliate_store
        .record_payout(&body.tenant_id, &currency, body.amount_cents, None)
        .await;
    let balance = state
        .affiliate_store
        .balance_for(&body.tenant_id, &currency)
        .await;
    Ok(Json(
        serde_json::json!({ "ok": true, "balance_cents": balance }),
    ))
}

/// Operator books: every GL account balance (these sum to zero — proof the
/// double-entry ledger balances). Admin-only.
async fn affiliate_ledger_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<AccountBalance>>, ApiError> {
    require_admin(&claims)?;
    Ok(Json(state.affiliate_store.account_balances().await))
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/affiliate/me", get(affiliate_me_handler))
        .route("/v1/affiliate/payout", post(affiliate_payout_handler))
        .route(
            "/v1/affiliate/payout-request",
            post(affiliate_request_payout_handler),
        )
        .route("/v1/affiliate/admin/ledger", get(affiliate_ledger_handler))
        .route(
            "/v1/affiliate/admin/payouts",
            get(affiliate_admin_payouts_handler),
        )
        .route(
            "/v1/affiliate/admin/payouts/process",
            post(affiliate_admin_process_handler),
        )
}

#[cfg(test)]
mod tests {
    use crate::http::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn affiliate_me_returns_code_and_zero_balance() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/affiliate/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v["code"].as_str().is_some_and(|c| !c.is_empty()));
        assert_eq!(v["balances"].as_array().map(|a| a.len()), Some(0));
        assert_eq!(v["referral_count"], 0);
    }
}
