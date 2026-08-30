// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;
use crate::desktop_commands::DesktopCommandError;
use crate::desktop_evidence::{
    prepare_evidence, EvidenceError, EvidenceRecord, EvidenceUploadRequest, SelectorStrategy,
};
use crate::device_connection::DeviceEvent;
use crate::device_pairing::{CreatePairingSessionRequest, PairingError};
use axum::extract::DefaultBodyLimit;
use desktop_protocol::{
    DesktopAction, DesktopCommand, DesktopCommandAcknowledgement, DesktopCommandCancellation,
    DesktopCommandResult, DeviceConnectionAccepted, Envelope, ExecutionLease, Heartbeat,
    HeartbeatAccepted,
};

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

#[derive(serde::Deserialize)]
struct DispatchDesktopCommandRequest {
    tenant_id: String,
    project_id: String,
    execution_id: String,
    device_id: String,
    action: DesktopAction,
    #[serde(default = "default_command_lease_seconds")]
    lease_seconds: u64,
}

fn default_command_lease_seconds() -> u64 {
    60
}

const HEARTBEAT_INTERVAL_SECONDS: u32 = 30;

fn command_event_data(command: &DesktopCommand) -> String {
    serde_json::to_string(&Envelope::new(
        format!("delivery-{}", command.command_id),
        unix_millis(),
        command.clone(),
    ))
    .unwrap_or_default()
}

fn device_auth(headers: &axum::http::HeaderMap) -> Result<(String, String), ApiError> {
    let secure = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .next()
                .is_some_and(|proto| proto.trim() == "https")
        })
        || headers
            .get("forwarded")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().contains("proto=https"));
    if !secure {
        return Err(ApiError {
            status: StatusCode::UPGRADE_REQUIRED,
            message: "Device connections require TLS".to_string(),
        });
    }
    let device_id = headers
        .get("x-device-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty() && value.len() <= 128)
        .ok_or_else(|| rotation_claim_error(PairingError::InvalidClaim))?;
    let credential = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Device "))
        .filter(|value| value.len() == 73 && value.is_ascii())
        .ok_or_else(|| rotation_claim_error(PairingError::InvalidClaim))?;
    Ok((device_id.to_string(), credential.to_string()))
}

fn command_error(error: DesktopCommandError) -> ApiError {
    match error {
        DesktopCommandError::NotFound => ApiError::not_found("Desktop command not found"),
        DesktopCommandError::Conflict => ApiError {
            status: StatusCode::CONFLICT,
            message: "Desktop command state conflict".to_string(),
        },
        DesktopCommandError::Expired => ApiError {
            status: StatusCode::GONE,
            message: "Desktop command lease expired".to_string(),
        },
        DesktopCommandError::Store(_) => ApiError::internal("desktop_command_store"),
    }
}

