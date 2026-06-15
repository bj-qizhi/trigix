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
    // Reject events whose signed timestamp is outside the tolerance window
    // (default 5 min, matching Stripe's SDKs) so a captured request can't be
    // replayed indefinitely.
    let tolerance = std::env::var("STRIPE_WEBHOOK_TOLERANCE_SECS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(300);
    if !crate::stripe_billing::webhook_timestamp_fresh(
        sig,
        crate::execution::unix_now() as i64,
        tolerance,
    ) {
        return (StatusCode::BAD_REQUEST, "stale webhook timestamp").into_response();
    }

    let Ok(event) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return (StatusCode::BAD_REQUEST, "invalid json").into_response();
    };

    // Idempotency: Stripe retries delivery on any non-2xx/slow response, so
    // process each event id at most once to avoid double-applying upgrades or
    // clawbacks. Events without an id (shouldn't happen) fall through.
    let event_id = event["id"].as_str().unwrap_or("");
    if !event_id.is_empty() && !state.billing_store.mark_stripe_event_processed(event_id) {
        return StatusCode::OK.into_response();
    }

    let event_type = event["type"].as_str().unwrap_or("");
    let obj = &event["data"]["object"];
    apply_stripe_event(&state, event_type, obj);

    // Server-side conversion → PostHog, credited to the tenant's first-touch
    // acquisition channel. Fire-and-forget so the webhook still returns 200 fast.
    if event_type == "checkout.session.completed" {
        if let Some(posthog) = state.posthog.clone() {
            let tenant_id = obj["metadata"]["tenant_id"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let tier = obj["metadata"]["tier"]
                .as_str()
                .unwrap_or("pro")
                .to_string();
            if !tenant_id.is_empty() {
                let attribution_store = Arc::clone(&state.attribution_store);
                tokio::spawn(async move {
                    use crate::attribution::AttributionStore;
                    let attr = attribution_store.get(&tenant_id).await;
                    let distinct_id = attr
                        .as_ref()
                        .and_then(|a| a.distinct_id.clone())
                        .unwrap_or_else(|| format!("tenant:{tenant_id}"));
                    let props = conversion_properties(&tenant_id, &tier, attr.as_ref());
                    posthog
                        .capture(&distinct_id, "subscription_started", props)
                        .await;
                });
            }
        }
    }

    StatusCode::OK.into_response()
}

/// Builds the PostHog properties for a paid-conversion event, merging the
/// tenant's first-touch attribution so revenue is credited to its channel.
fn conversion_properties(
    tenant_id: &str,
    tier: &str,
    attr: Option<&crate::attribution::AttributionRecord>,
) -> serde_json::Value {
    let mut props = serde_json::json!({ "tier": tier, "tenant_id": tenant_id });
    if let (Some(a), Some(map)) = (attr, props.as_object_mut()) {
        for (key, val) in [
            ("utm_source", &a.utm_source),
            ("utm_medium", &a.utm_medium),
            ("utm_campaign", &a.utm_campaign),
            ("utm_term", &a.utm_term),
            ("utm_content", &a.utm_content),
            ("referrer", &a.referrer),
            ("landing_page", &a.landing_page),
        ] {
            if let Some(v) = val {
                map.insert(key.to_string(), serde_json::Value::String(v.clone()));
            }
        }
    }
    props
}

/// Applies a verified Stripe webhook event to billing state. Pure over `state`
/// (no signature/HTTP concerns) so every branch — including credit-clawback — is
/// unit-testable without env-based signature setup.
fn apply_stripe_event(state: &AppState, event_type: &str, obj: &serde_json::Value) {
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
                // Attribute converted revenue (minor currency unit) to the tenant
                // so the acquisition-channel ROI view can sum it.
                if let Some(cents) = obj["amount_total"].as_i64() {
                    if cents > 0 {
                        state.billing_store.add_revenue(tenant_id, cents);
                    }
                }
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
        // Credit-clawback: a refund or chargeback revokes the granted tier so a
        // fraudulent/charged-back upgrade can't keep its paid quota. The customer
        // id lives directly on the charge object for these events.
        "charge.refunded" | "charge.dispute.created" | "charge.dispute.funds_withdrawn" => {
            let customer = obj["customer"].as_str().unwrap_or("");
            if !customer.is_empty() {
                if let Some(tenant_id) = state.billing_store.get_tenant_by_stripe_customer(customer)
                {
                    state.billing_store.set_quota(TenantQuota::free(&tenant_id));
                    state
                        .billing_store
                        .set_stripe_ids(&tenant_id, Some(customer), None);
                    info!(
                        tenant_id,
                        event_type, "Stripe refund/dispute → quota clawed back to free"
                    );
                }
            }
        }
        _ => {}
    }
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

#[cfg(test)]
mod tests {
    use crate::http::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::json;
    use tower::ServiceExt;

    #[test]
    fn conversion_properties_merge_attribution() {
        let attr = crate::attribution::AttributionRecord {
            tenant_id: "t1".into(),
            utm_source: Some("google".into()),
            utm_campaign: Some("launch".into()),
            ..Default::default()
        };
        let props = super::conversion_properties("t1", "pro", Some(&attr));
        assert_eq!(props["tier"], "pro");
        assert_eq!(props["tenant_id"], "t1");
        assert_eq!(props["utm_source"], "google");
        assert_eq!(props["utm_campaign"], "launch");
        // Absent fields are omitted, not null.
        assert!(props.get("utm_medium").is_none());

        // No attribution → just tier + tenant.
        let bare = super::conversion_properties("t1", "free", None);
        assert_eq!(bare["tier"], "free");
        assert!(bare.get("utm_source").is_none());
    }

    #[test]
    fn refund_claws_back_quota_to_free() {
        use crate::billing::BillingStore;
        let state = default_app_state();
        let tenant = "tenant-claw";
        let customer = "cus_claw";

        // Paid upgrade grants the pro tier.
        super::apply_stripe_event(
            &state,
            "checkout.session.completed",
            &json!({
                "metadata": {"tenant_id": tenant, "tier": "pro"},
                "customer": customer,
                "subscription": "sub_claw"
            }),
        );
        assert_eq!(state.billing_store.get_quota(tenant).tier, "pro");

        // A refund/chargeback claws the tier back to free.
        super::apply_stripe_event(&state, "charge.refunded", &json!({"customer": customer}));
        assert_eq!(state.billing_store.get_quota(tenant).tier, "free");

        // A dispute likewise revokes the grant.
        super::apply_stripe_event(
            &state,
            "checkout.session.completed",
            &json!({
                "metadata": {"tenant_id": tenant, "tier": "business"},
                "customer": customer,
                "subscription": "sub_claw2"
            }),
        );
        assert_eq!(state.billing_store.get_quota(tenant).tier, "business");
        super::apply_stripe_event(
            &state,
            "charge.dispute.created",
            &json!({"customer": customer}),
        );
        assert_eq!(state.billing_store.get_quota(tenant).tier, "free");
    }

    #[tokio::test]
    async fn billing_status_endpoint_returns_ok() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/billing/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.get("quota").is_some());
        assert!(v.get("usage").is_some());
        assert!(v.get("usage_pct").is_some());
    }

    // ── Slice 358: Webhook replay protection ──────────────────────────────────
}
