// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

async fn create_token(
    State(state): State<AppState>,
    Json(body): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let role: crate::auth::Role = body
        .role
        .as_deref()
        .and_then(|r| r.parse().ok())
        .unwrap_or_default();

    // First check stored API keys (takes precedence so tenant_id is enforced).
    if let Some(stored) = state.api_key_store.validate(&body.api_key).await {
        let tenant_id = stored.tenant_id.clone();
        let workspace_id = body
            .workspace_id
            .unwrap_or_else(|| "workspace-1".to_string());
        let project_id = body.project_id.unwrap_or_else(|| "project-1".to_string());
        let exp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + 7 * 24 * 3600;
        let role_str = role.as_str().to_string();
        let claims = Claims {
            sub: tenant_id.clone(),
            tenant_id: tenant_id.clone(),
            workspace_id: workspace_id.clone(),
            project_id: project_id.clone(),
            exp,
            role,
            user_id: None,
            email: None,
        };
        let token = sign_token(&claims).map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Failed to sign token".to_string(),
        })?;
        return Ok(Json(TokenResponse {
            token,
            tenant_id,
            workspace_id,
            project_id,
            role: role_str,
        }));
    }

    // Fall back to the static DEV_API_KEY.
    let expected_key = std::env::var("DEV_API_KEY").unwrap_or_else(|_| "dev".to_string());
    if body.api_key != expected_key {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid api_key".to_string(),
        });
    }
    let tenant_id = body.tenant_id.unwrap_or_else(|| "tenant-1".to_string());
    let workspace_id = body
        .workspace_id
        .unwrap_or_else(|| "workspace-1".to_string());
    let project_id = body.project_id.unwrap_or_else(|| "project-1".to_string());
    let exp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 3600;
    let role_str = role.as_str().to_string();
    let claims = Claims {
        sub: tenant_id.clone(),
        tenant_id: tenant_id.clone(),
        workspace_id: workspace_id.clone(),
        project_id: project_id.clone(),
        exp,
        role,
        user_id: None,
        email: None,
    };
    let token = sign_token(&claims).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Failed to sign token".to_string(),
    })?;
    Ok(Json(TokenResponse {
        token,
        tenant_id,
        workspace_id,
        project_id,
        role: role_str,
    }))
}

// ── User auth (register / login / me) ─────────────────────────────────────

async fn register_user(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let tenant_id = body.tenant_id.unwrap_or_else(|| "tenant-1".to_string());
    let store = &state.user_store;
    let user = tokio::task::spawn_blocking({
        let email = body.email.clone();
        let password = body.password.clone();
        let name = body.name.clone();
        let store = Arc::clone(store);
        move || store.create(&email, &password, name.as_deref(), &tenant_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .map_err(|e| match e {
        crate::users::UserError::EmailAlreadyExists => ApiError {
            status: StatusCode::CONFLICT,
            message: "Email already registered".to_string(),
        },
        other => ApiError {
            status: StatusCode::BAD_REQUEST,
            message: other.to_string(),
        },
    })?;

    // Fire verification email non-blocking
    {
        let ver_store = Arc::clone(&state.verification_store);
        let email_client = Arc::clone(&state.email_client);
        let uid = user.id.clone();
        let em = user.email.clone();
        tokio::spawn(async move {
            let ver = tokio::task::spawn_blocking(move || ver_store.create(&uid, &em, 24))
                .await
                .ok();
            if let Some(ver) = ver {
                email_client
                    .send_email_verification(&ver.email, &ver.token, ver.expires_at)
                    .await;
            }
        });
    }

    let token = make_user_token(&user)?;
    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user: crate::users::PublicUser::from(&user),
        }),
    ))
}

