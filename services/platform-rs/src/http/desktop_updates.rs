use super::*;
use crate::desktop_update_policy::{DesktopUpdatePolicyError, UpdateDesktopPolicyRequest};
use desktop_release::{classify_fleet_device, FleetCompliance, FleetDeviceVersion, FleetLifecycle};

const MAX_FLEET_SCAN: u32 = 10_000;

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FleetComplianceQuery {
    state: Option<String>,
    compliance: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize)]
struct FleetComplianceItem {
    device_id: String,
    agent_version: String,
    lifecycle: String,
    last_seen_at_unix_seconds: Option<u64>,
    compliance: &'static str,
    remediation: &'static str,
}

#[derive(Debug, Serialize)]
struct FleetComplianceResponse {
    policy_revision: u64,
    required_version: String,
    items: Vec<FleetComplianceItem>,
    next_offset: Option<u32>,
}

async fn get_policy(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let claims = require_desktop_admin(claims)?;
    let policy = state
        .desktop_update_policy_store
        .get(&claims.tenant_id)
        .await
        .map_err(policy_error)?;
    Ok(Json(serde_json::to_value(policy).unwrap_or_default()))
}

async fn update_policy(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<UpdateDesktopPolicyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let claims = require_desktop_admin(claims)?;
    let actor = claims
        .user_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&claims.sub);
    let policy = state
        .desktop_update_policy_store
        .update(&claims.tenant_id, actor, request)
        .await
        .map_err(policy_error)?;
    state.audit_store.record(
        &claims.tenant_id,
        crate::audit::action::DESKTOP_UPDATE_POLICY_CHANGED,
        "desktop_update_policy",
        &claims.tenant_id,
        Some(serde_json::json!({
            "actor_id": actor,
            "revision": policy.revision,
            "mode": policy.mode,
            "channel": policy.channel,
        })),
    );
    Ok(Json(serde_json::to_value(policy).unwrap_or_default()))
}

async fn fleet_compliance(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<FleetComplianceQuery>,
) -> Result<Json<FleetComplianceResponse>, ApiError> {
    let claims = require_desktop_admin(claims)?;
    let state_filter = query.state.as_deref().map(str::trim);
    if state_filter.is_some_and(|value| !valid_lifecycle(value)) {
        return Err(ApiError::bad_request("Invalid device state filter"));
    }
    let compliance_filter = query.compliance.as_deref().map(str::trim);
    if compliance_filter.is_some_and(|value| !valid_compliance(value)) {
        return Err(ApiError::bad_request("Invalid compliance filter"));
    }
    if query.limit.is_some_and(|value| !(1..=100).contains(&value))
        || query.offset.is_some_and(|value| value > MAX_FLEET_SCAN)
    {
        return Err(ApiError::bad_request("Invalid fleet pagination"));
    }
    let policy = state
        .desktop_update_policy_store
        .get(&claims.tenant_id)
        .await
        .map_err(policy_error)?;
    let now = now_unix_seconds();
    let mut source_offset = 0;
    let mut classified = Vec::new();
    loop {
        let page = state
            .device_pairing_store
            .list_devices(&claims.tenant_id, state_filter, 100, source_offset)
            .await
            .map_err(desktop_devices::pairing_error)?;
        for device in page.items {
            let lifecycle = lifecycle(&device.state);
            let inventory = FleetDeviceVersion {
                device_id: device.id.clone(),
                agent_version: device.agent_version.clone(),
                last_seen_at_unix_seconds: device
                    .last_seen_at
                    .and_then(|value| u64::try_from(value).ok())
                    .unwrap_or(0),
                lifecycle,
            };
            let compliance = classify_fleet_device(&inventory, &policy.required_version, now);
            if compliance_filter.is_none_or(|filter| compliance_name(compliance) == filter) {
                classified.push(FleetComplianceItem {
                    device_id: device.id,
                    agent_version: device.agent_version,
                    lifecycle: device.state,
                    last_seen_at_unix_seconds: device
                        .last_seen_at
                        .and_then(|value| u64::try_from(value).ok()),
                    compliance: compliance_name(compliance),
                    remediation: remediation(compliance),
                });
            }
        }
        match page.next_offset {
            Some(next) if next <= MAX_FLEET_SCAN => source_offset = next,
            Some(_) => {
                return Err(ApiError::bad_request(
                    "Fleet inventory exceeds bounded scan limit",
                ))
            }
            None => break,
        }
    }
    let offset = usize::try_from(query.offset.unwrap_or(0)).unwrap_or(usize::MAX);
    let limit = usize::try_from(query.limit.unwrap_or(50)).unwrap_or(100);
    let total = classified.len();
    let items = classified.into_iter().skip(offset).take(limit).collect();
    let consumed = offset.saturating_add(limit).min(total);
    Ok(Json(FleetComplianceResponse {
        policy_revision: policy.revision,
        required_version: policy.required_version,
        items,
        next_offset: (consumed < total).then(|| u32::try_from(consumed).unwrap_or(u32::MAX)),
    }))
}

fn require_desktop_admin(claims: Option<Claims>) -> Result<Claims, ApiError> {
    let claims = claims.ok_or_else(|| ApiError::forbidden("Authentication required"))?;
    if claims.tenant_id.trim().is_empty() {
        return Err(ApiError::forbidden("Tenant context required"));
    }
    if !claims.is_admin() {
        return Err(ApiError::forbidden("Admin role required"));
    }
    Ok(claims)
}

fn lifecycle(state: &str) -> FleetLifecycle {
    match state {
        "suspended" => FleetLifecycle::Suspended,
        "revoked" => FleetLifecycle::Revoked,
        _ => FleetLifecycle::Active,
    }
}

