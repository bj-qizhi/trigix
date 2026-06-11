// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

// Public: look up an invite by token (used by frontend before showing accept form)
async fn get_invitation_handler(
    State(state): State<AppState>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = Arc::clone(&state.invite_store);
    let inv = tokio::task::spawn_blocking(move || store.find_by_token(&token))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Invitation not found".to_string(),
        })?;
    if !inv.is_valid() {
        return Err(ApiError {
            status: StatusCode::GONE,
            message: "Invitation has expired or already been used".to_string(),
        });
    }
    Ok(Json(
        serde_json::json!({ "email": inv.email, "role": inv.role, "valid": true }),
    ))
}

async fn list_orgs_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::orgs::OrgRecord>>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let orgs = tokio::task::spawn_blocking(move || store.list_for_user(&user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(orgs))
}

async fn create_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<CreateOrgBody>,
) -> Result<(StatusCode, Json<crate::orgs::OrgRecord>), ApiError> {
    let user_id = require_user_id(&claims)?;
    let org_id = uuid::Uuid::new_v4().to_string();
    let store = Arc::clone(&state.org_store);
    let org = tokio::task::spawn_blocking(move || store.create(&org_id, &body.name, &user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;
    Ok((StatusCode::CREATED, Json(org)))
}

async fn get_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<Json<crate::orgs::OrgRecord>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();
    let member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        let user_id = user_id.clone();
        move || store.get_member(&org_id2, &user_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    if member.is_none() {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Not a member of this organization".to_string(),
        });
    }

    let org = tokio::task::spawn_blocking(move || store.find_by_id(&org_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Organization not found".to_string(),
        })?;
    Ok(Json(org))
}

async fn delete_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();

    // Only the org owner or an admin member can delete
    let org = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.find_by_id(&org_id2)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "Organization not found".to_string(),
    })?;

    if org.owner_id != user_id {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Only the owner can delete an organization".to_string(),
        });
    }

    tokio::task::spawn_blocking(move || store.delete(&org_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_org_members_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<crate::orgs::OrgMember>>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();

    // Must be a member to list members
    let member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &user_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    if member.is_none() {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Not a member of this organization".to_string(),
        });
    }

    let members = tokio::task::spawn_blocking(move || store.list_members(&org_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?;
    Ok(Json(members))
}

async fn add_org_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
    Json(body): Json<AddMemberBody>,
) -> Result<(StatusCode, Json<crate::orgs::OrgMember>), ApiError> {
    let caller_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();

    // Must be an admin member to add members
    let caller_member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &caller_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    match caller_member {
        Some(m) if m.role == "admin" => {}
        _ => {
            return Err(ApiError {
                status: StatusCode::FORBIDDEN,
                message: "Only admin members can add members".to_string(),
            })
        }
    }

    let member =
        tokio::task::spawn_blocking(move || store.add_member(&org_id, &body.user_id, &body.role))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?
            .map_err(|e| match e {
                crate::orgs::OrgError::AlreadyMember => ApiError {
                    status: StatusCode::CONFLICT,
                    message: "User is already a member".to_string(),
                },
                other => ApiError {
                    status: StatusCode::BAD_REQUEST,
                    message: other.to_string(),
                },
            })?;
    Ok((StatusCode::CREATED, Json(member)))
}

async fn remove_org_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path((org_id, target_user_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let caller_id = require_user_id(&claims)?;
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();
    let caller_id2 = caller_id.clone();

    let caller_member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &caller_id2)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?;

    // Admins can remove anyone; members can only remove themselves
    let is_admin = caller_member.map(|m| m.role == "admin").unwrap_or(false);
    let is_self = caller_id == target_user_id;
    if !is_admin && !is_self {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "Not authorized to remove this member".to_string(),
        });
    }

    tokio::task::spawn_blocking(move || store.remove_member(&org_id, &target_user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Member not found".to_string(),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

/// Issues a new JWT scoped to the given org's tenant_id with the caller's membership role.
async fn switch_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(org_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let user_email = claims.as_ref().and_then(|c| c.email.clone());
    let store = Arc::clone(&state.org_store);
    let org_id2 = org_id.clone();
    let user_id2 = user_id.clone();

    let member = tokio::task::spawn_blocking({
        let store = Arc::clone(&store);
        move || store.get_member(&org_id2, &user_id2)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::FORBIDDEN,
        message: "Not a member of this organization".to_string(),
    })?;

    let role: crate::auth::Role = member.role.parse().unwrap_or_default();
    let exp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 3600;
    let new_claims = Claims {
        sub: user_id.clone(),
        tenant_id: org_id.clone(),
        workspace_id: "workspace-1".to_string(),
        project_id: "project-1".to_string(),
        exp,
        role: role.clone(),
        user_id: Some(user_id),
        email: user_email,
    };
    let token = sign_token(&new_claims).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Failed to sign token".to_string(),
    })?;
    Ok(Json(serde_json::json!({
        "token": token,
        "org_id": org_id,
        "tenant_id": org_id,
        "role": role.as_str(),
    })))
}

// ── API Key management ─────────────────────────────────────────────────────

async fn list_notifications_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<NotifQuery>,
) -> Json<serde_json::Value> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));
    let user_id = claims.as_ref().and_then(|c| c.user_id.as_deref());
    let items = state
        .notification_store
        .list(&tenant_id, user_id, query.limit);
    let unread = state.notification_store.unread_count(&tenant_id, user_id);
    Json(serde_json::json!({ "notifications": items, "unread_count": unread }))
}

async fn mark_notification_read_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(notif_id): Path<String>,
    Query(q): Query<TenantQuery>,
) -> impl IntoResponse {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let found = state.notification_store.mark_read(&notif_id, &tenant_id);
    if found {
        (StatusCode::OK, Json(serde_json::json!({"ok": true})))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
    }
}

async fn mark_all_notifications_read_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(q): Query<TenantQuery>,
) -> Json<serde_json::Value> {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let user_id = claims.as_ref().and_then(|c| c.user_id.as_deref());
    state.notification_store.mark_all_read(&tenant_id, user_id);
    Json(serde_json::json!({"ok": true}))
}

async fn delete_notification_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(notif_id): Path<String>,
    Query(q): Query<TenantQuery>,
) -> impl IntoResponse {
    let tenant_id = effective_tenant_id(&claims, &q.tenant_id);
    let found = state.notification_store.delete(&notif_id, &tenant_id);
    if found {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response()
    }
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/invitations/:token", get(get_invitation_handler))
        .route("/v1/orgs", get(list_orgs_handler).post(create_org_handler))
        .route(
            "/v1/orgs/:org_id",
            get(get_org_handler).delete(delete_org_handler),
        )
        .route(
            "/v1/orgs/:org_id/members",
            get(list_org_members_handler).post(add_org_member_handler),
        )
        .route(
            "/v1/orgs/:org_id/members/:user_id",
            delete(remove_org_member_handler),
        )
        .route("/v1/orgs/:org_id/switch", post(switch_org_handler))
        .route("/v1/notifications", get(list_notifications_handler))
        .route(
            "/v1/notifications/read-all",
            post(mark_all_notifications_read_handler),
        )
        .route(
            "/v1/notifications/:notif_id",
            delete(delete_notification_handler),
        )
        .route(
            "/v1/notifications/:notif_id/read",
            post(mark_notification_read_handler),
        )
}