async fn login_user(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let store = Arc::clone(&state.user_store);
    let user = tokio::task::spawn_blocking({
        let email = body.email.clone();
        let password = body.password.clone();
        move || store.verify_password(&email, &password)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .map_err(|e| match e {
        crate::users::UserError::InvalidCredentials => ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid email or password".to_string(),
        },
        other => ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: other.to_string(),
        },
    })?;

    let token = make_user_token(&user)?;
    Ok(Json(AuthResponse {
        token,
        user: crate::users::PublicUser::from(&user),
    }))
}

// ── Enterprise SSO (OIDC) ───────────────────────────────────────────────────

async fn me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<crate::users::PublicUser>, ApiError> {
    let user_id = claims
        .as_ref()
        .and_then(|c| c.user_id.as_deref())
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "Not authenticated as a user".to_string(),
        })?;

    let store = Arc::clone(&state.user_store);
    let user = tokio::task::spawn_blocking(move || store.find_by_id(&user_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "User not found".to_string(),
        })?;

    Ok(Json(crate::users::PublicUser::from(&user)))
}

async fn update_me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<UpdateMeBody>,
) -> Result<Json<crate::users::PublicUser>, ApiError> {
    let user_id = require_user_id(&claims)?;

    if body.new_password.is_some() && body.current_password.is_none() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "current_password required to change password".to_string(),
        });
    }

    if let (Some(old_pw), Some(new_pw)) = (
        body.current_password.as_deref(),
        body.new_password.as_deref(),
    ) {
        let store = Arc::clone(&state.user_store);
        let uid = user_id.clone();
        let old_pw = old_pw.to_string();
        let new_pw = new_pw.to_string();
        tokio::task::spawn_blocking(move || store.update_password(&uid, &old_pw, &new_pw))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?
            .map_err(|e| match e {
                crate::users::UserError::InvalidCredentials => ApiError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "Current password is incorrect".to_string(),
                },
                other => ApiError {
                    status: StatusCode::BAD_REQUEST,
                    message: other.to_string(),
                },
            })?;
    }

    if let Some(name) = body.name.as_deref() {
        let store = Arc::clone(&state.user_store);
        let uid = user_id.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || store.update_name(&uid, &name))
            .await
            .map_err(|_| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Task join error".to_string(),
            })?
            .map_err(|e| ApiError {
                status: StatusCode::BAD_REQUEST,
                message: e.to_string(),
            })?;
    }

    let store = Arc::clone(&state.user_store);
    let uid = user_id.clone();
    let user = tokio::task::spawn_blocking(move || store.find_by_id(&uid))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "User not found".to_string(),
        })?;

    Ok(Json(crate::users::PublicUser::from(&user)))
}

async fn get_notifications_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<crate::notification_prefs::NotificationPrefs>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let prefs_store = Arc::clone(&state.notification_prefs_store);
    let prefs = tokio::task::spawn_blocking(move || prefs_store.get(&user_id))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;
    Ok(Json(prefs))
}

async fn put_notifications_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(body): Json<UpdateNotificationsBody>,
) -> Result<Json<crate::notification_prefs::NotificationPrefs>, ApiError> {
    let user_id = require_user_id(&claims)?;
    let prefs = crate::notification_prefs::NotificationPrefs {
        user_id: user_id.clone(),
        email_on_failure: body.email_on_failure,
        email_on_success: body.email_on_success,
    };
    let prefs_store = Arc::clone(&state.notification_prefs_store);
    let prefs_clone = prefs.clone();
    tokio::task::spawn_blocking(move || prefs_store.upsert(prefs_clone))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;
    Ok(Json(prefs))
}

// ── Admin: user management ────────────────────────────────────────────────