async fn authenticated_device_session(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<(crate::device_pairing::PairedDevice, String), ApiError> {
    let (device_id, credential) = device_auth(headers)?;
    let device = state
        .device_pairing_store
        .authenticate_device(&device_id, &credential)
        .await
        .map_err(rotation_claim_error)?;
    let session_id = headers
        .get("x-device-session-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
        .ok_or_else(|| rotation_claim_error(PairingError::InvalidClaim))?;
    if !state
        .device_pairing_store
        .connection_is_current(&device_id, session_id)
        .await
        .map_err(rotation_claim_error)?
    {
        return Err(rotation_claim_error(PairingError::InvalidClaim));
    }
    Ok((device, session_id.to_string()))
}

fn evidence_error(error: EvidenceError) -> ApiError {
    match error {
        EvidenceError::Invalid(field) => {
            ApiError::bad_request(&format!("Invalid desktop evidence: {field}"))
        }
        EvidenceError::Disabled | EvidenceError::EncryptionRequired => ApiError {
            status: StatusCode::FORBIDDEN,
            message: error.to_string(),
        },
        EvidenceError::NotFound => ApiError::not_found("Desktop evidence not found"),
        EvidenceError::Conflict => ApiError {
            status: StatusCode::CONFLICT,
            message: error.to_string(),
        },
        EvidenceError::Store(_) => ApiError::internal("desktop_evidence_store"),
    }
}

fn evidence_matches_action(request: &EvidenceUploadRequest, action: &DesktopAction) -> bool {
    match action {
        DesktopAction::ReadSystemInformation => {
            request.selector_strategy == SelectorStrategy::NotApplicable
                && request.application_id == "system_information"
        }
        DesktopAction::InspectTargets { .. } => matches!(
            request.selector_strategy,
            SelectorStrategy::NotApplicable | SelectorStrategy::WindowAutomationId
        ),
        DesktopAction::FocusWindow { .. } => matches!(
            request.selector_strategy,
            SelectorStrategy::WindowAutomationId | SelectorStrategy::ApplicationIdentity
        ),
        DesktopAction::ClickElement { .. } | DesktopAction::TypeText { .. } => matches!(
            request.selector_strategy,
            SelectorStrategy::AutomationId
                | SelectorStrategy::ControlTypeAndName
                | SelectorStrategy::NameAndSibling
        ),
        DesktopAction::LaunchApplication { application_id } => {
            request.selector_strategy == SelectorStrategy::ApplicationIdentity
                && request.application_id == application_id.0
        }
    }
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
            "paired"
                | "online"
                | "offline"
                | "busy"
                | "awaiting_approval"
                | "degraded"
                | "suspended"
                | "revoked"
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
    state
        .device_connections
        .disconnect(&device_id, &format!("device_{target_state}"));
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

async fn connect_device(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, ApiError> {
    let (device_id, credential) = device_auth(&headers)?;
    if !state
        .rate_limiter
        .check_with_limit(&format!("desktop-connect:{device_id}"), 20)
    {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Device connection rate limit exceeded".to_string(),
        });
    }
    let session_id = uuid::Uuid::new_v4().to_string();
    let establishment = state
        .device_connections
        .establishment_guard(&device_id)
        .await;
    let device = state
        .device_pairing_store
        .connect_device(&device_id, &credential, &session_id)
        .await
        .map_err(rotation_claim_error)?;
    let lease = state
        .device_connections
        .replace(&device_id, session_id.clone());
    drop(establishment);

    let accepted = DeviceConnectionAccepted {
        device_id: device_id.clone(),
        session_id: session_id.clone(),
        server_time_unix_ms: unix_millis(),
        heartbeat_interval_seconds: HEARTBEAT_INTERVAL_SECONDS,
    };
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tx.send(Ok(Event::default()
        .event("connected")
        .data(serde_json::to_string(&accepted).unwrap_or_default())))
        .await
        .map_err(|_| ApiError::internal("desktop_connection_channel"))?;

    for pending in state
        .desktop_command_store
        .pending_for_device(&device_id)
        .await
    {
        if state
            .desktop_command_store
            .mark_delivered(&pending.command.command_id, unix_millis())
            .await
            .is_ok()
        {
            let _ = tx
                .send(Ok(Event::default()
                    .event("command")
                    .data(command_event_data(&pending.command))))
                .await;
        }
    }

    state.audit_store.record(
        &device.tenant_id,
        crate::audit::action::DEVICE_CONNECTED,
        "device",
        &device_id,
        Some(serde_json::json!({"session_id": session_id})),
    );
    let tenant_id = device.tenant_id.clone();
    let task_state = state.clone();
    tokio::spawn(async move {
        let mut cancellation = lease.cancellation;
        let mut device_events = lease.events;
        let mut device_events_open = true;
        let mut validation = tokio::time::interval(std::time::Duration::from_secs(15));
        validation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        validation.tick().await;
        loop {
            tokio::select! {
                changed = cancellation.changed() => {
                    if changed.is_ok() {
                        let reason = cancellation.borrow().clone().unwrap_or_else(|| "disconnected".to_string());
                        let _ = tx.send(Ok(Event::default().event("disconnect").data(
                            serde_json::json!({"reason": reason}).to_string()
                        ))).await;
                    }
                    break;
                }
                _ = tx.closed() => break,
                event = device_events.recv(), if device_events_open => {
                    match event {
                        Ok(DeviceEvent::Command(command)) => {
                            if task_state.desktop_command_store.mark_delivered(&command.command_id, unix_millis()).await.is_ok() {
                                let _ = tx.send(Ok(Event::default().event("command").data(
                                    command_event_data(&command)
                                ))).await;
                            }
                        }
                        Ok(DeviceEvent::Cancellation(cancellation)) => {
                            let _ = tx.send(Ok(Event::default().event("command_cancelled").data(
                                serde_json::to_string(&cancellation).unwrap_or_default()
                            ))).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => device_events_open = false,
                    }
                }
                _ = validation.tick() => {
                    let current = task_state
                        .device_pairing_store
                        .connection_is_current(&device_id, &lease.session_id)
                        .await
                        .unwrap_or(false);
                    if !current {
                        let _ = tx.send(Ok(Event::default().event("disconnect").data(
                            serde_json::json!({"reason": "session_invalidated"}).to_string()
                        ))).await;
                        break;
                    }
                }
            }
        }
        if task_state
            .device_connections
            .release(&device_id, &lease.session_id)
        {
            let _ = task_state
                .device_pairing_store
                .disconnect_device(&device_id, &lease.session_id)
                .await;
            task_state.audit_store.record(
                &tenant_id,
                crate::audit::action::DEVICE_DISCONNECTED,
                "device",
                &device_id,
                Some(serde_json::json!({"session_id": lease.session_id})),
            );
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("connection-alive"),
    ))
}

async fn heartbeat_device(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(envelope): Json<Envelope<Heartbeat>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (device_id, credential) = device_auth(&headers)?;
    if !state
        .rate_limiter
        .check_with_limit(&format!("desktop-heartbeat:{device_id}"), 180)
    {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Device heartbeat rate limit exceeded".to_string(),
        });
    }
    envelope
        .validate()
        .and_then(|_| envelope.payload.validate())
        .map_err(|error| ApiError::bad_request(&error.to_string()))?;
    if envelope.payload.device_id != device_id {
        return Err(rotation_claim_error(PairingError::InvalidClaim));
    }
    let session_id = headers
        .get("x-device-session-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
        .ok_or_else(|| rotation_claim_error(PairingError::InvalidClaim))?;
    let device = state
        .device_pairing_store
        .record_heartbeat(&device_id, &credential, session_id, &envelope.payload)
        .await
        .map_err(rotation_claim_error)?;
    Ok(Json(
        serde_json::to_value(HeartbeatAccepted {
            device_id,
            session_id: session_id.to_string(),
            state: envelope.payload.state,
            server_time_unix_ms: unix_millis(),
        })
        .unwrap_or_else(|_| serde_json::json!({"tenant_id": device.tenant_id})),
    ))
}

async fn dispatch_desktop_command(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<DispatchDesktopCommandRequest>,
) -> Result<
    (
        StatusCode,
        Json<crate::desktop_commands::DesktopCommandRecord>,
    ),
    ApiError,
> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let claims = claims.ok_or_else(|| ApiError::forbidden("Authentication required"))?;
    if request.lease_seconds == 0 || request.lease_seconds > 300 {
        return Err(ApiError::bad_request(
            "lease_seconds must be between 1 and 300",
        ));
    }
    if request.action.risk_level() > desktop_protocol::RiskLevel::Low {
        require_admin(Some(claims.clone()))?;
    }
    let execution = state
        .execution_service
        .get(&tenant_id, &request.execution_id)
        .await?;
    if !matches!(
        execution.status,
        ExecutionStatus::Running | ExecutionStatus::WaitingApproval
    ) {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "Workflow Execution is not active".to_string(),
        });
    }
    let workflow = state
        .workflow_service
        .get_workflow(&tenant_id, &execution.workflow_id)
        .await?;
    if workflow.project_id != request.project_id
        || (!claims.project_id.is_empty() && claims.project_id != request.project_id)
    {
        return Err(ApiError::forbidden(
            "Project does not own this Workflow Execution",
        ));
    }
    let device = state
        .device_pairing_store
        .get_device(&tenant_id, &request.device_id)
        .await
        .map_err(pairing_error)?
        .ok_or_else(|| ApiError::not_found("Device not found"))?;
    if !matches!(
        device.state.as_str(),
        "online" | "busy" | "awaiting_approval"
    ) || device.stale
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "Device is not eligible for command dispatch".to_string(),
        });
    }
    if !device.capabilities.contains(&request.action.capability()) {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "Device does not advertise the required capability".to_string(),
        });
    }
    let platform_major = env!("CARGO_PKG_VERSION").split('.').next();
    let device_major = device.agent_version.split('.').next();
    if device_major != platform_major {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "Device Agent version is incompatible".to_string(),
        });
    }
    let now = unix_millis();
    let actor_id = claims.user_id.clone().unwrap_or_else(|| claims.sub.clone());
    let approval = (request.action.risk_level() > desktop_protocol::RiskLevel::Low).then(|| {
        desktop_protocol::DesktopCommandApproval {
            approved_by: actor_id.clone(),
            expires_at_unix_ms: now + request.lease_seconds * 1000,
        }
    });
    let command = DesktopCommand {
        command_id: format!("desktop-command-{}", uuid::Uuid::new_v4()),
        execution_id: execution.id,
        tenant_id: tenant_id.clone(),
        project_id: request.project_id,
        requested_by: actor_id,
        issued_at_unix_ms: now,
        lease: ExecutionLease {
            lease_id: format!("desktop-lease-{}", uuid::Uuid::new_v4()),
            expires_at_unix_ms: now + request.lease_seconds * 1000,
        },
        approval,
        action: request.action,
    };
    command
        .validate(now)
        .map_err(|error| ApiError::bad_request(&error.to_string()))?;
    let record = state
        .desktop_command_store
        .create(command.clone(), request.device_id.clone(), workflow.id)
        .await
        .map_err(command_error)?;
    state
        .device_connections
        .send(&request.device_id, DeviceEvent::Command(Box::new(command)));
    Ok((StatusCode::CREATED, Json(record)))
}

