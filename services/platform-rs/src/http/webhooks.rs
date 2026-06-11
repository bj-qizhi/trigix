// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn trigger_webhook(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<ExecutionRecord>), ApiError> {
    if DRAINING.load(Ordering::Relaxed) {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Server is shutting down; new executions are not accepted.".to_string(),
        });
    }
    let webhook = state
        .webhook_store
        .get_by_token(&token)
        .await
        .map_err(ApiError::from)?;

    // Capture delivery metadata before consuming webhook fields
    let delivery_token = webhook.token.clone();
    let delivery_tenant = webhook.tenant_id.clone();
    let delivery_store = state.webhook_store.clone();

    let inner_result: Result<ExecutionRecord, ApiError> = async {
        state
            .billing_store
            .check_execution_quota(&webhook.tenant_id)
            .map_err(|e| ApiError {
                status: StatusCode::PAYMENT_REQUIRED,
                message: e,
            })?;

        // Replay-attack protection: reject requests where the timestamp header is
        // absent (when a secret is set) or outside a ±5-minute window.
        if webhook.secret.is_some() {
            const WINDOW_SECS: u64 = 300;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let ts: u64 = headers
                .get("x-trigix-timestamp")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if ts == 0 || now.abs_diff(ts) > WINDOW_SECS {
                return Err(ApiError {
                    status: StatusCode::BAD_REQUEST,
                    message: format!(
                        "Missing or stale X-Trigix-Timestamp (window: ±{WINDOW_SECS}s). \
                         Send the current Unix timestamp in the header."
                    ),
                });
            }
        }

        // If the webhook has a secret, validate the HMAC-SHA256 signature.
        if let Some(secret) = &webhook.secret {
            let sig = headers
                .get("x-webhook-signature")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !crate::webhook::verify_signature(secret, &body, sig) {
                return Err(ApiError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "Invalid webhook signature".to_string(),
                });
            }
        }

        let mut input_json = if body.is_empty() {
            "{}".to_string()
        } else {
            String::from_utf8(body.to_vec()).unwrap_or_else(|_| "{}".to_string())
        };

        // Reject paused webhooks
        if webhook.paused {
            return Err(ApiError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "Webhook is paused".to_string(),
            });
        }

        // Per-webhook rate limit (in-memory sliding window)
        if let Some(max_per_min) = webhook.max_calls_per_minute {
            if !state
                .rate_limiter
                .check_with_limit(&format!("wh:{}", &webhook.token), max_per_min)
            {
                return Err(ApiError {
                    status: StatusCode::TOO_MANY_REQUESTS,
                    message: format!("Webhook rate limit exceeded ({max_per_min}/min)"),
                });
            }
        }

        // Evaluate optional condition expression against payload
        if let Some(cond) = &webhook.condition_expr {
            if !cond.is_empty() {
                let payload: serde_json::Value =
                    serde_json::from_str(&input_json).unwrap_or(serde_json::Value::Null);
                if !crate::webhook::eval_condition(cond, &payload) {
                    // Condition not met — accepted but no execution started (202 Accepted)
                    return Err(ApiError {
                        status: StatusCode::ACCEPTED,
                        message: format!("filtered: condition not met ({cond})"),
                    });
                }
            }
        }

        // Apply optional payload transform script
        if let Some(script) = &webhook.payload_transform_script {
            if !script.is_empty() {
                input_json = crate::webhook::apply_payload_transform(script, &input_json);
            }
        }

        let version = state
            .workflow_service
            .get_version(&webhook.tenant_id, &webhook.workflow_version_id)
            .await?;

        let graph = resolve_graph_credentials(
            version.graph,
            &state.credential_store,
            &state.env_store,
            &webhook.tenant_id,
            DEFAULT_SET,
        )
        .await;

        let record = state
            .execution_service
            .start(StartExecutionRequest {
                tenant_id: webhook.tenant_id,
                workflow_id: webhook.workflow_id,
                workflow_version_id: webhook.workflow_version_id,
                graph,
                input_json,
                label: None,
                callback_url: None,
                trigger_type: Some("webhook".to_string()),
                dry_run: false,
                retried_from: None,
            })
            .await?;
        let prev_used = state
            .billing_store
            .billing_status(&record.tenant_id)
            .usage
            .executions_used;
        state.billing_store.increment_execution(&record.tenant_id);
        spawn_quota_alert(&state, &record.tenant_id, prev_used);
        state.audit_store.record(
            &record.tenant_id,
            audit_action::EXECUTION_STARTED,
            "execution",
            &record.id,
            None,
        );
        Ok(record)
    }
    .await;

    // Record delivery outcome regardless of success or failure
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let delivery = crate::webhook::WebhookDelivery {
        id: uuid::Uuid::new_v4().to_string(),
        webhook_token: delivery_token,
        tenant_id: delivery_tenant,
        delivered_at: now,
        status_code: match &inner_result {
            Ok(_) => Some(202),
            Err(e) => Some(e.status.as_u16() as i32),
        },
        success: inner_result.is_ok(),
        error_message: match &inner_result {
            Err(e) => Some(e.message.clone()),
            Ok(_) => None,
        },
        execution_id: match &inner_result {
            Ok(r) => Some(r.id.clone()),
            Err(_) => None,
        },
    };
    delivery_store.record_delivery(delivery).await;
    inner_result.map(|r| (StatusCode::ACCEPTED, Json(r)))
}

