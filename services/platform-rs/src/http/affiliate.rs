// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;
use crate::affiliate::{AffiliateStore, LedgerEntry};

#[derive(serde::Serialize)]
struct AffiliateInfo {
    /// The tenant's shareable referral code.
    code: String,
    /// How many tenants this affiliate has referred.
    referral_count: i64,
    /// Accrued balance (commissions − clawbacks − payouts), minor currency unit.
    balance_cents: i64,
    /// Configured commission rate (percent of a referral's paid invoices).
    commission_pct: f64,
    entries: Vec<LedgerEntry>,
}

/// The caller's own affiliate dashboard: code, referrals, balance and ledger.
async fn affiliate_me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Json<AffiliateInfo> {
    let tenant_id = effective_tenant_id(&claims, "");
    let store = &state.affiliate_store;
    Json(AffiliateInfo {
        code: store.get_or_create_code(&tenant_id).await,
        referral_count: store.referral_count(&tenant_id).await,
        balance_cents: store.balance_cents(&tenant_id).await,
        commission_pct: crate::affiliate::commission_pct(),
        entries: store.list_entries(&tenant_id, 50).await,
    })
}

#[derive(serde::Deserialize)]
struct PayoutBody {
    /// The affiliate (referrer) tenant being paid out.
    tenant_id: String,
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
    state
        .affiliate_store
        .add_entry(LedgerEntry {
            id: uuid::Uuid::new_v4().to_string(),
            referrer_tenant: body.tenant_id.clone(),
            referee_tenant: None,
            amount_cents: -body.amount_cents,
            kind: crate::affiliate::kind::PAYOUT.to_string(),
            source_ref: None,
            created_at: crate::execution::unix_now(),
        })
        .await;
    let balance = state.affiliate_store.balance_cents(&body.tenant_id).await;
    Ok(Json(
        serde_json::json!({ "ok": true, "balance_cents": balance }),
    ))
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/affiliate/me", get(affiliate_me_handler))
        .route("/v1/affiliate/payout", post(affiliate_payout_handler))
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
        assert_eq!(v["balance_cents"], 0);
        assert_eq!(v["referral_count"], 0);
    }
}
