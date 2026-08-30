use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShellSnapshot {
    pub revision: u64,
    pub connection: ConnectionState,
    pub automation: AutomationState,
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

struct ShellState {
    revision: u64,
    connection: ConnectionState,
    automation: AutomationState,
    seen_requests: HashSet<String>,
    request_order: VecDeque<String>,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            revision: 1,
            connection: ConnectionState::Offline,
            automation: AutomationState::Idle,
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
}

fn snapshot_from(state: &ShellState) -> ShellSnapshot {
    ShellSnapshot {
        revision: state.revision,
        connection: state.connection,
        automation: state.automation,
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

#[cfg(target_os = "windows")]
mod native {
    use super::connection::{
        reconnect_delay, ConnectedEvent, ConnectionError, SseDecoder, SseEvent,
    };
    use super::pairing::ClaimedDeviceCredential;
    use super::{
        ConnectionState, PairingController, PairingIpcError, PairingSessionCreated,
        PairingSnapshot, ShellController, ShellIpcError, ShellSnapshot, StartPairingInput,
        StopAccepted, StopRequest,
    };
    use desktop_identity::{DeviceIdentity, WindowsCredentialStore, WindowsDeviceCredentialStore};
    use desktop_protocol::{DeviceDescriptor, DeviceState, Envelope, Heartbeat};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tauri::State;

    static HEARTBEAT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[tauri::command]
    fn shell_status(
        controller: State<'_, Arc<ShellController>>,
    ) -> Result<ShellSnapshot, ShellIpcError> {
        controller.snapshot()
    }

    #[tauri::command]
    fn request_automation_stop(
        controller: State<'_, Arc<ShellController>>,
        request: StopRequest,
    ) -> Result<StopAccepted, ShellIpcError> {
        controller.request_stop(request)
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
        input: StartPairingInput,
    ) -> Result<PairingSnapshot, PairingIpcError> {
        let input = input.validate()?;
        let identity_store = WindowsCredentialStore::new("primary-device");
        let identity = DeviceIdentity::load_or_create(&identity_store)?;
        let device_id = identity.device_id();
        let request = serde_json::json!({
            "device": DeviceDescriptor {
                device_id: device_id.clone(),
                display_name: input.display_name,
                operating_system: "windows".to_owned(),
                agent_version: env!("CARGO_PKG_VERSION").to_owned(),
                capabilities: vec![],
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
        let store = WindowsDeviceCredentialStore::new(&pending.device_id)?;
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
        let store = WindowsDeviceCredentialStore::new(&device_id)?;
        controller.forget(&store)
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

    fn load_connection_secret(
        pairing: &PairingController,
    ) -> Result<Option<super::pairing::ConnectionSecret>, ConnectionError> {
        let snapshot = pairing.snapshot().map_err(|_| ConnectionError::Transport)?;
        let Some(device_id) = snapshot.device_id else {
            return Ok(None);
        };
        let store = WindowsDeviceCredentialStore::new(&device_id)
            .map_err(|_| ConnectionError::Transport)?;
        pairing
            .connection_secret(&store)
            .map_err(|_| ConnectionError::Transport)
    }

    fn spawn_connection_runtime(shell: Arc<ShellController>, pairing: Arc<PairingController>) {
        let thread_shell = shell.clone();
        if std::thread::Builder::new()
            .name("trigix-device-connection".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(1)
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(connection_loop(thread_shell, pairing)),
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

    async fn connection_loop(shell: Arc<ShellController>, pairing: Arc<PairingController>) {
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
            let (error, established) = connection_once(&client, &secret, &shell, &pairing).await;
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
    ) -> Result<(), ConnectionError> {
        let now = unix_millis();
        let sequence = HEARTBEAT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let heartbeat = Envelope::new(
            format!("heartbeat-{now}-{sequence}"),
            now,
            Heartbeat {
                device_id: secret.device_id.clone(),
                state: DeviceState::Online,
                active_execution_id: None,
                agent_version: env!("CARGO_PKG_VERSION").to_owned(),
                capabilities: vec![],
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

    fn create_pairing_controller() -> PairingController {
        let controller = PairingController::default();
        let restored =
            DeviceIdentity::load_or_create(&WindowsCredentialStore::new("primary-device"))
                .map_err(PairingIpcError::from)
                .and_then(|identity| {
                    WindowsDeviceCredentialStore::new(&identity.device_id())
                        .map_err(PairingIpcError::from)
                })
                .and_then(|store| controller.restore(&store));
        if restored.is_err() {
            controller.mark_unavailable();
        }
        controller
    }

    pub fn run() {
        let shell = Arc::new(ShellController::default());
        let pairing = Arc::new(create_pairing_controller());
        spawn_connection_runtime(shell.clone(), pairing.clone());
        tauri::Builder::default()
            .manage(shell)
            .manage(pairing)
            .invoke_handler(tauri::generate_handler![
                shell_status,
                request_automation_stop,
                pairing_status,
                start_device_pairing,
                complete_device_pairing,
                forget_device_pairing
            ])
            .run(tauri::generate_context!())
            .expect("Trigix Desktop runtime failed");
    }
}

#[cfg(target_os = "windows")]
pub use native::run;

#[cfg(test)]
mod tests {
    use super::*;

    fn active_controller() -> ShellController {
        let controller = ShellController::default();
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
        assert_eq!(capability["platforms"], serde_json::json!(["windows"]));
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "allow-shell-status",
                "allow-request-automation-stop",
                "allow-pairing-status",
                "allow-start-device-pairing",
                "allow-complete-device-pairing",
                "allow-forget-device-pairing"
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
        assert!(!csp.contains("https:"));
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
}