async fn accept_invite_handler(
    State(state): State<AppState>,
    Json(body): Json<AcceptInviteBody>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    // Validate and consume the invite
    let invite_store = Arc::clone(&state.invite_store);
    let token = body.token.clone();
    let inv = tokio::task::spawn_blocking(move || invite_store.mark_used(&token))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Task join error".to_string(),
        })?
        .map_err(|e| match e {
            crate::invitations::InviteError::NotFound => ApiError {
                status: StatusCode::NOT_FOUND,
                message: "Invitation not found".to_string(),
            },
            crate::invitations::InviteError::AlreadyUsed => ApiError {
                status: StatusCode::GONE,
                message: "Invitation already used".to_string(),
            },
            crate::invitations::InviteError::Expired => ApiError {
                status: StatusCode::GONE,
                message: "Invitation has expired".to_string(),
            },
        })?;

    // Register the user with the invited email + tenant
    let user_store = Arc::clone(&state.user_store);
    let email = inv.email.clone();
    let password = body.password.clone();
    let name = body.name.clone();
    let tenant_id = inv.tenant_id.clone();
    let user = tokio::task::spawn_blocking(move || {
        user_store.create(&email, &password, name.as_deref(), &tenant_id)
    })
    .await
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Task join error".to_string(),
    })?
    .map_err(|e| match e {
        crate::users::UserError::EmailAlreadyExists => ApiError {
            status: StatusCode::CONFLICT,
            message: "Email already registered".to_string(),
        },
        other => ApiError {
            status: StatusCode::BAD_REQUEST,
            message: other.to_string(),
        },
    })?;

    let token = make_user_token(&user)?;
    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user: crate::users::PublicUser::from(&user),
        }),
    ))
}

// ── Password reset ─────────────────────────────────────────────────────────

async fn forgot_password_handler(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordBody>,
) -> Result<Json<ForgotPasswordResponse>, ApiError> {
    let email = body.email.trim().to_lowercase();
    let user_store = Arc::clone(&state.user_store);
    let em = email.clone();
    let user = tokio::task::spawn_blocking(move || user_store.find_by_email(&em))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;

    // Always return 200 to avoid email enumeration
    let (token_val, expires_at) = match user {
        Some(u) => {
            let reset_store = Arc::clone(&state.reset_store);
            let uid = u.id.clone();
            let em2 = email.clone();
            let reset = tokio::task::spawn_blocking(move || reset_store.create(&uid, &em2, 2))
                .await
                .map_err(|_| ApiError::internal("Task join error"))?;
            let tok = reset.token.clone();
            let exp = reset.expires_at;
            state
                .email_client
                .send_password_reset(&email, &tok, exp)
                .await;
            (Some(tok), exp)
        }
        None => {
            let exp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
                + 7200;
            (None, exp)
        }
    };

    // In prod/auth-required mode suppress the token from the response
    let expose_token = !auth_required();
    Ok(Json(ForgotPasswordResponse {
        message: "If an account exists with that email, a reset link has been sent.".to_string(),
        token: if expose_token { token_val } else { None },
        expires_at,
    }))
}

async fn reset_password_handler(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.new_password.len() < 6 {
        return Err(ApiError::bad_request(
            "Password must be at least 6 characters",
        ));
    }
    let reset_store = Arc::clone(&state.reset_store);
    let token = body.token.clone();
    let reset = tokio::task::spawn_blocking(move || reset_store.mark_used(&token))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|e| match e {
            crate::password_reset::ResetError::NotFound => {
                ApiError::not_found("Reset token not found or already used")
            }
            crate::password_reset::ResetError::AlreadyUsed => ApiError {
                status: StatusCode::GONE,
                message: "Reset token already used".to_string(),
            },
            crate::password_reset::ResetError::Expired => ApiError {
                status: StatusCode::GONE,
                message: "Reset token has expired".to_string(),
            },
            crate::password_reset::ResetError::StoreUnavailable => {
                ApiError::internal("Store unavailable")
            }
        })?;

    let user_store = Arc::clone(&state.user_store);
    let uid = reset.user_id.clone();
    let new_pw = body.new_password.clone();
    tokio::task::spawn_blocking(move || user_store.reset_password(&uid, &new_pw))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|_| ApiError::internal("Failed to update password"))?;

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "Password updated successfully" }),
    ))
}

