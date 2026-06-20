// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Workflow event-subscription handlers.

use super::*;

async fn list_event_subscriptions_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    let subs = state
        .subscription_store
        .list(&tenant_id)
        .await
        .map_err(|_| ApiError::internal("subscription_store"))?;
    Ok(Json(
        subs.into_iter()
            .map(|s| serde_json::to_value(&s).unwrap_or_default())
            .collect(),
    ))
}

async fn create_event_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut req): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    req.tenant_id = effective_tenant_id(&claims, &req.tenant_id);
    let sub = state
        .subscription_store
        .create(req)
        .await
        .map_err(|e| match e {
            SubscriptionError::InvalidUrl => {
                ApiError::bad_request("url must start with http or https")
            }
            _ => ApiError::internal("subscription_store"),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&sub).unwrap_or_default()),
    ))
}

async fn delete_event_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(sub_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    require_write(&claims)?;
    let tenant_id = q.get("tenant_id").map(|s| s.as_str()).unwrap_or("");
    let tenant_id = effective_tenant_id(&claims, tenant_id);
    state
        .subscription_store
        .delete(&tenant_id, &sub_id)
        .await
        .map_err(|e| match e {
            SubscriptionError::NotFound => ApiError::not_found("event_subscription"),
            _ => ApiError::internal("subscription_store"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Workflow Comments ──────────────────────────────────────────────────────

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/event-subscriptions",
            get(list_event_subscriptions_handler).post(create_event_subscription_handler),
        )
        .route(
            "/v1/event-subscriptions/:sub_id",
            delete(delete_event_subscription_handler),
        )
}