async fn list_webhooks_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<CredentialQuery>,
) -> Result<Json<Vec<crate::webhook::WebhookRecord>>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let records = state
        .webhook_store
        .list_by_tenant(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(records))
}

async fn delete_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Query(query): Query<CredentialQuery>,
) -> Result<StatusCode, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .webhook_store
        .delete_by_token(&tenant_id, &token)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_webhook_deliveries_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(q): Query<DeliveryQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let deliveries = state.webhook_store.list_deliveries(&token, limit).await;
    Json(deliveries)
}

async fn update_webhook_condition_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Json(body): Json<SetWebhookConditionBody>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_condition(&tenant_id, &token, body.condition_expr)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn update_webhook_rate_limit_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Json(body): Json<SetWebhookRateLimitBody>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_rate_limit(&tenant_id, &token, body.max_calls_per_minute)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn rotate_webhook_secret_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    let new_secret = format!(
        "{}{}",
        uuid::Uuid::new_v4().to_string().replace('-', ""),
        uuid::Uuid::new_v4().to_string().replace('-', ""),
    );
    state
        .webhook_store
        .rotate_secret(&tenant_id, &token, new_secret.clone())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::json!({ "secret": new_secret })))
}

async fn pause_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_paused(&tenant_id, &token, true)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn resume_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_paused(&tenant_id, &token, false)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn set_payload_transform_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(token): Path<String>,
    Json(body): Json<SetPayloadTransformBody>,
) -> Result<Json<crate::webhook::WebhookRecord>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, "");
    state
        .webhook_store
        .set_payload_transform(&tenant_id, &token, body.script)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/webhooks", get(list_webhooks_handler))
        .route(
            "/v1/webhooks/:token",
            get(method_not_allowed)
                .post(trigger_webhook)
                .delete(delete_webhook_handler),
        )
        .route(
            "/v1/webhooks/:token/deliveries",
            get(list_webhook_deliveries_handler),
        )
        .route(
            "/v1/webhooks/:token/condition",
            patch(update_webhook_condition_handler),
        )
        .route(
            "/v1/webhooks/:token/rate-limit",
            patch(update_webhook_rate_limit_handler),
        )
        .route("/v1/webhooks/:token/pause", post(pause_webhook_handler))
        .route("/v1/webhooks/:token/resume", post(resume_webhook_handler))
        .route(
            "/v1/webhooks/:token/rotate-secret",
            post(rotate_webhook_secret_handler),
        )
        .route(
            "/v1/webhooks/:token/payload-transform",
            post(set_payload_transform_handler),
        )
}

