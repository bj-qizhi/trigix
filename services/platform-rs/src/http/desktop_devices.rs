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

#[derive(serde::Deserialize)]
struct DeviceListQuery {
    state: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(serde::Deserialize)]
struct UpdateDeviceRequest {
    display_name: String,
}

#[derive(serde::Deserialize)]
struct ClaimRotationRequest {
    current_credential: String,
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
        PairingError::AlreadyUsed | PairingError::DeviceConflict | PairingError::InvalidState => {
            ApiError {
                status: StatusCode::CONFLICT,
                message: error.to_string(),
            }
        }
        PairingError::AttemptsExceeded => ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: error.to_string(),
        },
        PairingError::Store(_) => ApiError::internal("desktop_pairing_store"),
    }
}

fn rotation_claim_error(error: PairingError) -> ApiError {
    match error {
        PairingError::NotFound | PairingError::InvalidClaim | PairingError::InvalidState => {
            ApiError {
                status: StatusCode::NOT_FOUND,
                message: "Device not found or Credential is invalid".to_string(),
            }
        }
        other => pairing_error(other),
    }
}

fn require_admin(claims: Option<Claims>) -> Result<Claims, ApiError> {
    let claims = claims
        .filter(Claims::is_admin)
        .ok_or_else(|| ApiError::forbidden("Tenant admin role required"))?;
    if claims.tenant_id.trim().is_empty() {
        return Err(ApiError::forbidden("Tenant context required"));
    }
    Ok(claims)
}

fn actor_id(claims: &Claims) -> Result<String, ApiError> {
    let actor = claims
        .user_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| claims.sub.clone());
    if actor.trim().is_empty() {
        return Err(ApiError::forbidden("Authenticated actor required"));
    }
    Ok(actor)
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
    let claims = require_admin(claims)?;
    let rate_key = format!("desktop-pairing-approve:{}", claims.tenant_id);
    if !state.rate_limiter.check_with_limit(&rate_key, 20) {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Pairing approval rate limit exceeded".to_string(),
        });
    }
    let actor_id = actor_id(&claims)?;
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

async fn list_devices(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<DeviceListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let claims = require_admin(claims)?;
    let state_filter = query.state.as_deref().map(str::trim);
    if state_filter.is_some_and(|value| {
        !matches!(
            value,
            "paired" | "online" | "offline" | "suspended" | "revoked"
        )
    }) {
        return Err(ApiError::bad_request("Invalid device state filter"));
    }
    let list = state
        .device_pairing_store
        .list_devices(
            &claims.tenant_id,
            state_filter,
            query.limit.unwrap_or(50).clamp(1, 100),
            query.offset.unwrap_or(0),
        )
        .await
        .map_err(pairing_error)?;
    Ok(Json(serde_json::to_value(list).unwrap_or_default()))
}

async fn get_device(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(device_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let claims = require_admin(claims)?;
    let device = state
        .device_pairing_store
        .get_device(&claims.tenant_id, &device_id)
        .await
        .map_err(pairing_error)?
        .ok_or_else(|| pairing_error(PairingError::NotFound))?;
    Ok(Json(serde_json::to_value(device).unwrap_or_default()))
}

async fn update_device(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(device_id): Path<String>,
    Json(request): Json<UpdateDeviceRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let claims = require_admin(claims)?;
    let actor_id = actor_id(&claims)?;
    let display_name = request.display_name.trim();
    if display_name.is_empty()
        || display_name.len() > 128
        || display_name.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "display_name must contain 1 to 128 safe characters",
        ));
    }
    let device = state
        .device_pairing_store
        .rename_device(&claims.tenant_id, &device_id, display_name)
        .await
        .map_err(pairing_error)?;
    state.audit_store.record(
        &claims.tenant_id,
        crate::audit::action::DEVICE_UPDATED,
        "device",
        &device_id,
        Some(serde_json::json!({"actor_id": actor_id, "display_name": display_name})),
    );
    Ok(Json(serde_json::to_value(device).unwrap_or_default()))
}

async fn transition_device(
    state: AppState,
    claims: Option<Claims>,
    device_id: String,
    target_state: &'static str,
) -> Result<Json<serde_json::Value>, ApiError> {
    let claims = require_admin(claims)?;
    let actor_id = actor_id(&claims)?;
    let device = state
        .device_pairing_store
        .set_device_state(&claims.tenant_id, &device_id, target_state)
        .await
        .map_err(pairing_error)?;
    let action = if target_state == "revoked" {
        crate::audit::action::DEVICE_REVOKED
    } else {
        crate::audit::action::DEVICE_SUSPENDED
    };
    state.audit_store.record(
        &claims.tenant_id,
        action,
        "device",
        &device_id,
        Some(serde_json::json!({"actor_id": actor_id})),
    );
    Ok(Json(serde_json::to_value(device).unwrap_or_default()))
}