async fn get_desktop_command(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(command_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<Json<crate::desktop_commands::DesktopCommandRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .desktop_command_store
        .get(&tenant_id, &command_id)
        .await
        .map(Json)
        .map_err(command_error)
}

async fn cancel_desktop_command(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(command_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<Json<crate::desktop_commands::DesktopCommandRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let record = state
        .desktop_command_store
        .cancel(&tenant_id, &command_id)
        .await
        .map_err(command_error)?;
    state.device_connections.send(
        &record.device_id,
        DeviceEvent::Cancellation(DesktopCommandCancellation {
            command_id: record.command.command_id.clone(),
            execution_id: record.command.execution_id.clone(),
            reason: "cancelled_by_platform".to_string(),
        }),
    );
    Ok(Json(record))
}

async fn acknowledge_desktop_command(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(envelope): Json<Envelope<DesktopCommandAcknowledgement>>,
) -> Result<Json<crate::desktop_commands::DesktopCommandRecord>, ApiError> {
    let (device, _) = authenticated_device_session(&state, &headers).await?;
    envelope
        .validate()
        .and_then(|_| envelope.payload.validate())
        .map_err(|error| ApiError::bad_request(&error.to_string()))?;
    state
        .desktop_command_store
        .acknowledge(&device.id, &envelope.payload, unix_millis())
        .await
        .map(Json)
        .map_err(command_error)
}

async fn complete_desktop_command(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(envelope): Json<Envelope<DesktopCommandResult>>,
) -> Result<Json<crate::desktop_commands::DesktopCommandRecord>, ApiError> {
    let (device, _) = authenticated_device_session(&state, &headers).await?;
    envelope
        .validate()
        .and_then(|_| envelope.payload.validate())
        .map_err(|error| ApiError::bad_request(&error.to_string()))?;
    state
        .desktop_command_store
        .complete(&device.id, envelope.payload)
        .await
        .map(Json)
        .map_err(command_error)
}

#[derive(serde::Deserialize)]
struct EvidenceListQuery {
    tenant_id: String,
    execution_id: String,
}

#[derive(serde::Serialize)]
struct EvidenceExport {
    execution_id: String,
    exported_at_unix_ms: u64,
    records: Vec<EvidenceRecord>,
}

async fn upload_desktop_evidence(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<EvidenceUploadRequest>,
) -> Result<(StatusCode, Json<EvidenceRecord>), ApiError> {
    let (device, _) = authenticated_device_session(&state, &headers).await?;
    let command = state
        .desktop_command_store
        .get(&device.tenant_id, &request.command_id)
        .await
        .map_err(command_error)?;
    let matching_result = command.result.as_ref().is_some_and(|result| {
        result.execution_id == request.execution_id && result.outcome == request.outcome
    });
    if command.device_id != device.id
        || command.command.execution_id != request.execution_id
        || command.command.project_id != request.project_id
        || !evidence_matches_action(&request, &command.command.action)
        || !matching_result
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "Desktop evidence does not match the terminal command".to_owned(),
        });
    }
    let prepared = prepare_evidence(
        &device.tenant_id,
        &device.id,
        request,
        &state.desktop_evidence_policy,
        unix_millis(),
    )
    .map_err(evidence_error)?;
    let record = state
        .desktop_evidence_store
        .create(prepared)
        .await
        .map_err(evidence_error)?;
    state.audit_store.record(
        &record.tenant_id,
        crate::audit::action::DESKTOP_EVIDENCE_RECORDED,
        "desktop_evidence",
        &record.evidence_id,
        Some(serde_json::json!({
            "execution_id": record.execution_id,
            "command_id": record.command_id,
            "device_id": record.device_id,
            "kind": record.kind,
            "selector_strategy": record.selector_strategy,
            "application_id": record.application_id,
            "started_at_unix_ms": record.started_at_unix_ms,
            "completed_at_unix_ms": record.completed_at_unix_ms,
            "outcome": record.outcome,
            "redacted_regions": record.redacted_regions,
        })),
    );
    Ok((StatusCode::CREATED, Json(record)))
}

