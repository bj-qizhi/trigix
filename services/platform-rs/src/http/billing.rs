// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn billing_status_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Json<BillingStatusResponse> {
    let tenant_id = effective_tenant_id(&claims, "");
    let status = state.billing_store.billing_status(&tenant_id);
    let (_, subscription_id) = state.billing_store.get_stripe_ids(&tenant_id);
    let has_subscription = subscription_id.is_some();
    let stripe_enabled = state.stripe_client.is_some();
    let reset_in_secs = secs_until_quota_reset();
    Json(BillingStatusResponse {
        status,
        has_subscription,
        stripe_enabled,
        reset_in_secs,
    })
}

async fn billing_history_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<HistoryQuery>,
) -> Json<Vec<crate::billing::UsageSummary>> {
    let tenant_id = effective_tenant_id(&claims, "");
    let months = q.months.clamp(1, 24);
    Json(state.billing_store.get_usage_history(&tenant_id, months))
}

async fn billing_checkout_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CheckoutBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let stripe = state
        .stripe_client
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Stripe not configured"))?;
    let tenant_id = effective_tenant_id(&claims, "");
    let price_id =
        tier_to_price_id(&body.tier).ok_or_else(|| ApiError::bad_request("Unknown tier"))?;

    let (customer_id, _) = state.billing_store.get_stripe_ids(&tenant_id);
    let customer_email = if let Some(uid) = claims.as_ref().and_then(|c| c.user_id.clone()) {
        let store = Arc::clone(&state.user_store);
        tokio::task::spawn_blocking(move || store.find_by_id(&uid))
            .await
            .ok()
            .flatten()
            .map(|u| u.email)
    } else {
        None
    };

    let base =
        std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let success_url = format!("{base}/account?billing=success");
    let cancel_url = format!("{base}/account?billing=canceled");

    let url = stripe
        .create_checkout_session(
            &price_id,
            &body.tier,
            &tenant_id,
            customer_id.as_deref(),
            customer_email.as_deref(),
            &success_url,
            &cancel_url,
        )
        .await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(Json(serde_json::json!({ "url": url })))
}

async fn billing_portal_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let stripe = state
        .stripe_client
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Stripe not configured"))?;
    let tenant_id = effective_tenant_id(&claims, "");
    let (customer_id, _) = state.billing_store.get_stripe_ids(&tenant_id);
    let customer_id =
        customer_id.ok_or_else(|| ApiError::bad_request("No Stripe customer found"))?;

    let base =
        std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let return_url = format!("{base}/account");

    let url = stripe
        .create_portal_session(&customer_id, &return_url)
        .await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(Json(serde_json::json!({ "url": url })))
}

async fn stripe_webhook_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
    if secret.is_empty() {
        return (StatusCode::OK, "webhook secret not configured").into_response();
    }
    if !StripeClient::verify_webhook_signature(&body, sig, &secret) {
        return (StatusCode::BAD_REQUEST, "invalid signature").into_response();
    }

    let Ok(event) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return (StatusCode::BAD_REQUEST, "invalid json").into_response();
    };

    let event_type = event["type"].as_str().unwrap_or("");
    let obj = &event["data"]["object"];

    match event_type {
        "checkout.session.completed" => {
            let tenant_id = obj["metadata"]["tenant_id"].as_str().unwrap_or("");
            let tier = obj["metadata"]["tier"].as_str().unwrap_or("pro");
            let customer = obj["customer"].as_str();
            let sub_id = obj["subscription"].as_str();
            if !tenant_id.is_empty() {
                let quota = match tier {
                    "pro" => TenantQuota::pro(tenant_id),
                    "business" => TenantQuota::business(tenant_id),
                    "enterprise" => TenantQuota::unlimited(tenant_id),
                    _ => TenantQuota::pro(tenant_id),
                };
                state.billing_store.set_quota(quota);
                state
                    .billing_store
                    .set_stripe_ids(tenant_id, customer, sub_id);
                info!(
                    tenant_id,
                    tier, "Stripe checkout.session.completed → quota upgraded"
                );
            }
        }
        "customer.subscription.updated" => {
            let customer = obj["customer"].as_str().unwrap_or("");
            let sub_id = obj["id"].as_str();
            let tier = obj["items"]["data"][0]["price"]["id"]
                .as_str()
                .and_then(|pid| price_id_to_tier(pid));
            if let Some(tenant_id) = state.billing_store.get_tenant_by_stripe_customer(customer) {
                if let Some(ref t) = tier {
                    let quota = match t.as_str() {
                        "pro" => TenantQuota::pro(&tenant_id),
                        "business" => TenantQuota::business(&tenant_id),
                        "enterprise" => TenantQuota::unlimited(&tenant_id),
                        _ => TenantQuota::pro(&tenant_id),
                    };
                    state.billing_store.set_quota(quota);
                }
                state
                    .billing_store
                    .set_stripe_ids(&tenant_id, Some(customer), sub_id);
                info!(tenant_id, "Stripe customer.subscription.updated");
            }
        }
        "customer.subscription.deleted" => {
            let customer = obj["customer"].as_str().unwrap_or("");
            if let Some(tenant_id) = state.billing_store.get_tenant_by_stripe_customer(customer) {
                state.billing_store.set_quota(TenantQuota::free(&tenant_id));
                state
                    .billing_store
                    .set_stripe_ids(&tenant_id, Some(customer), None);
                info!(
                    tenant_id,
                    "Stripe customer.subscription.deleted → downgraded to free"
                );
            }
        }
        _ => {}
    }

    StatusCode::OK.into_response()
}

async fn get_token_usage_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TokenUsageQuery>,
) -> Json<crate::token_usage::TokenUsageSummary> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let days = query.days.unwrap_or(30).min(365);
    let since = crate::execution::unix_now().saturating_sub(days * 86400);
    let summary = state.token_usage_store.summarize(&tenant_id, since).await;
    Json(summary)
}

// ── Execution stats ───────────────────────────────────────────────────────

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/token-usage", get(get_token_usage_handler))
        .route("/v1/billing/status", get(billing_status_handler))
        .route("/v1/billing/history", get(billing_history_handler))
        .route("/v1/billing/checkout", post(billing_checkout_handler))
        .route("/v1/billing/portal", post(billing_portal_handler))
        .route("/v1/stripe/webhook", post(stripe_webhook_handler))
}
