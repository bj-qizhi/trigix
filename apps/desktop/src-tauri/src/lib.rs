use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
mod command_runtime;
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
mod connection;
mod pairing;

pub use pairing::{
    PairingController, PairingIpcError, PairingPhase, PairingSessionCreated, PairingSnapshot,
    StartPairingInput,
};

const MAX_REQUEST_ID_LENGTH: usize = 96;
const REPLAY_WINDOW_SIZE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Offline,
    Connecting,
    Online,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationState {
    Idle,
    Running,
    AwaitingApproval,
    Stopping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationHostState {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShellSnapshot {
    pub revision: u64,
    pub connection: ConnectionState,
    pub automation: AutomationState,
    pub automation_host: AutomationHostState,
    pub can_request_stop: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopRequest {
    pub request_id: String,
    pub observed_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StopAccepted {
    pub request_id: String,
    pub revision: u64,
    pub automation: AutomationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum ShellIpcError {
    InvalidRequest(&'static str),
    StaleState,
    ReplayDetected,
    NoActiveAutomation,
    StateUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealtimeVoiceBootstrap {
    pub schema_version: String,
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub client_secret: String,
    pub client_secret_expires_at_unix_seconds: u64,
    pub session_expires_at_unix_seconds: u64,
    pub calls_url: String,
    pub policy_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalVoiceTranscriptInput {
    pub session_id: String,
    pub sequence: u32,
    pub occurred_at_unix_ms: u64,
    pub transcript: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceTelemetryInput {
    pub session_id: String,
    pub event: String,
    pub duration_ms: Option<u32>,
    pub attempt: Option<u8>,
    pub failure_category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmRealtimeVoiceInput {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum VoiceIpcError {
    NotPaired,
    TransportUnavailable,
    CredentialRejected,
    ServiceUnavailable,
    InvalidPlatformResponse,
    InvalidRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvatarQualificationInput {
    pub startup_ms: u32,
    pub frame_time_p95_micros: u32,
    pub memory_bytes: u64,
    pub dropped_frame_percent: u8,
    pub resize_recovered: bool,
    pub device_loss_recovered: bool,
    pub background_suspended: bool,
    pub interruption_recovered: bool,
    pub long_session_minutes: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AvatarIpcError {
    QualificationFailed,
}

struct ShellState {
    revision: u64,
    connection: ConnectionState,
    automation: AutomationState,
    automation_host: AutomationHostState,
    seen_requests: HashSet<String>,
    request_order: VecDeque<String>,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            revision: 1,
            connection: ConnectionState::Offline,
            automation: AutomationState::Idle,
            automation_host: AutomationHostState::Unavailable,
            seen_requests: HashSet::new(),
            request_order: VecDeque::new(),
        }
    }
}

pub struct ShellController {
    state: Mutex<ShellState>,
    stop_requested: AtomicBool,
}

impl Default for ShellController {
    fn default() -> Self {
        Self {
            state: Mutex::new(ShellState::default()),
            stop_requested: AtomicBool::new(false),
        }
    }
}

impl ShellController {
    pub fn snapshot(&self) -> Result<ShellSnapshot, ShellIpcError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ShellIpcError::StateUnavailable)?;
        Ok(snapshot_from(&state))
    }

    pub fn request_stop(&self, request: StopRequest) -> Result<StopAccepted, ShellIpcError> {
        validate_request_id(&request.request_id)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ShellIpcError::StateUnavailable)?;

        if state.seen_requests.contains(&request.request_id) {
            return Err(ShellIpcError::ReplayDetected);
        }
        if request.observed_revision != state.revision {
            return Err(ShellIpcError::StaleState);
        }
        if !matches!(
            state.automation,
            AutomationState::Running | AutomationState::AwaitingApproval
        ) {
            return Err(ShellIpcError::NoActiveAutomation);
        }

        remember_request(&mut state, request.request_id.clone());
        state.revision = state.revision.saturating_add(1);
        state.automation = AutomationState::Stopping;
        self.stop_requested.store(true, Ordering::Release);

        Ok(StopAccepted {
            request_id: request.request_id,
            revision: state.revision,
            automation: state.automation,
        })
    }

    pub fn take_stop_request(&self) -> bool {
        self.stop_requested.swap(false, Ordering::AcqRel)
    }

    pub fn update_runtime(
        &self,
        connection: ConnectionState,
        automation: AutomationState,
    ) -> Result<ShellSnapshot, ShellIpcError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ShellIpcError::StateUnavailable)?;
        if state.connection != connection || state.automation != automation {
            state.revision = state.revision.saturating_add(1);
            state.connection = connection;
            state.automation = automation;
        }
        Ok(snapshot_from(&state))
    }

    pub fn update_connection(
        &self,
        connection: ConnectionState,
    ) -> Result<ShellSnapshot, ShellIpcError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ShellIpcError::StateUnavailable)?;
        if state.connection != connection {
            state.revision = state.revision.saturating_add(1);
            state.connection = connection;
        }
        Ok(snapshot_from(&state))
    }

    pub fn update_automation_host(
        &self,
        automation_host: AutomationHostState,
    ) -> Result<ShellSnapshot, ShellIpcError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ShellIpcError::StateUnavailable)?;
        if state.automation_host != automation_host {
            state.revision = state.revision.saturating_add(1);
            state.automation_host = automation_host;
        }
        Ok(snapshot_from(&state))
    }
}

fn snapshot_from(state: &ShellState) -> ShellSnapshot {
    ShellSnapshot {
        revision: state.revision,
        connection: state.connection,
        automation: state.automation,
        automation_host: state.automation_host,
        can_request_stop: matches!(
            state.automation,
            AutomationState::Running | AutomationState::AwaitingApproval
        ),
    }
}

fn validate_request_id(request_id: &str) -> Result<(), ShellIpcError> {
    if request_id.is_empty()
        || request_id.len() > MAX_REQUEST_ID_LENGTH
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ShellIpcError::InvalidRequest("invalid request identifier"));
    }
    Ok(())
}