async fn list_desktop_evidence(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<EvidenceListQuery>,
) -> Result<Json<Vec<EvidenceRecord>>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .desktop_evidence_store
        .list(&tenant_id, &query.execution_id)
        .await
        .map(Json)
        .map_err(evidence_error)
}

async fn export_desktop_evidence(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<EvidenceListQuery>,
) -> Result<Json<EvidenceExport>, ApiError> {
    require_admin(claims.clone())?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let records = state
        .desktop_evidence_store
        .list(&tenant_id, &query.execution_id)
        .await
        .map_err(evidence_error)?;
    Ok(Json(EvidenceExport {
        execution_id: query.execution_id,
        exported_at_unix_ms: unix_millis(),
        records,
    }))
}

async fn delete_desktop_evidence(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(evidence_id): Path<String>,
    Query(query): Query<GetExecutionQuery>,
) -> Result<StatusCode, ApiError> {
    let claims = require_admin(claims)?;
    let tenant_id = effective_tenant_id(&Some(claims.clone()), &query.tenant_id);
    state
        .desktop_evidence_store
        .delete(&tenant_id, &evidence_id)
        .await
        .map_err(evidence_error)?;
    state.audit_store.record(
        &tenant_id,
        crate::audit::action::DESKTOP_EVIDENCE_DELETED,
        "desktop_evidence",
        &evidence_id,
        Some(serde_json::json!({
            "deleted_by": claims.user_id.unwrap_or(claims.sub),
        })),
    );
    Ok(StatusCode::NO_CONTENT)
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
        .route("/v1/desktop/device-connection", get(connect_device))
        .route("/v1/desktop/device-heartbeats", post(heartbeat_device))
        .route("/v1/desktop/commands", post(dispatch_desktop_command))
        .route(
            "/v1/desktop/commands/:command_id",
            get(get_desktop_command).delete(cancel_desktop_command),
        )
        .route(
            "/v1/desktop/device-command-acknowledgements",
            post(acknowledge_desktop_command),
        )
        .route(
            "/v1/desktop/device-command-results",
            post(complete_desktop_command),
        )
        .route(
            "/v1/desktop/device-evidence",
            post(upload_desktop_evidence).layer(DefaultBodyLimit::max(1_400_000)),
        )
        .route("/v1/desktop/evidence", get(list_desktop_evidence))
        .route("/v1/desktop/evidence/export", get(export_desktop_evidence))
        .route(
            "/v1/desktop/evidence/:evidence_id",
            delete(delete_desktop_evidence),
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

    #[tokio::test]
    async fn secure_connection_accepts_clock_skew_and_newest_session_owns_heartbeat() {
        use tokio_stream::StreamExt;

        let state = default_app_state();
        let command_store = state.desktop_command_store.clone();
        let evidence_store = state.desktop_evidence_store.clone();
        let audit_store = state.audit_store.clone();
        let app = build_router(state);
        let admin_token = crate::auth::sign_token(&Claims {
            sub: "admin-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            role: crate::auth::Role::Admin,
            ..Default::default()
        })
        .unwrap();
        let create = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/pairing-sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        create_body("device-connection-http").to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = response_json(create).await;
        app.clone()
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
        let credential = response_json(claim).await["credential"]
            .as_str()
            .unwrap()
            .to_string();

        let insecure = app
            .clone()
            .oneshot(
                Request::get("/v1/desktop/device-connection")
                    .header("x-device-id", "device-connection-http")
                    .header("authorization", format!("Device {credential}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(insecure.status(), StatusCode::UPGRADE_REQUIRED);

        let connect = app
            .clone()
            .oneshot(
                Request::get("/v1/desktop/device-connection")
                    .header("x-forwarded-proto", "https")
                    .header("x-device-id", "device-connection-http")
                    .header("authorization", format!("Device {credential}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(connect.status(), StatusCode::OK);
        let mut first_stream = connect.into_body().into_data_stream();
        let first_chunk = first_stream.next().await.unwrap().unwrap();
        let first_text = String::from_utf8(first_chunk.to_vec()).unwrap();
        let first_data = first_text
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap();
        let first_connected: serde_json::Value = serde_json::from_str(first_data).unwrap();
        let first_session = first_connected["session_id"].as_str().unwrap().to_string();

        let heartbeat = serde_json::json!({
            "protocol_version": desktop_protocol::PROTOCOL_VERSION,
            "message_id": "clock-skew-heartbeat",
            "sent_at_unix_ms": 0,
            "payload": {
                "device_id": "device-connection-http",
                "state": "busy",
                "active_execution_id": "execution-1",
                "agent_version": "1.1.0",
                "capabilities": ["system_information"],
                "health_detail": null
            }
        });
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/device-heartbeats")
                    .header("x-forwarded-proto", "https")
                    .header("x-device-id", "device-connection-http")
                    .header("x-device-session-id", &first_session)
                    .header("authorization", format!("Device {credential}"))
                    .header("content-type", "application/json")
                    .body(Body::from(heartbeat.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response_json(response).await["server_time_unix_ms"]
                .as_u64()
                .unwrap()
                > 0
        );

        let reconnect = app
            .clone()
            .oneshot(
                Request::get("/v1/desktop/device-connection")
                    .header("x-forwarded-proto", "https")
                    .header("x-device-id", "device-connection-http")
                    .header("authorization", format!("Device {credential}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(reconnect.status(), StatusCode::OK);
        let mut reconnect_stream = reconnect.into_body().into_data_stream();
        let connected_chunk = reconnect_stream.next().await.unwrap().unwrap();
        let connected_text = String::from_utf8(connected_chunk.to_vec()).unwrap();
        let connected_data = connected_text
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap();
        let reconnected: serde_json::Value = serde_json::from_str(connected_data).unwrap();
        let current_session = reconnected["session_id"].as_str().unwrap().to_string();
        let replaced_chunk =
            tokio::time::timeout(std::time::Duration::from_secs(1), first_stream.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert!(String::from_utf8(replaced_chunk.to_vec())
            .unwrap()
            .contains("replaced_by_newer_session"));

        let stale_session = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/device-heartbeats")
                    .header("x-forwarded-proto", "https")
                    .header("x-device-id", "device-connection-http")
                    .header("x-device-session-id", first_session)
                    .header("authorization", format!("Device {credential}"))
                    .header("content-type", "application/json")
                    .body(Body::from(heartbeat.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stale_session.status(), StatusCode::NOT_FOUND);

        let execution = app
            .clone()
            .oneshot(
                Request::post("/v1/executions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "tenant_id": "tenant-1",
                            "workflow_id": "workflow-1",
                            "workflow_version_id": "version-1",
                            "graph": {
                                "workflow_version_id": "version-1",
                                "nodes": [
                                    {"id": "trigger", "type": "trigger"},
                                    {"id": "pause", "type": "delay", "config": {"seconds": 5}}
                                ],
                                "edges": [{"source": "trigger", "target": "pause"}]
                            },
                            "input_json": "{}"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(execution.status(), StatusCode::ACCEPTED);
        let execution_id = response_json(execution).await["id"]
            .as_str()
            .unwrap()
            .to_string();

        let rejected = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/commands")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"tenant_id":"tenant-1","project_id":"project-1","execution_id":execution_id,"device_id":"device-connection-http","action":{"kind":"focus_window","selector":{"executable":"notepad.exe","title":null,"automation_id":null}}}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rejected.status(), StatusCode::CONFLICT);

        let dispatch = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/commands")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"tenant_id":"tenant-1","project_id":"project-1","execution_id":execution_id,"device_id":"device-connection-http","action":{"kind":"read_system_information"},"lease_seconds":30}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(dispatch.status(), StatusCode::CREATED);
        let dispatched = response_json(dispatch).await;
        let command_id = dispatched["command"]["command_id"].as_str().unwrap();
        let lease_id = dispatched["command"]["lease"]["lease_id"].as_str().unwrap();

        let command_chunk =
            tokio::time::timeout(std::time::Duration::from_secs(1), reconnect_stream.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert!(String::from_utf8(command_chunk.to_vec())
            .unwrap()
            .contains("event: command"));

        let ack = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/device-command-acknowledgements")
                    .header("x-forwarded-proto", "https")
                    .header("x-device-id", "device-connection-http")
                    .header("x-device-session-id", &current_session)
                    .header("authorization", format!("Device {credential}"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"protocol_version":"desktop.v1","message_id":"ack-1","sent_at_unix_ms":0,"payload":{"command_id":command_id,"execution_id":execution_id,"lease_id":lease_id,"acknowledged_at_unix_ms":0}}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ack.status(), StatusCode::OK);
        assert_eq!(response_json(ack).await["status"], "acknowledged");

        let result_body = serde_json::json!({"protocol_version":"desktop.v1","message_id":"result-1","sent_at_unix_ms":0,"payload":{"command_id":command_id,"execution_id":execution_id,"outcome":"succeeded","completed_at_unix_ms":0,"output":{"hostname":"desktop"},"error_code":null,"error_message":null}});
        for expected in [StatusCode::OK, StatusCode::OK] {
            let result = app
                .clone()
                .oneshot(
                    Request::post("/v1/desktop/device-command-results")
                        .header("x-forwarded-proto", "https")
                        .header("x-device-id", "device-connection-http")
                        .header("x-device-session-id", &current_session)
                        .header("authorization", format!("Device {credential}"))
                        .header("content-type", "application/json")
                        .body(Body::from(result_body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(result.status(), expected);
            assert_eq!(response_json(result).await["status"], "succeeded");
        }

        let evidence_body = serde_json::json!({
            "command_id": command_id,
            "execution_id": execution_id,
            "project_id": "project-1",
            "kind": "adapter_audit",
            "selector_strategy": "not_applicable",
            "application_id": "system_information",
            "started_at_unix_ms": 1000,
            "completed_at_unix_ms": 1100,
            "outcome": "succeeded",
            "retention_days": 7,
            "capture_opt_in": false,
            "redaction": {
                "policy_version": "redaction-v1",
                "succeeded": true,
                "sensitive_regions": 0,
                "redacted_regions": 0
            }
        });
        let unauthenticated_evidence = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/device-evidence")
                    .header("content-type", "application/json")
                    .body(Body::from(evidence_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            unauthenticated_evidence.status(),
            StatusCode::UPGRADE_REQUIRED
        );
        let evidence = app
            .clone()
            .oneshot(
                Request::post("/v1/desktop/device-evidence")
                    .header("x-forwarded-proto", "https")
                    .header("x-device-id", "device-connection-http")
                    .header("x-device-session-id", &current_session)
                    .header("authorization", format!("Device {credential}"))
                    .header("content-type", "application/json")
                    .body(Body::from(evidence_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(evidence.status(), StatusCode::CREATED);
        let evidence = response_json(evidence).await;
        assert!(evidence.get("payload_ciphertext").is_none());
        assert_eq!(
            evidence_store
                .list("tenant-1", &execution_id)
                .await
                .unwrap()
                .len(),
            1
        );

        let export = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/v1/desktop/evidence/export?tenant_id=tenant-1&execution_id={execution_id}"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(export.status(), StatusCode::OK);
        let exported = response_json(export).await;
        assert_eq!(exported["records"].as_array().unwrap().len(), 1);
        assert!(exported["records"][0].get("payload_ciphertext").is_none());
        let audit_json = serde_json::to_string(&audit_store.list("tenant-1", 20).await).unwrap();
        assert!(audit_json.contains(crate::audit::action::DESKTOP_EVIDENCE_RECORDED));
        assert!(!audit_json.contains("payload_base64"));

        let cancellable = command_store
            .create(
                DesktopCommand {
                    command_id: "cancel-command".to_string(),
                    execution_id: execution_id.clone(),
                    tenant_id: "tenant-1".to_string(),
                    project_id: "project-1".to_string(),
                    requested_by: "admin-1".to_string(),
                    issued_at_unix_ms: unix_millis(),
                    lease: ExecutionLease {
                        lease_id: "cancel-lease".to_string(),
                        expires_at_unix_ms: unix_millis() + 30_000,
                    },
                    approval: None,
                    action: DesktopAction::ReadSystemInformation,
                },
                "device-connection-http".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        let cancelled = app
            .clone()
            .oneshot(
                Request::delete(format!(
                    "/v1/desktop/commands/{}?tenant_id=tenant-1",
                    cancellable.command.command_id
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cancelled.status(), StatusCode::OK);
        assert_eq!(response_json(cancelled).await["status"], "cancelled");
        let cancellation_chunk =
            tokio::time::timeout(std::time::Duration::from_secs(1), reconnect_stream.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert!(String::from_utf8(cancellation_chunk.to_vec())
            .unwrap()
            .contains("event: command_cancelled"));

        let timeout_command = DesktopCommand {
            command_id: "timeout-command".to_string(),
            execution_id: execution_id.clone(),
            tenant_id: "tenant-1".to_string(),
            project_id: "project-1".to_string(),
            requested_by: "admin-1".to_string(),
            issued_at_unix_ms: 100,
            lease: ExecutionLease {
                lease_id: "timeout-lease".to_string(),
                expires_at_unix_ms: 200,
            },
            approval: None,
            action: DesktopAction::ReadSystemInformation,
        };
        command_store
            .create(
                timeout_command,
                "device-connection-http".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(command_store.expire(200).await[0].status, "timed_out");
    }
}