async fn verify_email_handler(
    State(state): State<AppState>,
    Json(body): Json<VerifyEmailBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    use crate::email_verification::VerificationError;
    let ver_store = Arc::clone(&state.verification_store);
    let token = body.token.clone();
    let ver = tokio::task::spawn_blocking(move || ver_store.mark_used(&token))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|e| match e {
            VerificationError::NotFound => {
                ApiError::not_found("Verification token not found or already used")
            }
            VerificationError::AlreadyUsed => ApiError {
                status: StatusCode::GONE,
                message: "Verification token already used".to_string(),
            },
            VerificationError::Expired => ApiError {
                status: StatusCode::GONE,
                message: "Verification token has expired".to_string(),
            },
            VerificationError::StoreUnavailable => ApiError::internal("Store unavailable"),
        })?;

    let user_store = Arc::clone(&state.user_store);
    let uid = ver.user_id.clone();
    tokio::task::spawn_blocking(move || user_store.mark_email_verified(&uid))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?
        .map_err(|_| ApiError::internal("Failed to mark email verified"))?;

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "Email verified successfully" }),
    ))
}

async fn resend_verification_handler(
    State(state): State<AppState>,
    Json(body): Json<ResendVerificationBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = body.email.trim().to_lowercase();
    let user_store = Arc::clone(&state.user_store);
    let em = email.clone();
    let user = tokio::task::spawn_blocking(move || user_store.find_by_email(&em))
        .await
        .map_err(|_| ApiError::internal("Task join error"))?;

    // Always 200 to avoid email enumeration
    if let Some(u) = user {
        if !u.email_verified {
            let ver_store = Arc::clone(&state.verification_store);
            let email_client = Arc::clone(&state.email_client);
            let uid = u.id.clone();
            let em2 = email.clone();
            tokio::spawn(async move {
                let ver = tokio::task::spawn_blocking(move || ver_store.create(&uid, &em2, 24))
                    .await
                    .ok();
                if let Some(ver) = ver {
                    email_client
                        .send_email_verification(&ver.email, &ver.token, ver.expires_at)
                        .await;
                }
            });
        }
    }

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "If an unverified account exists with that email, a verification link has been sent." }),
    ))
}

// ── Organization management ────────────────────────────────────────────────

async fn list_api_keys_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<ApiKeyQuery>,
) -> Json<Vec<crate::api_keys::ApiKeyRecord>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    Json(state.api_key_store.list(&tenant_id).await)
}

async fn create_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CreateApiKeyBody>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), ApiError> {
    require_admin(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    let raw_key = crate::api_keys::generate_api_key();
    let record = state
        .api_key_store
        .create(&body.tenant_id, &body.name, &raw_key)
        .await;
    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            record,
            key: raw_key,
        }),
    ))
}

async fn delete_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(key_id): Path<String>,
    Query(query): Query<ApiKeyQuery>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    if state.api_key_store.delete(&tenant_id, &key_id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/api-keys",
            get(list_api_keys_handler).post(create_api_key_handler),
        )
        .route(
            "/v1/api-keys/:key_id",
            get(method_not_allowed).delete(delete_api_key_handler),
        )
        .route("/v1/auth/token", get(method_not_allowed).post(create_token))
        .route(
            "/v1/auth/register",
            get(method_not_allowed).post(register_user),
        )
        .route("/v1/auth/login", get(method_not_allowed).post(login_user))
        .route("/v1/auth/me", get(me_handler).patch(update_me_handler))
        .route(
            "/v1/auth/me/notifications",
            get(get_notifications_handler).put(put_notifications_handler),
        )
        .route("/v1/auth/accept-invite", post(accept_invite_handler))
        .route("/v1/auth/forgot-password", post(forgot_password_handler))
        .route("/v1/auth/reset-password", post(reset_password_handler))
        .route("/v1/auth/verify-email", post(verify_email_handler))
        .route(
            "/v1/auth/resend-verification",
            post(resend_verification_handler),
        )
}