fn remember_request(state: &mut ShellState, request_id: String) {
    if state.request_order.len() == REPLAY_WINDOW_SIZE {
        if let Some(expired) = state.request_order.pop_front() {
            state.seen_requests.remove(&expired);
        }
    }
    state.seen_requests.insert(request_id.clone());
    state.request_order.push_back(request_id);
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native {
    use super::command_runtime::CommandRuntime;
    use super::connection::{
        parse_cancellation_event, parse_command_event, reconnect_delay, ConnectedEvent,
        ConnectionError, SseDecoder, SseEvent,
    };
    use super::pairing::ClaimedDeviceCredential;
    use super::{
        AutomationHostState, AutomationState, ConfirmRealtimeVoiceInput, ConnectionState,
        FinalVoiceTranscriptInput, PairingController, PairingIpcError, PairingSessionCreated,
        PairingSnapshot, RealtimeVoiceBootstrap, ShellController, ShellIpcError, ShellSnapshot,
        StartPairingInput, StopAccepted, StopRequest, VoiceIpcError, VoiceTelemetryInput,
    };
    use desktop_identity::{DeviceIdentity, NativeCredentialStore, NativeDeviceCredentialStore};
    use desktop_protocol::{
        CommandOutcome, DesktopCommandAcknowledgement, DeviceCapability, DeviceDescriptor,
        DeviceState, Envelope, Heartbeat,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tauri::{Manager, State};
    use trigix_desktop_automation::AutomationPermissionSnapshot;

    static HEARTBEAT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
    static VOICE_QUALIFIED: AtomicBool = AtomicBool::new(false);
    static AVATAR_QUALIFIED: AtomicBool = AtomicBool::new(false);
    static PENDING_VOICE_BOOTSTRAP: Mutex<Option<(String, u64)>> = Mutex::new(None);

    fn qualified_capabilities(command: Option<&CommandRuntime>) -> Vec<DeviceCapability> {
        let mut capabilities = command
            .map(CommandRuntime::capabilities)
            .unwrap_or_default();
        if VOICE_QUALIFIED.load(Ordering::Acquire)
            && !capabilities.contains(&DeviceCapability::VoiceConversation)
        {
            capabilities.push(DeviceCapability::VoiceConversation);
        }
        if AVATAR_QUALIFIED.load(Ordering::Acquire)
            && !capabilities.contains(&DeviceCapability::AvatarRendering)
        {
            capabilities.push(DeviceCapability::AvatarRendering);
        }
        capabilities
    }

    struct NativeRuntimeState {
        command: Option<Arc<CommandRuntime>>,
    }

    impl NativeRuntimeState {
        fn capabilities(&self) -> Vec<DeviceCapability> {
            qualified_capabilities(self.command.as_deref())
        }
    }

    #[tauri::command]
    fn shell_status(
        controller: State<'_, Arc<ShellController>>,
    ) -> Result<ShellSnapshot, ShellIpcError> {
        controller.snapshot()
    }

    #[tauri::command]
    fn automation_permission_status() -> AutomationPermissionSnapshot {
        trigix_desktop_automation::automation_permission_status()
    }

    #[tauri::command]
    fn request_automation_permission() -> AutomationPermissionSnapshot {
        #[cfg(target_os = "macos")]
        let _ = trigix_desktop_automation::request_macos_accessibility();
        trigix_desktop_automation::automation_permission_status()
    }

    #[tauri::command]
    fn request_automation_stop(
        controller: State<'_, Arc<ShellController>>,
        runtime: State<'_, NativeRuntimeState>,
        request: StopRequest,
    ) -> Result<StopAccepted, ShellIpcError> {
        let accepted = controller.request_stop(request)?;
        if let Some(command) = runtime.command.as_deref() {
            command.cancel_active();
        }
        Ok(accepted)
    }

    #[tauri::command]
    fn pairing_status(
        controller: State<'_, Arc<PairingController>>,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        controller.snapshot()
    }

    #[tauri::command(async)]
    async fn start_device_pairing(
        controller: State<'_, Arc<PairingController>>,
        runtime: State<'_, NativeRuntimeState>,
        input: StartPairingInput,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        let input = input.validate()?;
        VOICE_QUALIFIED.store(false, Ordering::Release);
        AVATAR_QUALIFIED.store(false, Ordering::Release);
        if let Ok(mut pending) = PENDING_VOICE_BOOTSTRAP.lock() {
            *pending = None;
        }
        let identity_store = NativeCredentialStore::new("primary-device");
        let identity = DeviceIdentity::load_or_create(&identity_store)?;
        let device_id = identity.device_id();
        let request = serde_json::json!({
            "device": DeviceDescriptor {
                device_id: device_id.clone(),
                display_name: input.display_name,
                operating_system: std::env::consts::OS.to_owned(),
                agent_version: env!("CARGO_PKG_VERSION").to_owned(),
                capabilities: runtime.capabilities(),
            },
            "device_public_key": identity.public_key(),
        });
        let response = pairing_client()?
            .post(format!(
                "{}/v1/desktop/pairing-sessions",
                input.platform_url
            ))
            .json(&request)
            .send()
            .await
            .map_err(|_| PairingIpcError::TransportUnavailable)?;
        if response.status() != reqwest::StatusCode::CREATED {
            return Err(PairingIpcError::TransportUnavailable);
        }
        let created = response
            .json::<PairingSessionCreated>()
            .await
            .map_err(|_| PairingIpcError::InvalidPlatformResponse)?;
        controller.begin(input.platform_url, device_id, created, unix_seconds())
    }

    #[tauri::command(async)]
    async fn complete_device_pairing(
        controller: State<'_, Arc<PairingController>>,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        let pending = controller.pending_claim(unix_seconds())?;
        let response = pairing_client()?
            .post(format!(
                "{}/v1/desktop/pairing-sessions/{}/claim",
                pending.platform_url, pending.session_id
            ))
            .json(&serde_json::json!({ "claim_secret": pending.claim_secret }))
            .send()
            .await
            .map_err(|_| PairingIpcError::TransportUnavailable)?;
        if !response.status().is_success() {
            return Err(PairingIpcError::TransportUnavailable);
        }
        let claimed = response
            .json::<ClaimedDeviceCredential>()
            .await
            .map_err(|_| PairingIpcError::InvalidPlatformResponse)?;
        let store = NativeDeviceCredentialStore::new(&pending.device_id)?;
        controller.complete(claimed, &store, unix_seconds())
    }

    #[tauri::command(async)]
    fn forget_device_pairing(
        controller: State<'_, Arc<PairingController>>,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        let device_id = controller
            .snapshot()?
            .device_id
            .ok_or(PairingIpcError::InvalidState)?;
        let store = NativeDeviceCredentialStore::new(&device_id)?;
        let snapshot = controller.forget(&store)?;
        VOICE_QUALIFIED.store(false, Ordering::Release);
        AVATAR_QUALIFIED.store(false, Ordering::Release);
        if let Ok(mut pending) = PENDING_VOICE_BOOTSTRAP.lock() {
            *pending = None;
        }
        Ok(snapshot)
    }

    fn pairing_client() -> Result<reqwest::Client, PairingIpcError> {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|_| PairingIpcError::TransportUnavailable)
    }

    fn unix_seconds() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    fn unix_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn connection_client() -> Result<reqwest::Client, ConnectionError> {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|_| ConnectionError::Transport)
    }

    fn voice_client() -> Result<reqwest::Client, VoiceIpcError> {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|_| VoiceIpcError::TransportUnavailable)
    }

    fn voice_secret(
        pairing: &PairingController,
    ) -> Result<super::pairing::ConnectionSecret, VoiceIpcError> {
        let snapshot = pairing.snapshot().map_err(|_| VoiceIpcError::NotPaired)?;
        let device_id = snapshot.device_id.ok_or(VoiceIpcError::NotPaired)?;
        let store =
            NativeDeviceCredentialStore::new(&device_id).map_err(|_| VoiceIpcError::NotPaired)?;
        pairing
            .connection_secret(&store)
            .map_err(|_| VoiceIpcError::NotPaired)?
            .ok_or(VoiceIpcError::NotPaired)
    }

    fn map_voice_status(status: reqwest::StatusCode) -> VoiceIpcError {
        match status.as_u16() {
            401 | 403 | 410 => VoiceIpcError::CredentialRejected,
            400 | 409 | 422 => VoiceIpcError::InvalidRequest,
            503 => VoiceIpcError::ServiceUnavailable,
            _ => VoiceIpcError::TransportUnavailable,
        }
    }

    #[tauri::command(async)]
    async fn bootstrap_realtime_voice(
        pairing: State<'_, Arc<PairingController>>,
    ) -> Result<RealtimeVoiceBootstrap, VoiceIpcError> {
        let secret = voice_secret(&pairing)?;
        let response = voice_client()?
            .post(format!(
                "{}/v1/desktop/voice/realtime-sessions",
                secret.platform_url
            ))
            .header("x-device-id", &secret.device_id)
            .header("authorization", format!("Device {}", secret.credential))
            .send()
            .await
            .map_err(|_| VoiceIpcError::TransportUnavailable)?;
        if !response.status().is_success() {
            return Err(map_voice_status(response.status()));
        }
        let bootstrap = response
            .json::<RealtimeVoiceBootstrap>()
            .await
            .map_err(|_| VoiceIpcError::InvalidPlatformResponse)?;
        if bootstrap.schema_version != "realtime-voice-bootstrap-v1"
            || bootstrap.provider != "openai"
            || !bootstrap.client_secret.starts_with("ek_")
            || bootstrap.client_secret.len() > 2_048
            || bootstrap.client_secret_expires_at_unix_seconds <= unix_seconds() as u64
            || bootstrap.session_expires_at_unix_seconds
                <= bootstrap.client_secret_expires_at_unix_seconds
            || bootstrap.session_expires_at_unix_seconds > unix_seconds() as u64 + 3_600
            || bootstrap.calls_url != "https://api.openai.com/v1/realtime/calls"
        {
            return Err(VoiceIpcError::InvalidPlatformResponse);
        }
        let mut pending = PENDING_VOICE_BOOTSTRAP
            .lock()
            .map_err(|_| VoiceIpcError::InvalidPlatformResponse)?;
        *pending = Some((
            bootstrap.session_id.clone(),
            bootstrap.session_expires_at_unix_seconds,
        ));
        Ok(bootstrap)
    }

    #[tauri::command]
    fn confirm_realtime_voice_connected(
        input: ConfirmRealtimeVoiceInput,
    ) -> Result<(), VoiceIpcError> {
        let pending = PENDING_VOICE_BOOTSTRAP
            .lock()
            .map_err(|_| VoiceIpcError::InvalidPlatformResponse)?;
        let valid = pending.as_ref().is_some_and(|(session_id, expires_at)| {
            session_id == &input.session_id && *expires_at > unix_seconds() as u64
        });
        if !valid {
            return Err(VoiceIpcError::InvalidRequest);
        }
        VOICE_QUALIFIED.store(true, Ordering::Release);
        Ok(())
    }

    #[tauri::command]
    fn confirm_avatar_renderer_qualified(
        input: super::AvatarQualificationInput,
    ) -> Result<(), super::AvatarIpcError> {
        let metrics = desktop_avatar::RendererMetrics {
            startup_ms: input.startup_ms,
            frame_time_p95_micros: input.frame_time_p95_micros,
            memory_bytes: input.memory_bytes,
            dropped_frame_percent: input.dropped_frame_percent,
            resize_recovered: input.resize_recovered,
            device_loss_recovered: input.device_loss_recovered,
            background_suspended: input.background_suspended,
            interruption_recovered: input.interruption_recovered,
            long_session_minutes: input.long_session_minutes,
        };
        if !desktop_avatar::qualify_builtin_renderer(metrics) {
            AVATAR_QUALIFIED.store(false, Ordering::Release);
            return Err(super::AvatarIpcError::QualificationFailed);
        }
        AVATAR_QUALIFIED.store(true, Ordering::Release);
        Ok(())
    }

    #[tauri::command(async)]
    async fn accept_final_voice_transcript(
        pairing: State<'_, Arc<PairingController>>,
        input: FinalVoiceTranscriptInput,
    ) -> Result<(), VoiceIpcError> {
        if input.sequence == 0
            || input.session_id.is_empty()
            || input.session_id.len() > 128
            || input.transcript.trim().is_empty()
            || input.transcript.len() > 16_384
            || input.transcript.chars().any(|character| character == '\0')
        {
            return Err(VoiceIpcError::InvalidRequest);
        }
        let secret = voice_secret(&pairing)?;
        let response = voice_client()?
            .post(format!(
                "{}/v1/desktop/voice/final-transcripts",
                secret.platform_url
            ))
            .header("x-device-id", &secret.device_id)
            .header("authorization", format!("Device {}", secret.credential))
            .json(&serde_json::json!({
                "session_id": input.session_id,
                "sequence": input.sequence,
                "occurred_at_unix_ms": input.occurred_at_unix_ms,
                "transcript": input.transcript,
            }))
            .send()
            .await
            .map_err(|_| VoiceIpcError::TransportUnavailable)?;
        if !response.status().is_success() {
            return Err(map_voice_status(response.status()));
        }
        Ok(())
    }

    #[tauri::command(async)]
    async fn record_voice_telemetry(
        pairing: State<'_, Arc<PairingController>>,
        input: VoiceTelemetryInput,
    ) -> Result<(), VoiceIpcError> {
        let valid_event = matches!(
            input.event.as_str(),
            "session_connected" | "reconnect_scheduled" | "interruption" | "failure" | "stopped"
        );
        let valid_failure = input.failure_category.as_deref().is_none_or(|category| {
            matches!(
                category,
                "network_unavailable"
                    | "provider_timeout"
                    | "session_expired"
                    | "device_revoked"
                    | "transcript_rejected"
            )
        });
        if !valid_event
            || input.session_id.is_empty()
            || input.session_id.len() > 128
            || input
                .duration_ms
                .is_some_and(|duration| duration > 3_600_000)
            || input
                .attempt
                .is_some_and(|attempt| !(1..=5).contains(&attempt))
            || !valid_failure
            || (input.event == "reconnect_scheduled") != input.attempt.is_some()
            || (input.event == "failure") != input.failure_category.is_some()
        {
            return Err(VoiceIpcError::InvalidRequest);
        }
        let secret = voice_secret(&pairing)?;
        let response = voice_client()?
            .post(format!(
                "{}/v1/desktop/voice/telemetry",
                secret.platform_url
            ))
            .header("x-device-id", &secret.device_id)
            .header("authorization", format!("Device {}", secret.credential))
            .json(&input)
            .send()
            .await
            .map_err(|_| VoiceIpcError::TransportUnavailable)?;
        if !response.status().is_success() {
            return Err(map_voice_status(response.status()));
        }
        Ok(())
    }

    fn load_connection_secret(
        pairing: &PairingController,
    ) -> Result<Option<super::pairing::ConnectionSecret>, ConnectionError> {
        let snapshot = pairing.snapshot().map_err(|_| ConnectionError::Transport)?;
        let Some(device_id) = snapshot.device_id else {
            return Ok(None);
        };
        let store =
            NativeDeviceCredentialStore::new(&device_id).map_err(|_| ConnectionError::Transport)?;
        pairing
            .connection_secret(&store)
            .map_err(|_| ConnectionError::Transport)
    }

    fn spawn_connection_runtime(
        shell: Arc<ShellController>,
        pairing: Arc<PairingController>,
        command: Option<Arc<CommandRuntime>>,
    ) {
        let thread_shell = shell.clone();
        if std::thread::Builder::new()
            .name("trigix-device-connection".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(1)
                    .build();
                match runtime {
                    Ok(runtime) => {
                        runtime.block_on(connection_loop(thread_shell, pairing, command))
                    }
                    Err(_) => {
                        let _ = thread_shell.update_connection(ConnectionState::Degraded);
                    }
                }
            })
            .is_err()
        {
            let _ = shell.update_connection(ConnectionState::Degraded);
        }
    }

    async fn connection_loop(
        shell: Arc<ShellController>,
        pairing: Arc<PairingController>,
        command: Option<Arc<CommandRuntime>>,
    ) {
        let client = match connection_client() {
            Ok(client) => client,
            Err(_) => {
                let _ = shell.update_connection(ConnectionState::Degraded);
                return;
            }
        };
        let mut attempt = 0_u32;
        loop {
            let secret = match load_connection_secret(&pairing) {
                Ok(Some(secret)) => secret,
                Ok(None) => {
                    attempt = 0;
                    let _ = shell.update_connection(ConnectionState::Offline);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
                Err(_) => {
                    let _ = shell.update_connection(ConnectionState::Degraded);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };
            let _ = shell.update_connection(ConnectionState::Connecting);
            let (error, established) =
                connection_once(&client, &secret, &shell, &pairing, command.clone()).await;
            if let Some(command) = command.as_deref() {
                command.cancel_active();
            }
            let _ = shell.update_runtime(ConnectionState::Degraded, AutomationState::Idle);
            if error == ConnectionError::Unpaired {
                attempt = 0;
                let _ = shell.update_connection(ConnectionState::Offline);
                continue;
            }
            let _ = shell.update_connection(ConnectionState::Degraded);
            if established {
                attempt = 0;
            }
            let delay = if error == ConnectionError::CredentialRejected {
                Duration::from_secs(60)
            } else {
                let delay = reconnect_delay(attempt, unix_millis());
                attempt = attempt.saturating_add(1);
                delay
            };
            tokio::time::sleep(delay).await;
        }
    }

    async fn connection_once(
        client: &reqwest::Client,
        secret: &super::pairing::ConnectionSecret,
        shell: &ShellController,
        pairing: &PairingController,
        command: Option<Arc<CommandRuntime>>,
    ) -> (ConnectionError, bool) {
        let response = match client
            .get(format!(
                "{}/v1/desktop/device-connection",
                secret.platform_url
            ))
            .header("x-device-id", &secret.device_id)
            .header("authorization", format!("Device {}", secret.credential))
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => return (ConnectionError::Transport, false),
        };
        if matches!(response.status().as_u16(), 401 | 403) {
            return (ConnectionError::CredentialRejected, false);
        }
        if !response.status().is_success() {
            return (ConnectionError::Transport, false);
        }

        let mut response = response;
        let mut decoder = SseDecoder::default();
        let mut session_id = None;
        let mut heartbeat_interval = Duration::from_secs(30);
        let mut next_heartbeat = tokio::time::Instant::now() + heartbeat_interval;
        let mut next_pairing_check = tokio::time::Instant::now() + Duration::from_secs(1);
        let mut next_stop_check = tokio::time::Instant::now() + Duration::from_millis(100);
        let (result_sender, mut result_receiver) = tokio::sync::mpsc::channel(1);
        let mut active_command = None;
        loop {
            tokio::select! {
                chunk = response.chunk() => {
                    let chunk = match chunk {
                        Ok(Some(chunk)) => chunk,
                        _ => return (ConnectionError::Transport, session_id.is_some()),
                    };
                    let events = match decoder.push(&chunk) {
                        Ok(events) => events,
                        Err(error) => return (error, session_id.is_some()),
                    };
                    for event in events {
                        if event.event == "command" {
                            let Some(session_id) = session_id.as_deref() else {
                                return (ConnectionError::InvalidStream, false);
                            };
                            let Some(runtime) = command.as_ref() else {
                                return (ConnectionError::UnsupportedCommand, true);
                            };
                            if active_command.is_some() {
                                return (ConnectionError::UnsupportedCommand, true);
                            }
                            let now = unix_millis();
                            let desktop_command = match parse_command_event(&event.data, now) {
                                Ok(command) => command,
                                Err(error) => return (error, true),
                            };
                            if runtime.reserve(&desktop_command).is_err() {
                                return (ConnectionError::UnsupportedCommand, true);
                            }
                            let acknowledgement = Envelope::new(
                                format!("ack-{}", desktop_command.command_id),
                                now,
                                DesktopCommandAcknowledgement {
                                    command_id: desktop_command.command_id.clone(),
                                    execution_id: desktop_command.execution_id.clone(),
                                    lease_id: desktop_command.lease.lease_id.clone(),
                                    acknowledged_at_unix_ms: now,
                                },
                            );
                            if let Err(error) = post_device_message(
                                client,
                                secret,
                                session_id,
                                "device-command-acknowledgements",
                                &acknowledgement,
                            ).await {
                                runtime.abandon(&desktop_command.command_id);
                                return (error, true);
                            }
                            active_command = Some((
                                desktop_command.command_id.clone(),
                                desktop_command.execution_id.clone(),
                            ));
                            let _ = shell.update_runtime(ConnectionState::Online, AutomationState::Running);
                            let runtime = runtime.clone();
                            let sender = result_sender.clone();
                            tokio::task::spawn_blocking(move || {
                                let command_id = desktop_command.command_id.clone();
                                let result = runtime.execute(&desktop_command, unix_millis());
                                let _ = sender.blocking_send((command_id, result));
                            });
                            continue;
                        }
                        if event.event == "command_cancelled" {
                            let Some((active_command_id, active_execution_id)) = active_command.as_ref() else {
                                return (ConnectionError::InvalidStream, true);
                            };
                            let cancellation = match parse_cancellation_event(
                                &event.data,
                                active_command_id,
                                active_execution_id,
                            ) {
                                Ok(cancellation) => cancellation,
                                Err(error) => return (error, true),
                            };
                            if let Some(runtime) = command.as_deref() {
                                runtime.cancel(&cancellation.command_id);
                            }
                            let _ = shell.update_runtime(ConnectionState::Online, AutomationState::Stopping);
                            continue;
                        }
                        match handle_event(event, &secret.device_id, session_id.is_none()) {
                            Ok(Some(connected)) => {
                                heartbeat_interval = Duration::from_secs(u64::from(
                                    connected.heartbeat_interval_seconds,
                                ));
                                next_heartbeat = tokio::time::Instant::now();
                                session_id = Some(connected.session_id);
                                let _ = shell.update_connection(ConnectionState::Online);
                            }
                            Ok(None) => {}
                            Err(error) => return (error, session_id.is_some()),
                        }
                    }
                }
                _ = tokio::time::sleep_until(next_heartbeat), if session_id.is_some() => {
                    let result = post_heartbeat(
                        client,
                        secret,
                        session_id.as_deref().unwrap_or_default(),
                        command.as_deref(),
                    ).await;
                    if let Err(error) = result {
                        return (error, true);
                    }
                    next_heartbeat = tokio::time::Instant::now() + heartbeat_interval;
                }
                _ = tokio::time::sleep_until(next_pairing_check) => {
                    match load_connection_secret(pairing) {
                        Ok(Some(current))
                            if current.device_id == secret.device_id
                                && current.platform_url == secret.platform_url
                                && current.credential == secret.credential => {}
                        Ok(Some(_)) => return (ConnectionError::Transport, session_id.is_some()),
                        Ok(None) => return (ConnectionError::Unpaired, session_id.is_some()),
                        Err(error) => return (error, session_id.is_some()),
                    }
                    next_pairing_check = tokio::time::Instant::now() + Duration::from_secs(1);
                }
                _ = tokio::time::sleep_until(next_stop_check) => {
                    if shell.take_stop_request() {
                        if let Some(runtime) = command.as_deref() {
                            runtime.cancel_active();
                        }
                    }
                    next_stop_check = tokio::time::Instant::now() + Duration::from_millis(100);
                }
                Some((command_id, result)) = result_receiver.recv(), if active_command.is_some() => {
                    if active_command.as_ref().map(|active| active.0.as_str()) != Some(&command_id) {
                        return (ConnectionError::InvalidStream, true);
                    }
                    let result = match result {
                        Ok(result) => result,
                        Err(_) => return (ConnectionError::UnsupportedCommand, true),
                    };
                    if result.outcome == CommandOutcome::AwaitingApproval {
                        let _ = shell.update_runtime(
                            ConnectionState::Online,
                            AutomationState::AwaitingApproval,
                        );
                    }
                    let result_envelope = Envelope::new(
                        format!("result-{command_id}"),
                        unix_millis(),
                        result,
                    );
                    if let Err(error) = post_device_message(
                        client,
                        secret,
                        session_id.as_deref().unwrap_or_default(),
                        "device-command-results",
                        &result_envelope,
                    ).await {
                        return (error, true);
                    }
                    if command
                        .as_deref()
                        .ok_or(ConnectionError::UnsupportedCommand)
                        .and_then(|runtime| runtime.confirm_result_delivery(&result_envelope.payload).map_err(|_| ConnectionError::Transport))
                        .is_err()
                    {
                        return (ConnectionError::Transport, true);
                    }
                    active_command = None;
                    let _ = shell.update_runtime(ConnectionState::Online, AutomationState::Idle);
                }
            }
        }
    }

    fn handle_event(
        event: SseEvent,
        expected_device_id: &str,
        awaiting_connected: bool,
    ) -> Result<Option<ConnectedEvent>, ConnectionError> {
        match event.event.as_str() {
            "connected" if awaiting_connected => {
                let connected: ConnectedEvent = serde_json::from_str(&event.data)
                    .map_err(|_| ConnectionError::InvalidStream)?;
                if connected.device_id != expected_device_id
                    || connected.session_id.is_empty()
                    || connected.session_id.len() > 128
                    || !connected.session_id.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                    })
                    || connected.server_time_unix_ms == 0
                    || !(5..=300).contains(&connected.heartbeat_interval_seconds)
                {
                    return Err(ConnectionError::InvalidStream);
                }
                Ok(Some(connected))
            }
            "connected" | "command" | "command_cancelled" => {
                Err(ConnectionError::UnsupportedCommand)
            }
            "disconnect" => Err(ConnectionError::Transport),
            _ => Err(ConnectionError::InvalidStream),
        }
    }

    async fn post_heartbeat(
        client: &reqwest::Client,
        secret: &super::pairing::ConnectionSecret,
        session_id: &str,
        command: Option<&CommandRuntime>,
    ) -> Result<(), ConnectionError> {
        let now = unix_millis();
        let sequence = HEARTBEAT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let active_execution_id = command.and_then(CommandRuntime::active_execution_id);
        let heartbeat = Envelope::new(
            format!("heartbeat-{now}-{sequence}"),
            now,
            Heartbeat {
                device_id: secret.device_id.clone(),
                state: if active_execution_id.is_some() {
                    DeviceState::Busy
                } else {
                    DeviceState::Online
                },
                active_execution_id,
                agent_version: env!("CARGO_PKG_VERSION").to_owned(),
                capabilities: qualified_capabilities(command),
                health_detail: None,
            },
        );
        let response = client
            .post(format!(
                "{}/v1/desktop/device-heartbeats",
                secret.platform_url
            ))
            .header("x-device-id", &secret.device_id)
            .header("x-device-session-id", session_id)
            .header("authorization", format!("Device {}", secret.credential))
            .json(&heartbeat)
            .send()
            .await
            .map_err(|_| ConnectionError::Transport)?;
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(ConnectionError::CredentialRejected);
        }
        if !response.status().is_success() {
            return Err(ConnectionError::Transport);
        }
        Ok(())
    }

    async fn post_device_message<T: serde::Serialize>(
        client: &reqwest::Client,
        secret: &super::pairing::ConnectionSecret,
        session_id: &str,
        path: &str,
        message: &T,
    ) -> Result<(), ConnectionError> {
        let response = client
            .post(format!("{}/v1/desktop/{path}", secret.platform_url))
            .header("x-device-id", &secret.device_id)
            .header("x-device-session-id", session_id)
            .header("authorization", format!("Device {}", secret.credential))
            .json(message)
            .send()
            .await
            .map_err(|_| ConnectionError::Transport)?;
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(ConnectionError::CredentialRejected);
        }
        if !response.status().is_success() {
            return Err(ConnectionError::Transport);
        }
        Ok(())
    }

    fn create_pairing_controller() -> PairingController {
        let controller = PairingController::default();
        let restored =
            DeviceIdentity::load_or_create(&NativeCredentialStore::new("primary-device"))
                .map_err(PairingIpcError::from)
                .and_then(|identity| {
                    NativeDeviceCredentialStore::new(&identity.device_id())
                        .map_err(PairingIpcError::from)
                })
                .and_then(|store| controller.restore(&store));
        if restored.is_err() {
            controller.mark_unavailable();
        }
        controller
    }

    fn initialize_command_runtime(app: &tauri::App) -> Option<Arc<CommandRuntime>> {
        let executable = std::env::current_exe().ok()?;
        let host_name = if cfg!(target_os = "windows") {
            "desktop-automation-host.exe"
        } else {
            "desktop-automation-host"
        };
        let host_executable = executable.parent()?.join(PathBuf::from(host_name));
        let recovery_path = app
            .path()
            .app_local_data_dir()
            .ok()?
            .join("desktop-command-recovery.json");
        CommandRuntime::initialize(host_executable, recovery_path, unix_millis())
            .ok()
            .map(Arc::new)
    }

    pub fn run() {
        let shell = Arc::new(ShellController::default());
        let pairing = Arc::new(create_pairing_controller());
        let setup_shell = shell.clone();
        let setup_pairing = pairing.clone();
        tauri::Builder::default()
            .manage(shell)
            .manage(pairing)
            .setup(move |app| {
                let command = initialize_command_runtime(app);
                let host_state = if command.is_some() {
                    AutomationHostState::Ready
                } else {
                    AutomationHostState::Unavailable
                };
                let _ = setup_shell.update_automation_host(host_state);
                app.manage(NativeRuntimeState {
                    command: command.clone(),
                });
                spawn_connection_runtime(setup_shell.clone(), setup_pairing.clone(), command);
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                shell_status,
                automation_permission_status,
                request_automation_permission,
                request_automation_stop,
                pairing_status,
                start_device_pairing,
                complete_device_pairing,
                forget_device_pairing,
                bootstrap_realtime_voice,
                accept_final_voice_transcript,
                record_voice_telemetry,
                confirm_realtime_voice_connected,
                confirm_avatar_renderer_qualified
            ])
            .run(tauri::generate_context!())
            .expect("Trigix Desktop runtime failed");
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use native::run;

#[cfg(test)]
mod tests {
    use super::*;

    fn active_controller() -> ShellController {
        let controller = ShellController::default();
        controller
            .update_automation_host(AutomationHostState::Ready)
            .unwrap();
        controller
            .update_runtime(ConnectionState::Online, AutomationState::Running)
            .unwrap();
        controller
    }

    #[test]
    fn snapshot_contains_only_bounded_presentation_state() {
        let controller = active_controller();
        let snapshot = controller.snapshot().unwrap();

        assert_eq!(snapshot.connection, ConnectionState::Online);
        assert_eq!(snapshot.automation, AutomationState::Running);
        assert_eq!(snapshot.automation_host, AutomationHostState::Ready);
        assert!(snapshot.can_request_stop);
    }

    #[test]
    fn stop_request_is_versioned_and_replay_protected() {
        let controller = active_controller();
        let revision = controller.snapshot().unwrap().revision;
        let request = StopRequest {
            request_id: "stop-001".to_owned(),
            observed_revision: revision,
        };

        let accepted = controller.request_stop(request.clone()).unwrap();
        assert_eq!(accepted.automation, AutomationState::Stopping);
        assert!(controller.take_stop_request());
        assert!(!controller.take_stop_request());
        assert_eq!(
            controller.request_stop(request),
            Err(ShellIpcError::ReplayDetected)
        );
    }

    #[test]
    fn stale_or_unbounded_stop_requests_fail_closed() {
        let controller = active_controller();

        assert_eq!(
            controller.request_stop(StopRequest {
                request_id: "stop-stale".to_owned(),
                observed_revision: 1,
            }),
            Err(ShellIpcError::StaleState)
        );
        assert_eq!(
            controller.request_stop(StopRequest {
                request_id: "contains spaces".to_owned(),
                observed_revision: controller.snapshot().unwrap().revision,
            }),
            Err(ShellIpcError::InvalidRequest("invalid request identifier"))
        );
    }

    #[test]
    fn idle_shell_cannot_manufacture_a_stop_side_effect() {
        let controller = ShellController::default();
        assert_eq!(
            controller.request_stop(StopRequest {
                request_id: "stop-idle".to_owned(),
                observed_revision: 1,
            }),
            Err(ShellIpcError::NoActiveAutomation)
        );
        assert!(!controller.take_stop_request());
    }

    #[test]
    fn tauri_capability_stays_local_and_least_privilege() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main-shell.json")).unwrap();

        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert_eq!(
            capability["platforms"],
            serde_json::json!(["windows", "macOS"])
        );
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "allow-shell-status",
                "allow-automation-permission-status",
                "allow-request-automation-permission",
                "allow-request-automation-stop",
                "allow-pairing-status",
                "allow-start-device-pairing",
                "allow-complete-device-pairing",
                "allow-forget-device-pairing",
                "allow-bootstrap-realtime-voice",
                "allow-accept-final-voice-transcript",
                "allow-record-voice-telemetry",
                "allow-confirm-realtime-voice-connected",
                "allow-confirm-avatar-renderer-qualified"
            ])
        );
        assert!(capability.get("remote").is_none());
    }

    #[test]
    fn shell_content_security_policy_has_no_remote_or_inline_execution() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let security = &config["app"]["security"];
        let csp = security["csp"].as_str().unwrap();

        assert_eq!(security["capabilities"], serde_json::json!(["main-shell"]));
        assert_eq!(security["freezePrototype"], true);
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("object-src 'none'"));
        assert!(!csp.contains("unsafe-inline"));
        assert!(!csp.contains("unsafe-eval"));
        assert!(csp.contains("connect-src ipc: http://ipc.localhost https://api.openai.com"));
        assert!(!csp.contains("https://*"));

        let bundle = &config["bundle"];
        assert_eq!(bundle["active"], true);
        assert!(bundle.get("targets").is_none());
        assert_eq!(
            bundle["externalBin"],
            serde_json::json!(["binaries/desktop-automation-host"])
        );

        let windows: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.windows.conf.json")).unwrap();
        let macos: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.macos.conf.json")).unwrap();
        assert_eq!(windows["bundle"]["targets"], serde_json::json!(["nsis"]));
        assert_eq!(macos["bundle"]["targets"], serde_json::json!(["dmg"]));
        assert_eq!(macos["bundle"]["macOS"]["minimumSystemVersion"], "15.0");
        assert_eq!(
            macos["bundle"]["macOS"]["entitlements"],
            "Entitlements.plist"
        );
    }

    #[test]
    fn pairing_views_preserve_native_hidden_state() {
        let styles = include_str!("../../ui/styles.css");
        assert!(styles.contains("[hidden]"));
        assert!(styles.contains("display: none !important"));

        let script = include_str!("../../ui/app.js");
        assert!(!script.contains("claim_secret"));
        assert!(!script.contains("claimSecret"));
    }

    #[test]
    fn voice_capture_requires_consent_and_releases_every_local_track() {
        let html = include_str!("../../ui/index.html");
        assert!(html.contains("id=\"start-voice\""));
        assert!(html.contains("id=\"stop-voice\""));
        assert!(html.contains("id=\"voice-status\""));
        assert!(html.contains("role=\"status\""));
        assert!(html.contains("data-state=\"idle\""));

        let script = include_str!("../../ui/app.js");
        let consent_handler = script
            .find("elements.startVoice.addEventListener(\"click\"")
            .expect("explicit microphone consent handler");
        let capture = script.find("navigator.mediaDevices.getUserMedia").unwrap();
        assert!(consent_handler < capture);
        assert_eq!(
            script
                .matches("navigator.mediaDevices.getUserMedia")
                .count(),
            2
        );
        assert!(script.contains("stream.getTracks().forEach((track) => track.stop())"));
        assert!(script.contains("if (voiceStream || voiceRequestPending)"));
        assert!(script.contains("document.hidden"));
        assert!(script.contains("pagehide"));
        assert!(script.contains("beforeunload"));
        assert!(script.contains("echoCancellation: true"));
        assert!(script.contains("noiseSuppression: true"));
        assert!(script.contains("video: false"));
        assert!(script.contains("navigator.mediaDevices.enumerateDevices()"));
        assert!(script.contains("deviceId: { exact: deviceId }"));
        assert!(script.contains("await activateVoiceStream(replacement)"));
        assert!(script.contains("currentStream.getTracks().forEach((track) => track.stop())"));
        assert!(script.contains("getByteTimeDomainData"));
        assert!(script.contains("window.cancelAnimationFrame"));
        assert!(!script.contains("trigix.desktop.voice"));
        assert!(!script.contains("MediaRecorder"));
        assert!(!script.contains("audio_base64"));
        assert!(script.contains("new RTCPeerConnection()"));
        assert!(script.contains("createDataChannel(\"oai-events\")"));
        assert!(script.contains("conversation.item.input_audio_transcription.completed"));
        assert!(script.contains("accept_final_voice_transcript"));
        assert!(script.contains("maximumVoiceReconnectAttempts = 5"));
        assert!(script.contains("new AbortController()"));
        assert!(script.contains("abortController.abort()"));
        assert!(script.contains("peer.connectionState === \"disconnected\""));
        assert!(!script.contains("sender.replaceTrack"));
        assert!(script.contains("voiceDataChannel.close()"));
        assert!(script.contains("voicePeer.close()"));
        assert!(script.contains("window.clearTimeout(voiceReconnectTimer)"));
        assert!(!script.contains("console.log"));
        assert!(!script.contains("localStorage.setItem(\"voice"));
        assert!(script.contains("shell.connection !== \"online\""));
        assert!(script.contains("confirm_realtime_voice_connected"));

        let styles = include_str!("../../ui/styles.css");
        assert!(styles.contains(".voice-status[data-state=\"listening\"]"));
        assert!(styles.contains("prefers-reduced-motion: no-preference"));
        assert!(styles.contains("forced-colors: active"));
    }

    #[test]
    fn avatar_renderer_is_local_bounded_accessible_and_authority_free() {
        let html = include_str!("../../ui/index.html");
        assert!(html.contains("id=\"avatar-stage\""));
        assert!(html.contains("id=\"avatar-enabled\""));
        assert!(html.contains("id=\"avatar-stop\""));
        assert!(html.contains("aria-live=\"polite\""));

        let script = include_str!("../../ui/app.js");
        assert!(script.contains("confirm_avatar_renderer_qualified"));
        assert!(script.contains("trigix.desktop.avatar.preferences.v1"));
        assert!(script.contains("window.requestAnimationFrame"));
        assert!(script.contains("performance.memory"));
        assert!(script.contains("document.hidden"));
        assert!(!script.contains("avatarTranscript"));
        assert!(!script.contains("avatarAudio"));
        assert!(!script.contains("avatarTool"));
        assert!(!script.contains("avatarUrl"));

        let styles = include_str!("../../ui/styles.css");
        assert!(styles.contains(".avatar-stage[data-motion=\"reduced\"]"));
        assert!(styles.contains(".avatar-panel[data-high-contrast=\"true\"]"));
        assert!(styles.contains("prefers-reduced-motion"));
    }

    #[test]
    fn desktop_shell_accessibility_localization_and_recovery_are_bounded() {
        let html = include_str!("../../ui/index.html");
        assert!(html.contains("class=\"skip-link\""));
        assert!(html.contains("role=\"alert\""));
        assert!(html.contains("aria-live=\"polite\""));
        assert!(html.contains("<dialog id=\"forget-confirm\""));
        assert!(html.contains("data-i18n=\"stop_automation\""));

        let script = include_str!("../../ui/app.js");
        assert!(script.contains("trigix.desktop.locale"));
        assert!(script.contains("document.documentElement.lang"));
        assert!(script.contains("showModal()"));
        assert!(script.contains("visibilitychange"));
        assert!(script.contains("document.hidden"));
        assert!(script.contains("zh: {"));
        assert!(script.contains("en: {"));
        assert!(!script.contains("error.message"));
        let (english, chinese_and_runtime) = script.split_once("  zh: {").unwrap();
        let chinese = chinese_and_runtime.split_once("const elements").unwrap().0;
        for attribute in ["data-i18n=\"", "data-i18n-aria=\""] {
            for occurrence in html.split(attribute).skip(1) {
                let key = occurrence.split_once('"').unwrap().0;
                let declaration = format!("    {key}:");
                assert!(english.contains(&declaration), "missing English key {key}");
                assert!(chinese.contains(&declaration), "missing Chinese key {key}");
            }
        }
        for key in [
            "approve_before",
            "runtime_unavailable",
            "pairing_start_error",
            "pairing_claim_error",
            "pairing_forget_error",
            "stop_error",
            "state_microphone_off",
            "state_requesting_permission",
            "state_listening",
            "state_microphone_stopped",
            "state_permission_denied",
            "state_microphone_unavailable",
            "requesting_microphone",
            "microphone_active",
            "microphone_stopped",
            "microphone_hidden_stop",
            "microphone_permission_denied",
            "microphone_unavailable",
            "input_switched",
            "input_switch_error",
            "state_offline",
            "state_connecting",
            "state_online",
            "state_degraded",
            "state_idle",
            "state_running",
            "state_awaiting_approval",
            "state_stopping",
            "state_ready",
            "state_unavailable",
            "state_unpaired",
            "state_waiting_for_approval",
            "state_paired",
        ] {
            let declaration = format!("    {key}:");
            assert!(english.contains(&declaration), "missing English key {key}");
            assert!(chinese.contains(&declaration), "missing Chinese key {key}");
        }

        let styles = include_str!("../../ui/styles.css");
        assert!(styles.contains(":focus-visible"));
        assert!(styles.contains("prefers-reduced-motion: no-preference"));
        assert!(styles.contains("forced-colors: active"));
    }
}
