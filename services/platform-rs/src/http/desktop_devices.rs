// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;
use crate::device_pairing::{CreatePairingSessionRequest, PairingError};

#[derive(serde::Deserialize)]
struct ApprovePairingRequest {
    pairing_code: String,
}

#[derive(serde::Deserialize)]
struct ClaimPairingRequest {
    claim_secret: String,
}

fn pairing_error(error: PairingError) -> ApiError {
    match error {
        PairingError::InvalidRequest(message) => ApiError::bad_request(&message),
        PairingError::NotFound | PairingError::InvalidClaim => ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Pairing session not found or claim is invalid".to_string(),
        },
        PairingError::Expired => ApiError {
            status: StatusCode::GONE,
            message: error.to_string(),
        },
        PairingError::AlreadyUsed | PairingError::DeviceConflict => ApiError {
            status: StatusCode::CONFLICT,
            message: error.to_string(),
        },
        PairingError::AttemptsExceeded => ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: error.to_string(),
        },
        PairingError::Store(_) => ApiError::internal("desktop_pairing_store"),
    }
}

async fn create_pairing_session(
    State(state): State<AppState>,
    Json(request): Json<CreatePairingSessionRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    if !state
        .rate_limiter
        .check_with_limit("desktop-pairing-create:anonymous", 60)
    {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Pairing session creation rate limit exceeded".to_string(),
        });
    }
    let rate_key = format!("desktop-pairing-create:{}", request.device.device_id);
    if !state.rate_limiter.check_with_limit(&rate_key, 10) {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Pairing session creation rate limit exceeded".to_string(),
        });
    }
    let created = state
        .device_pairing_store
        .create_session(request)
        .await
        .map_err(pairing_error)?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(created).unwrap_or_default()),
    ))
}

async fn approve_pairing_session(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<ApprovePairingRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let claims = claims
        .filter(Claims::is_admin)
        .ok_or_else(|| ApiError::forbidden("Tenant admin role required"))?;
    if claims.tenant_id.trim().is_empty() {
        return Err(ApiError::forbidden("Tenant context required"));
    }
    let rate_key = format!("desktop-pairing-approve:{}", claims.tenant_id);
    if !state.rate_limiter.check_with_limit(&rate_key, 20) {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Pairing approval rate limit exceeded".to_string(),
        });
    }
    let actor_id = claims
        .user_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| claims.sub.clone());
    if actor_id.trim().is_empty() {
        return Err(ApiError::forbidden("Authenticated actor required"));
    }
    let code = request.pairing_code.trim().to_uppercase();
    if code.len() != 8 || !code.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "pairing_code must be 8 hexadecimal characters",
        ));
    }
    let result = state
        .device_pairing_store
        .approve(&code, &claims.tenant_id, &actor_id)
        .await;
    match result {
        Ok(device) => {
            state.audit_store.record(
                &claims.tenant_id,
                crate::audit::action::DEVICE_PAIRED,
                "device",
                &device.id,
                Some(serde_json::json!({
                    "actor_id": actor_id,
                    "operating_system": device.operating_system,
                    "agent_version": device.agent_version,
                })),
            );
            Ok(Json(serde_json::to_value(device).unwrap_or_default()))
        }
        Err(error) => {
            state.audit_store.record(
                &claims.tenant_id,
                crate::audit::action::DEVICE_PAIRING_REJECTED,
                "pairing_session",
                "redacted",
                Some(serde_json::json!({
                    "actor_id": actor_id,
                    "reason": error.code(),
                })),
            );
            Err(pairing_error(error))
        }
    }
}

async fn claim_pairing_credential(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<ClaimPairingRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if uuid::Uuid::parse_str(&session_id).is_err()
        || request.claim_secret.len() != 71
        || !request.claim_secret.is_ascii()
    {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Pairing session not found or claim is invalid".to_string(),
        });
    }
    let rate_key = format!("desktop-pairing-claim:{session_id}");
    if !state.rate_limiter.check_with_limit(&rate_key, 10) {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Pairing claim rate limit exceeded".to_string(),
        });
    }
    let credential = state
        .device_pairing_store
        .claim(&session_id, &request.claim_secret)
        .await
        .map_err(pairing_error)?;
    state.audit_store.record(
        &credential.tenant_id,
        crate::audit::action::DEVICE_CREDENTIAL_CLAIMED,
        "device",
        &credential.device_id,
        Some(serde_json::json!({
            "credential_id": credential.credential_id,
        })),
    );
    Ok(Json(serde_json::to_value(credential).unwrap_or_default()))
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/desktop/pairing-sessions", post(create_pairing_session))
        .route(
            "/v1/desktop/pairing-sessions/approve",
            post(approve_pairing_session),
        )
        .route(
            "/v1/desktop/pairing-sessions/:session_id/claim",
            post(claim_pairing_credential),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use desktop_protocol::{DeviceCapability, DeviceDescriptor};
    use tower::ServiceExt;

    fn create_body(device_id: &str) -> serde_json::Value {
        serde_json::json!({
            "device": DeviceDescriptor {
                device_id: device_id.to_string(),
                display_name: "Windows Test Device".to_string(),
                operating_system: "windows".to_string(),
                agent_version: "1.0.0".to_string(),
                capabilities: vec![DeviceCapability::SystemInformation],
            },
            "device_public_key": "A".repeat(64),
        })
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn complete_pairing_flow_keeps_credential_out_of_admin_response() {
        let state = default_app_state();
        let audit_store = Arc::clone(&state.audit_store);
        let app = build_router(state);
        let create = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/pairing-sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(create_body("device-http-1").to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        let created = response_json(create).await;

        let token = crate::auth::sign_token(&Claims {
            sub: "admin-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            role: crate::auth::Role::Admin,
            user_id: Some("admin-1".to_string()),
            ..Default::default()
        })
        .unwrap();
        let approve = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/pairing-sessions/approve")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"pairing_code": created["pairing_code"]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(approve.status(), StatusCode::OK);
        let approved = response_json(approve).await;
        assert!(approved.get("credential").is_none());
        assert_eq!(approved["tenant_id"], "tenant-1");

        let claim = app
            .oneshot(
                Request::post(format!(
                    "/v1/desktop/pairing-sessions/{}/claim",
                    created["session_id"].as_str().unwrap()
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"claim_secret": created["claim_secret"]}).to_string(),
                ))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claim.status(), StatusCode::OK);
        let claimed = response_json(claim).await;
        assert!(claimed["credential"]
            .as_str()
            .unwrap()
            .starts_with("desktop_"));

        let audit_json = serde_json::to_string(&audit_store.list("tenant-1", 10).await).unwrap();
        for secret in [
            created["pairing_code"].as_str().unwrap(),
            created["claim_secret"].as_str().unwrap(),
            claimed["credential"].as_str().unwrap(),
        ] {
            assert!(!audit_json.contains(secret));
        }
    }

    #[tokio::test]
    async fn approval_requires_tenant_admin_even_when_global_auth_is_optional() {
        let app = build_router(default_app_state());
        let response = app
            .oneshot(
                Request::post("/v1/desktop/pairing-sessions/approve")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"pairing_code":"0123ABCD"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn editor_cannot_approve_pairing() {
        let app = build_router(default_app_state());
        let token = crate::auth::sign_token(&Claims {
            sub: "editor-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            role: crate::auth::Role::Editor,
            ..Default::default()
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::post("/v1/desktop/pairing-sessions/approve")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"pairing_code":"0123ABCD"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
