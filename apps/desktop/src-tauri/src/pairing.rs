use desktop_identity::{DeviceCredentialStore, IdentityError};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use url::Url;

const MAX_DISPLAY_NAME_LENGTH: usize = 128;
const CLAIM_SECRET_LENGTH: usize = 71;
const DEVICE_CREDENTIAL_LENGTH: usize = 73;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingPhase {
    Unavailable,
    Unpaired,
    WaitingForApproval,
    Paired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PairingSnapshot {
    pub revision: u64,
    pub phase: PairingPhase,
    pub device_id: Option<String>,
    pub pairing_code: Option<String>,
    pub expires_at_unix_seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartPairingInput {
    pub platform_url: String,
    pub display_name: String,
}

impl StartPairingInput {
    pub fn validate(&self) -> Result<ValidatedPairingInput, PairingIpcError> {
        let display_name = self.display_name.trim();
        if display_name.is_empty()
            || display_name.len() > MAX_DISPLAY_NAME_LENGTH
            || display_name.chars().any(char::is_control)
        {
            return Err(PairingIpcError::InvalidRequest(
                "display name must contain 1 to 128 visible characters",
            ));
        }
        let platform_url = normalize_platform_url(&self.platform_url)?;
        Ok(ValidatedPairingInput {
            platform_url,
            display_name: display_name.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPairingInput {
    pub platform_url: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingSessionCreated {
    pub session_id: String,
    pub pairing_code: String,
    pub claim_secret: String,
    pub expires_at: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) struct ClaimedDeviceCredential {
    pub device_id: String,
    pub tenant_id: String,
    pub credential_id: String,
    pub credential: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum PairingIpcError {
    InvalidRequest(&'static str),
    InvalidPlatformResponse,
    InvalidState,
    Expired,
    TransportUnavailable,
    SecureStorageUnavailable,
    StateUnavailable,
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
pub(crate) struct PendingClaim {
    pub platform_url: String,
    pub session_id: String,
    pub claim_secret: String,
    pub device_id: String,
}

struct PendingPairing {
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    platform_url: String,
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    session_id: String,
    pairing_code: String,
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    claim_secret: String,
    device_id: String,
    expires_at: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPairing {
    schema_version: u8,
    platform_url: String,
    device_id: String,
    credential: String,
}

#[derive(Clone)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) struct ConnectionSecret {
    pub platform_url: String,
    pub device_id: String,
    pub credential: String,
}

enum PairingState {
    Unavailable,
    Unpaired,
    Waiting(PendingPairing),
    Paired { device_id: String },
}

struct VersionedPairingState {
    revision: u64,
    state: PairingState,
}

impl Default for VersionedPairingState {
    fn default() -> Self {
        Self {
            revision: 1,
            state: PairingState::Unpaired,
        }
    }
}

#[derive(Default)]
pub struct PairingController {
    state: Mutex<VersionedPairingState>,
}

impl PairingController {
    pub fn mark_unavailable(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.revision = state.revision.saturating_add(1);
            state.state = PairingState::Unavailable;
        }
    }

    pub fn snapshot(&self) -> Result<PairingSnapshot, PairingIpcError> {
        let state = self
            .state
            .lock()
            .map_err(|_| PairingIpcError::StateUnavailable)?;
        Ok(snapshot_from(&state))
    }

    pub fn begin(
        &self,
        platform_url: String,
        device_id: String,
        created: PairingSessionCreated,
        now_unix_seconds: i64,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        validate_created_session(&created, now_unix_seconds)?;
        let platform_url = normalize_platform_url(&platform_url)?;
        validate_device_id(&device_id)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairingIpcError::StateUnavailable)?;
        if matches!(state.state, PairingState::Paired { .. }) {
            return Err(PairingIpcError::InvalidState);
        }
        state.revision = state.revision.saturating_add(1);
        state.state = PairingState::Waiting(PendingPairing {
            platform_url,
            session_id: created.session_id,
            pairing_code: created.pairing_code,
            claim_secret: created.claim_secret,
            device_id,
            expires_at: created.expires_at,
        });
        Ok(snapshot_from(&state))
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn pending_claim(
        &self,
        now_unix_seconds: i64,
    ) -> Result<PendingClaim, PairingIpcError> {
        let state = self
            .state
            .lock()
            .map_err(|_| PairingIpcError::StateUnavailable)?;
        let PairingState::Waiting(pending) = &state.state else {
            return Err(PairingIpcError::InvalidState);
        };
        if pending.expires_at <= now_unix_seconds {
            return Err(PairingIpcError::Expired);
        }
        Ok(PendingClaim {
            platform_url: pending.platform_url.clone(),
            session_id: pending.session_id.clone(),
            claim_secret: pending.claim_secret.clone(),
            device_id: pending.device_id.clone(),
        })
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn complete(
        &self,
        claimed: ClaimedDeviceCredential,
        store: &impl DeviceCredentialStore,
        now_unix_seconds: i64,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairingIpcError::StateUnavailable)?;
        let PairingState::Waiting(pending) = &state.state else {
            return Err(PairingIpcError::InvalidState);
        };
        if pending.expires_at <= now_unix_seconds {
            return Err(PairingIpcError::Expired);
        }
        validate_claimed_credential(&claimed, &pending.device_id)?;
        let stored = StoredPairing {
            schema_version: 1,
            platform_url: pending.platform_url.clone(),
            device_id: pending.device_id.clone(),
            credential: claimed.credential,
        };
        let encoded = serde_json::to_string(&stored)
            .map_err(|_| PairingIpcError::SecureStorageUnavailable)?;
        store
            .store(&encoded)
            .map_err(|_| PairingIpcError::SecureStorageUnavailable)?;

        let device_id = pending.device_id.clone();
        state.revision = state.revision.saturating_add(1);
        state.state = PairingState::Paired { device_id };
        Ok(snapshot_from(&state))
    }

    pub fn restore(
        &self,
        store: &impl DeviceCredentialStore,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        let Some(encoded) = store
            .load()
            .map_err(|_| PairingIpcError::SecureStorageUnavailable)?
        else {
            return self.snapshot();
        };
        let stored: StoredPairing = serde_json::from_str(&encoded)
            .map_err(|_| PairingIpcError::SecureStorageUnavailable)?;
        if stored.schema_version != 1 {
            return Err(PairingIpcError::SecureStorageUnavailable);
        }
        normalize_platform_url(&stored.platform_url)?;
        validate_device_id(&stored.device_id)?;
        validate_credential(&stored.credential)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairingIpcError::StateUnavailable)?;
        state.revision = state.revision.saturating_add(1);
        state.state = PairingState::Paired {
            device_id: stored.device_id,
        };
        Ok(snapshot_from(&state))
    }

    pub fn forget(
        &self,
        store: &impl DeviceCredentialStore,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        store
            .delete()
            .map_err(|_| PairingIpcError::SecureStorageUnavailable)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairingIpcError::StateUnavailable)?;
        state.revision = state.revision.saturating_add(1);
        state.state = PairingState::Unpaired;
        Ok(snapshot_from(&state))
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn connection_secret(
        &self,
        store: &impl DeviceCredentialStore,
    ) -> Result<Option<ConnectionSecret>, PairingIpcError> {
        let expected_device_id = {
            let state = self
                .state
                .lock()
                .map_err(|_| PairingIpcError::StateUnavailable)?;
            let PairingState::Paired { device_id } = &state.state else {
                return Ok(None);
            };
            device_id.clone()
        };
        let Some(encoded) = store
            .load()
            .map_err(|_| PairingIpcError::SecureStorageUnavailable)?
        else {
            return Err(PairingIpcError::SecureStorageUnavailable);
        };
        let stored: StoredPairing = serde_json::from_str(&encoded)
            .map_err(|_| PairingIpcError::SecureStorageUnavailable)?;
        if stored.schema_version != 1 || stored.device_id != expected_device_id {
            return Err(PairingIpcError::SecureStorageUnavailable);
        }
        let platform_url = normalize_platform_url(&stored.platform_url)?;
        validate_device_id(&stored.device_id)?;
        validate_credential(&stored.credential)?;
        Ok(Some(ConnectionSecret {
            platform_url,
            device_id: stored.device_id,
            credential: stored.credential,
        }))
    }
}

fn snapshot_from(state: &VersionedPairingState) -> PairingSnapshot {
    match &state.state {
        PairingState::Unavailable => PairingSnapshot {
            revision: state.revision,
            phase: PairingPhase::Unavailable,
            device_id: None,
            pairing_code: None,
            expires_at_unix_seconds: None,
        },
        PairingState::Unpaired => PairingSnapshot {
            revision: state.revision,
            phase: PairingPhase::Unpaired,
            device_id: None,
            pairing_code: None,
            expires_at_unix_seconds: None,
        },
        PairingState::Waiting(pending) => PairingSnapshot {
            revision: state.revision,
            phase: PairingPhase::WaitingForApproval,
            device_id: Some(pending.device_id.clone()),
            pairing_code: Some(pending.pairing_code.clone()),
            expires_at_unix_seconds: Some(pending.expires_at),
        },
        PairingState::Paired { device_id } => PairingSnapshot {
            revision: state.revision,
            phase: PairingPhase::Paired,
            device_id: Some(device_id.clone()),
            pairing_code: None,
            expires_at_unix_seconds: None,
        },
    }
}

fn normalize_platform_url(input: &str) -> Result<String, PairingIpcError> {
    if input.len() > 2048 || input.chars().any(char::is_whitespace) {
        return Err(PairingIpcError::InvalidRequest("invalid platform URL"));
    }
    let mut parsed =
        Url::parse(input).map_err(|_| PairingIpcError::InvalidRequest("invalid platform URL"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(PairingIpcError::InvalidRequest("invalid platform URL"));
    }
    parsed.set_path("");
    Ok(parsed.as_str().trim_end_matches('/').to_owned())
}

fn validate_created_session(
    created: &PairingSessionCreated,
    now_unix_seconds: i64,
) -> Result<(), PairingIpcError> {
    if !is_bounded_identifier(&created.session_id, 128)
        || created.pairing_code.len() != 8
        || !created
            .pairing_code
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || created.claim_secret.len() != CLAIM_SECRET_LENGTH
        || !created.claim_secret.is_ascii()
        || !created.claim_secret.starts_with("claim_")
        || created.expires_at <= now_unix_seconds
        || created.expires_at > now_unix_seconds.saturating_add(600)
    {
        return Err(PairingIpcError::InvalidPlatformResponse);
    }
    Ok(())
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn validate_claimed_credential(
    claimed: &ClaimedDeviceCredential,
    expected_device_id: &str,
) -> Result<(), PairingIpcError> {
    if claimed.device_id != expected_device_id
        || !is_bounded_identifier(&claimed.tenant_id, 128)
        || !is_bounded_identifier(&claimed.credential_id, 128)
    {
        return Err(PairingIpcError::InvalidPlatformResponse);
    }
    validate_credential(&claimed.credential)
}

fn validate_credential(credential: &str) -> Result<(), PairingIpcError> {
    if credential.len() != DEVICE_CREDENTIAL_LENGTH
        || !credential.is_ascii()
        || !credential.starts_with("desktop_")
    {
        return Err(PairingIpcError::InvalidPlatformResponse);
    }
    Ok(())
}

fn validate_device_id(device_id: &str) -> Result<(), PairingIpcError> {
    if !is_bounded_identifier(device_id, 128) {
        return Err(PairingIpcError::InvalidRequest("invalid device identifier"));
    }
    Ok(())
}

fn is_bounded_identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

impl From<IdentityError> for PairingIpcError {
    fn from(_: IdentityError) -> Self {
        Self::SecureStorageUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryCredentialStore(Mutex<Option<String>>);

    impl DeviceCredentialStore for MemoryCredentialStore {
        fn load(&self) -> Result<Option<String>, IdentityError> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn store(&self, credential: &str) -> Result<(), IdentityError> {
            *self.0.lock().unwrap() = Some(credential.to_owned());
            Ok(())
        }

        fn delete(&self) -> Result<(), IdentityError> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    fn created() -> PairingSessionCreated {
        PairingSessionCreated {
            session_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            pairing_code: "12AB34CD".to_owned(),
            claim_secret: format!("claim_{}", "a".repeat(65)),
            expires_at: 1_300,
        }
    }

    fn claimed() -> ClaimedDeviceCredential {
        ClaimedDeviceCredential {
            device_id: "desktop-device-1".to_owned(),
            tenant_id: "tenant-1".to_owned(),
            credential_id: "credential-1".to_owned(),
            credential: format!("desktop_{}", "b".repeat(65)),
        }
    }

    #[test]
    fn platform_url_requires_a_clean_https_origin() {
        assert_eq!(
            StartPairingInput {
                platform_url: "https://platform.example/".to_owned(),
                display_name: " Workstation 7 ".to_owned(),
            }
            .validate()
            .unwrap(),
            ValidatedPairingInput {
                platform_url: "https://platform.example".to_owned(),
                display_name: "Workstation 7".to_owned(),
            }
        );
        for invalid in [
            "http://platform.example",
            "https://user@platform.example",
            "https://platform.example/path",
            "https://platform.example?token=secret",
        ] {
            assert!(StartPairingInput {
                platform_url: invalid.to_owned(),
                display_name: "Device".to_owned(),
            }
            .validate()
            .is_err());
        }
    }

    #[test]
    fn claim_secret_never_appears_in_snapshot_or_debug_output() {
        let controller = PairingController::default();
        let secret = created().claim_secret;
        controller
            .begin(
                "https://platform.example".to_owned(),
                "desktop-device-1".to_owned(),
                created(),
                1_000,
            )
            .unwrap();

        let snapshot = controller.snapshot().unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains(&secret));
        assert_eq!(snapshot.pairing_code.as_deref(), Some("12AB34CD"));
    }

    #[test]
    fn paired_state_is_reported_only_after_secure_persistence() {
        let controller = PairingController::default();
        let store = MemoryCredentialStore::default();
        controller
            .begin(
                "https://platform.example".to_owned(),
                "desktop-device-1".to_owned(),
                created(),
                1_000,
            )
            .unwrap();

        let snapshot = controller.complete(claimed(), &store, 1_100).unwrap();
        assert_eq!(snapshot.phase, PairingPhase::Paired);
        let persisted = store.load().unwrap().unwrap();
        assert!(persisted.contains("https://platform.example"));
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("desktop_b"));

        let connection = controller.connection_secret(&store).unwrap().unwrap();
        assert_eq!(connection.platform_url, "https://platform.example");
        assert_eq!(connection.device_id, "desktop-device-1");
        assert_eq!(connection.credential, format!("desktop_{}", "b".repeat(65)));

        let restored = PairingController::default();
        assert_eq!(
            restored.restore(&store).unwrap().phase,
            PairingPhase::Paired
        );
        assert_eq!(
            restored.forget(&store).unwrap().phase,
            PairingPhase::Unpaired
        );
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn mismatched_or_expired_platform_responses_fail_closed() {
        let controller = PairingController::default();
        assert_eq!(
            controller.begin(
                "https://platform.example".to_owned(),
                "desktop-device-1".to_owned(),
                created(),
                1_301,
            ),
            Err(PairingIpcError::InvalidPlatformResponse)
        );

        controller
            .begin(
                "https://platform.example".to_owned(),
                "desktop-device-1".to_owned(),
                created(),
                1_000,
            )
            .unwrap();
        let mut wrong = claimed();
        wrong.device_id = "different-device".to_owned();
        assert_eq!(
            controller.complete(wrong, &MemoryCredentialStore::default(), 1_100),
            Err(PairingIpcError::InvalidPlatformResponse)
        );
    }
}
