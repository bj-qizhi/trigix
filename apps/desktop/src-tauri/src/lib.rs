use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

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
    use super::{ShellController, ShellIpcError, ShellSnapshot, StopAccepted, StopRequest};
    use tauri::State;

    #[tauri::command]
    fn shell_status(
        controller: State<'_, ShellController>,
    ) -> Result<ShellSnapshot, ShellIpcError> {
        controller.snapshot()
    }

    #[tauri::command]
    fn request_automation_stop(
        controller: State<'_, ShellController>,
        request: StopRequest,
    ) -> Result<StopAccepted, ShellIpcError> {
        controller.request_stop(request)
    }

    pub fn run() {
        tauri::Builder::default()
            .manage(ShellController::default())
            .invoke_handler(tauri::generate_handler![
                shell_status,
                request_automation_stop
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
            serde_json::json!(["allow-shell-status", "allow-request-automation-stop"])
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
}
