// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

/// GET /v1/admin/dlq — list recent dead-letter entries (admin only).
async fn dlq_list_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<DlqListResponse>, ApiError> {
    require_admin(&claims)?;
    let dead_stream = crate::cache::keys::exec_queue_dead_stream();
    let depth = state.cache.xlen(dead_stream).await;
    let raw = state.cache.xrange_last(dead_stream, 100).await;
    let entries = raw
        .into_iter()
        .map(|(id, fields)| {
            let get = |k: &str| {
                fields
                    .iter()
                    .find(|(fk, _)| fk == k)
                    .map(|(_, v)| v.clone())
            };
            DlqEntry {
                id,
                error: get("error"),
                failed_at: get("failed_at"),
                original_msg_id: get("original_msg_id"),
                worker_id: get("worker_id"),
                job: get("job"),
            }
        })
        .collect();
    Ok(Json(DlqListResponse { depth, entries }))
}

/// POST /v1/admin/dlq/requeue — re-drive all dead-letter jobs back onto the main
/// execution queue, then remove them from the dead-letter stream (admin only).
/// Note: re-running a job re-executes the whole workflow (at-least-once); only
/// re-drive when side effects are safe to repeat.
async fn dlq_requeue_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<DlqRequeueResponse>, ApiError> {
    require_admin(&claims)?;
    let dead_stream = crate::cache::keys::exec_queue_dead_stream();
    let main_stream = crate::cache::keys::exec_queue_stream();
    let entries = state.cache.xrange_last(dead_stream, 1000).await;

    let mut requeued = 0usize;
    let mut delete_ids = Vec::new();
    for (id, fields) in entries {
        if let Some((_, job)) = fields.iter().find(|(k, _)| k == "job") {
            if state
                .cache
                .xadd(main_stream, &[("job", job)])
                .await
                .is_some()
            {
                requeued += 1;
            }
        }
        delete_ids.push(id);
    }
    state.cache.xdel(dead_stream, &delete_ids).await;
    Ok(Json(DlqRequeueResponse { requeued }))
}

// ── Billing helpers ───────────────────────────────────────────────────────────

async fn admin_set_quota_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(tenant_id): Path<String>,
    Json(body): Json<SetQuotaBody>,
) -> Result<Json<TenantQuota>, ApiError> {
    require_write(&claims)?;
    let quota = match body.tier.as_str() {
        "free" => TenantQuota::free(&tenant_id),
        "pro" => TenantQuota::pro(&tenant_id),
        "business" => TenantQuota::business(&tenant_id),
        "enterprise" => TenantQuota::unlimited(&tenant_id),
        other => {
            return Err(ApiError::bad_request(&format!(
                "Unknown tier: {other}. Valid: free, pro, business, enterprise"
            )))
        }
    };
    state.billing_store.set_quota(quota.clone());
    state.audit_store.record(
        &tenant_id,
        "billing.quota.updated",
        "tenant",
        &tenant_id,
        None,
    );
    Ok(Json(quota))
}

// ── MCP (Model Context Protocol) ──────────────────────────────────────────────

async fn admin_list_users_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::users::PublicUser>>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = claims
        .as_ref()
        .map(|c| c.tenant_id.as_str())
        .unwrap_or("tenant-1")
        .to_string();
    let store = Arc::clone(&state.user_store);
    let users = tokio::task::spawn_blocking(move || store.list_by_tenant(&tenant_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(users))
}

async fn admin_delete_user_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let caller_id = require_user_id(&claims)?;
    if user_id == caller_id {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Cannot delete your own account".to_string(),
        });
    }
    let store = Arc::clone(&state.user_store);
    tokio::task::spawn_blocking(move || store.delete_user(&user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|e| match e {
            crate::users::UserError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "User not found".to_string(),
            },
            other => ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: other.to_string(),
            },
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Admin: invitations ────────────────────────────────────────────────────

async fn admin_create_invitation_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CreateInvitationBody>,
) -> Result<(StatusCode, Json<crate::invitations::Invitation>), ApiError> {
    require_admin(&claims)?;
    let tenant_id = claims
        .as_ref()
        .map(|c| c.tenant_id.as_str())
        .unwrap_or("tenant-1")
        .to_string();
    let store = Arc::clone(&state.invite_store);
    let email = body.email.clone();
    let role = body.role.clone();
    let expires_hours = body.expires_hours;
    let inv =
        tokio::task::spawn_blocking(move || store.create(&email, &role, &tenant_id, expires_hours))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?;
    // Send invitation email (non-blocking, best-effort)
    let email_client = Arc::clone(&state.email_client);
    let inv_token = inv.token.clone();
    let inv_email = inv.email.clone();
    let inv_role = inv.role.clone();
    let inv_expires = inv.expires_at;
    tokio::spawn(async move {
        email_client
            .send_invitation(&inv_email, &inv_token, &inv_role, inv_expires)
            .await;
    });
    Ok((StatusCode::CREATED, Json(inv)))
}

async fn admin_list_invitations_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::invitations::Invitation>>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = claims
        .as_ref()
        .map(|c| c.tenant_id.as_str())
        .unwrap_or("tenant-1")
        .to_string();
    let store = Arc::clone(&state.invite_store);
    let list = tokio::task::spawn_blocking(move || store.list_by_tenant(&tenant_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(list))
}

async fn admin_delete_invitation_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    axum::extract::Path(invite_id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let store = Arc::clone(&state.invite_store);
    tokio::task::spawn_blocking(move || store.delete(&invite_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Invitation not found".to_string(),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/admin/dlq", get(dlq_list_handler))
        .route("/v1/admin/dlq/requeue", post(dlq_requeue_handler))
        .route("/v1/admin/users", get(admin_list_users_handler))
        .route(
            "/v1/admin/users/:user_id",
            delete(admin_delete_user_handler),
        )
        .route(
            "/v1/admin/invitations",
            get(admin_list_invitations_handler).post(admin_create_invitation_handler),
        )
        .route(
            "/v1/admin/invitations/:invite_id",
            delete(admin_delete_invitation_handler),
        )
        .route(
            "/v1/admin/billing/:tenant_id/quota",
            put(admin_set_quota_handler),
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
    async fn invite_flow_create_and_accept() {
        let app = router();

        // Create an invitation (admin; no AUTH_REQUIRED so passes through)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/admin/invitations")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "invited@example.com", "role": "editor"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let token = inv["token"].as_str().unwrap().to_string();

        // Look up the invite
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/invitations/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let info: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(info["email"], "invited@example.com");
        assert_eq!(info["valid"], true);

        // Accept the invite
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/accept-invite")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"token": token, "password": "securepass", "name": "Invited User"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let auth: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(auth["user"]["email"], "invited@example.com");
        assert!(auth["token"]
            .as_str()
            .map(|t| t.len() > 10)
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn invite_cannot_be_used_twice() {
        let app = router();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/admin/invitations")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "twice@example.com"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let token = inv["token"].as_str().unwrap().to_string();

        // Use it once
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/accept-invite")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"token": &token, "password": "pw1"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Second attempt — should return GONE
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/accept-invite")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"token": &token, "password": "pw2"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn invite_list_and_revoke() {
        let app = router();

        // Create an invite
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/admin/invitations")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "revoke_me@example.com"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let invite_id = inv["id"].as_str().unwrap().to_string();

        // List — should contain the invite
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/admin/invitations")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(list
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["id"] == invite_id));

        // Revoke it
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/admin/invitations/{invite_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    // ── Org management HTTP tests ───────────────────────────────────────────
}