async fn suspend_device(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(device_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    transition_device(state, claims, device_id, "suspended").await
}

async fn revoke_device(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(device_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    transition_device(state, claims, device_id, "revoked").await
}

async fn start_credential_rotation(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(device_id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let claims = require_admin(claims)?;
    let actor_id = actor_id(&claims)?;
    let rotation = state
        .device_pairing_store
        .start_rotation(&claims.tenant_id, &device_id)
        .await
        .map_err(pairing_error)?;
    state.audit_store.record(
        &claims.tenant_id,
        crate::audit::action::DEVICE_CREDENTIAL_ROTATION_STARTED,
        "device",
        &device_id,
        Some(serde_json::json!({"actor_id": actor_id, "rotation_id": rotation.rotation_id})),
    );
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(rotation).unwrap_or_default()),
    ))
}

async fn claim_credential_rotation(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Json(request): Json<ClaimRotationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if request.current_credential.len() != 73 || !request.current_credential.is_ascii() {
        return Err(rotation_claim_error(PairingError::InvalidClaim));
    }
    let rate_key = format!("desktop-credential-rotation:{device_id}");
    if !state.rate_limiter.check_with_limit(&rate_key, 10) {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Credential rotation rate limit exceeded".to_string(),
        });
    }
    let credential = state
        .device_pairing_store
        .claim_rotation(&device_id, &request.current_credential)
        .await
        .map_err(rotation_claim_error)?;
    state.audit_store.record(
        &credential.tenant_id,
        crate::audit::action::DEVICE_CREDENTIAL_ROTATED,
        "device",
        &credential.device_id,
        Some(serde_json::json!({"credential_id": credential.credential_id})),
    );
    Ok(Json(serde_json::to_value(credential).unwrap_or_default()))
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
        .route("/v1/desktop/devices", get(list_devices))
        .route(
            "/v1/desktop/devices/:device_id",
            get(get_device).patch(update_device),
        )
        .route(
            "/v1/desktop/devices/:device_id/suspend",
            post(suspend_device),
        )
        .route("/v1/desktop/devices/:device_id/revoke", post(revoke_device))
        .route(
            "/v1/desktop/devices/:device_id/credential-rotation",
            post(start_credential_rotation),
        )
        .route(
            "/v1/desktop/devices/:device_id/credential-rotation/claim",
            post(claim_credential_rotation),
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

    #[tokio::test]
    async fn registry_management_is_tenant_scoped_audited_and_rotates_credentials() {
        let state = default_app_state();
        let audit_store = Arc::clone(&state.audit_store);
        let app = build_router(state);
        let admin_token = crate::auth::sign_token(&Claims {
            sub: "admin-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            role: crate::auth::Role::Admin,
            user_id: Some("admin-1".to_string()),
            ..Default::default()
        })
        .unwrap();
        let other_token = crate::auth::sign_token(&Claims {
            sub: "admin-2".to_string(),
            tenant_id: "tenant-2".to_string(),
            role: crate::auth::Role::Admin,
            ..Default::default()
        })
        .unwrap();

        let create = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/pairing-sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(create_body("device-registry-http").to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = response_json(create).await;
        let approve = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/pairing-sessions/approve")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"pairing_code": created["pairing_code"]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(approve.status(), StatusCode::OK);
        let claim = app
            .clone()
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
        let original = response_json(claim).await;

        let other_tenant = app
            .clone()
            .oneshot(
                Request::get("/v1/desktop/devices/device-registry-http")
                    .header("authorization", format!("Bearer {other_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(other_tenant.status(), StatusCode::NOT_FOUND);

        let update = app
            .clone()
            .oneshot(
                Request::patch("/v1/desktop/devices/device-registry-http")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"display_name":"Operations PC"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update.status(), StatusCode::OK);
        assert_eq!(response_json(update).await["display_name"], "Operations PC");

        let start = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/devices/device-registry-http/credential-rotation")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(start.status(), StatusCode::CREATED);
        let started = response_json(start).await;
        assert!(started.get("credential").is_none());
        assert!(started["rotation_id"].is_string());

        let rotate = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/devices/device-registry-http/credential-rotation/claim")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"current_credential": original["credential"]})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rotate.status(), StatusCode::OK);
        let rotated = response_json(rotate).await;
        assert_ne!(rotated["credential"], original["credential"]);

        let replay = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/devices/device-registry-http/credential-rotation/claim")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"current_credential": original["credential"]})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::NOT_FOUND);

        let revoke = app
            .oneshot(
                Request::post("/v1/desktop/devices/device-registry-http/revoke")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(revoke.status(), StatusCode::OK);
        assert_eq!(response_json(revoke).await["state"], "revoked");

        let audit_json = serde_json::to_string(&audit_store.list("tenant-1", 20).await).unwrap();
        assert!(audit_json.contains(crate::audit::action::DEVICE_UPDATED));
        assert!(audit_json.contains(crate::audit::action::DEVICE_CREDENTIAL_ROTATED));
        assert!(audit_json.contains(crate::audit::action::DEVICE_REVOKED));
        assert!(!audit_json.contains(original["credential"].as_str().unwrap()));
        assert!(!audit_json.contains(rotated["credential"].as_str().unwrap()));
    }
}