fn valid_lifecycle(value: &str) -> bool {
    matches!(
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
}

fn valid_compliance(value: &str) -> bool {
    matches!(
        value,
        "compliant"
            | "update_required"
            | "ahead_of_policy"
            | "stale"
            | "suspended"
            | "revoked"
            | "invalid_inventory"
    )
}

fn compliance_name(value: FleetCompliance) -> &'static str {
    match value {
        FleetCompliance::Compliant => "compliant",
        FleetCompliance::UpdateRequired => "update_required",
        FleetCompliance::AheadOfPolicy => "ahead_of_policy",
        FleetCompliance::Stale => "stale",
        FleetCompliance::Suspended => "suspended",
        FleetCompliance::Revoked => "revoked",
        FleetCompliance::InvalidInventory => "invalid_inventory",
    }
}

fn remediation(value: FleetCompliance) -> &'static str {
    match value {
        FleetCompliance::Compliant => "none",
        FleetCompliance::UpdateRequired => "schedule_update",
        FleetCompliance::AheadOfPolicy => "review_channel_assignment",
        FleetCompliance::Stale => "restore_device_health",
        FleetCompliance::Suspended => "review_suspension",
        FleetCompliance::Revoked => "replace_or_repair_device",
        FleetCompliance::InvalidInventory => "repair_inventory",
    }
}

fn policy_error(error: DesktopUpdatePolicyError) -> ApiError {
    match error {
        DesktopUpdatePolicyError::InvalidRequest => {
            ApiError::bad_request("Invalid desktop update policy")
        }
        DesktopUpdatePolicyError::Conflict => ApiError {
            status: StatusCode::CONFLICT,
            message: "Desktop update policy revision conflict".to_owned(),
        },
        DesktopUpdatePolicyError::StoreUnavailable => ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Desktop update policy store unavailable".to_owned(),
        },
    }
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/desktop/update-policy",
            get(get_policy).patch(update_policy),
        )
        .route("/v1/desktop/fleet-compliance", get(fleet_compliance))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Role;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use desktop_protocol::{DeviceCapability, DeviceDescriptor};
    use tower::ServiceExt;

    fn token(tenant_id: &str, role: Role) -> String {
        crate::auth::sign_token(&Claims {
            sub: "admin-1".to_owned(),
            tenant_id: tenant_id.to_owned(),
            workspace_id: "workspace-1".to_owned(),
            project_id: "project-1".to_owned(),
            exp: now_unix_seconds() + 3600,
            role,
            user_id: Some("admin-1".to_owned()),
            email: None,
        })
        .unwrap()
    }

    async fn json(response: Response) -> serde_json::Value {
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn policy_api_is_tenant_scoped_and_revision_safe() {
        let state = default_app_state();
        let audit_store = state.audit_store.clone();
        let app = build_router(state);
        let admin = token("tenant-a", Role::Admin);
        let default = app
            .clone()
            .oneshot(
                Request::get("/v1/desktop/update-policy")
                    .header("authorization", format!("Bearer {admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(default.status(), StatusCode::OK);
        assert_eq!(json(default).await["mode"], "disabled");

        let body = serde_json::json!({
            "observed_revision": 0,
            "mode": "manual",
            "channel": "stable",
            "required_version": "1.5.1",
            "pinned_version": null,
            "maintenance_window": null,
            "allow_offline_import": false,
            "allow_emergency_rollback": true
        })
        .to_string();
        let saved = app
            .clone()
            .oneshot(
                Request::patch("/v1/desktop/update-policy")
                    .header("authorization", format!("Bearer {admin}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saved.status(), StatusCode::OK);
        assert_eq!(json(saved).await["revision"], 1);
        let events = audit_store.list("tenant-a", 10).await;
        assert_eq!(
            events[0].action,
            crate::audit::action::DESKTOP_UPDATE_POLICY_CHANGED
        );
        let detail = events[0].detail.as_deref().unwrap_or_default();
        assert!(!detail.contains("required_version"));
        assert!(!detail.contains("offline"));

        let stale = app
            .clone()
            .oneshot(
                Request::patch("/v1/desktop/update-policy")
                    .header("authorization", format!("Bearer {admin}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stale.status(), StatusCode::CONFLICT);

        let other = token("tenant-b", Role::Admin);
        let isolated = app
            .clone()
            .oneshot(
                Request::get("/v1/desktop/update-policy")
                    .header("authorization", format!("Bearer {other}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(json(isolated).await["revision"], 0);

        let editor = token("tenant-a", Role::Editor);
        let forbidden = app
            .oneshot(
                Request::get("/v1/desktop/update-policy")
                    .header("authorization", format!("Bearer {editor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn fleet_api_classifies_bounded_inventory() {
        let state = default_app_state();
        let session = state
            .device_pairing_store
            .create_session(crate::device_pairing::CreatePairingSessionRequest {
                device: DeviceDescriptor {
                    device_id: "fleet-device-1".to_owned(),
                    display_name: "Fleet Device".to_owned(),
                    operating_system: "windows".to_owned(),
                    agent_version: "1.0.0".to_owned(),
                    capabilities: vec![DeviceCapability::SystemInformation],
                },
                device_public_key: "A".repeat(64),
            })
            .await
            .unwrap();
        state
            .device_pairing_store
            .approve(&session.pairing_code, "tenant-a", "admin-1")
            .await
            .unwrap();
        let app = build_router(state);
        let response = app
            .oneshot(
                Request::get("/v1/desktop/fleet-compliance?compliance=stale&limit=10")
                    .header(
                        "authorization",
                        format!("Bearer {}", token("tenant-a", Role::Admin)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json(response).await;
        assert_eq!(body["items"][0]["device_id"], "fleet-device-1");
        assert_eq!(body["items"][0]["compliance"], "stale");
        assert!(body["items"][0].get("capabilities").is_none());
    }
}