#[cfg(test)]
mod tests {
    use crate::http::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn webhook_delivery_recorded_on_trigger_success() {
        use crate::webhook::{WebhookRecord, WebhookStore};
        let state = default_app_state();
        state
            .webhook_store
            .upsert(WebhookRecord {
                token: "test-delivery-token".to_string(),
                tenant_id: "tenant-1".to_string(),
                workflow_id: "workflow-1".to_string(),
                workflow_version_id: "version-1".to_string(),
                secret: None,
                condition_expr: None,
                max_calls_per_minute: None,
                paused: false,
                payload_transform_script: None,
            })
            .await
            .unwrap();
        let app = build_router(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/test-delivery-token")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"x": 1}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let deliveries = state
            .webhook_store
            .list_deliveries("test-delivery-token", 10)
            .await;
        assert_eq!(deliveries.len(), 1);
        assert!(deliveries[0].success);
        assert_eq!(deliveries[0].status_code, Some(202));
        assert!(deliveries[0].execution_id.is_some());
    }

    #[tokio::test]
    async fn webhook_delivery_recorded_on_quota_failure() {
        use crate::billing::{BillingStore, TenantQuota};
        use crate::webhook::{WebhookRecord, WebhookStore};
        let state = default_app_state();
        state
            .webhook_store
            .upsert(WebhookRecord {
                token: "test-quota-tok".to_string(),
                tenant_id: "tenant-1".to_string(),
                workflow_id: "workflow-1".to_string(),
                workflow_version_id: "version-1".to_string(),
                secret: None,
                condition_expr: None,
                max_calls_per_minute: None,
                paused: false,
                payload_transform_script: None,
            })
            .await
            .unwrap();
        state.billing_store.set_quota(TenantQuota {
            tenant_id: "tenant-1".to_string(),
            tier: "free".to_string(),
            max_executions_per_month: 0,
            max_concurrent_executions: 10,
            max_workflows: 100,
        });
        let app = build_router(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/test-quota-tok")
                    .header("content-type", "application/json")
                    .body(Body::from("{}".to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        let deliveries = state
            .webhook_store
            .list_deliveries("test-quota-tok", 10)
            .await;
        assert_eq!(deliveries.len(), 1);
        assert!(!deliveries[0].success);
        assert_eq!(deliveries[0].status_code, Some(402));
        assert!(deliveries[0].error_message.is_some());
    }

    #[tokio::test]
    async fn list_webhook_deliveries_endpoint() {
        use crate::webhook::{WebhookDelivery, WebhookRecord, WebhookStore};
        let state = default_app_state();
        state
            .webhook_store
            .upsert(WebhookRecord {
                token: "tok-list-del".to_string(),
                tenant_id: "tenant-1".to_string(),
                workflow_id: "workflow-1".to_string(),
                workflow_version_id: "version-1".to_string(),
                secret: None,
                condition_expr: None,
                max_calls_per_minute: None,
                paused: false,
                payload_transform_script: None,
            })
            .await
            .unwrap();
        // Pre-populate a delivery
        state
            .webhook_store
            .record_delivery(WebhookDelivery {
                id: "del-1".to_string(),
                webhook_token: "tok-list-del".to_string(),
                tenant_id: "tenant-1".to_string(),
                delivered_at: 1_700_000_000,
                status_code: Some(202),
                success: true,
                error_message: None,
                execution_id: Some("exec-1".to_string()),
            })
            .await;
        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/webhooks/tok-list-del/deliveries")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["success"], true);
    }

    #[tokio::test]
    async fn webhook_condition_blocks_non_matching_payload() {
        use crate::webhook::{WebhookRecord, WebhookStore};
        let state = default_app_state();
        state
            .webhook_store
            .upsert(WebhookRecord {
                token: "cond-filter-tok".to_string(),
                tenant_id: "tenant-1".to_string(),
                workflow_id: "workflow-1".to_string(),
                workflow_version_id: "version-1".to_string(),
                secret: None,
                condition_expr: Some("event == \"purchase\"".to_string()),
                max_calls_per_minute: None,
                paused: false,
                payload_transform_script: None,
            })
            .await
            .unwrap();
        let app = build_router(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/cond-filter-tok")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"event":"login"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(body["error"]
            .as_str()
            .unwrap_or("")
            .starts_with("filtered:"));
    }

    #[tokio::test]
    async fn webhook_condition_passes_matching_payload() {
        use crate::webhook::{WebhookRecord, WebhookStore};
        let state = default_app_state();
        state
            .webhook_store
            .upsert(WebhookRecord {
                token: "cond-match-tok".to_string(),
                tenant_id: "tenant-1".to_string(),
                workflow_id: "workflow-1".to_string(),
                workflow_version_id: "version-1".to_string(),
                secret: None,
                condition_expr: Some("event == \"purchase\"".to_string()),
                max_calls_per_minute: None,
                paused: false,
                payload_transform_script: None,
            })
            .await
            .unwrap();
        let app = build_router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/cond-match-tok")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"event":"purchase","amount":99.99}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(body["id"].is_string());
    }
}
