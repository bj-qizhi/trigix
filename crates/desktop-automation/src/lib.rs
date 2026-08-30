use desktop_protocol::{
    AutomationPattern, DesktopAction, DesktopInspectionRequest, DesktopInspectionResult,
    ElementSelector, InspectedElement, InspectedWindow, ProtocolError, RedactionReason,
    WindowSelector, WindowTitlePolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::io::{self, BufRead, Write};

mod agent_executor;
mod supervisor;

pub use agent_executor::{SupervisedActionExecutor, SupervisedActionExecutorHandle};
pub use supervisor::{AutomationCancellation, AutomationHostSupervisor, SupervisorConfig};

pub const MAX_HOST_MESSAGE_BYTES: u64 = 64 * 1024;
pub const FIXTURE_WINDOW_AUTOMATION_ID: &str = "Trigix.AutomationFixture.Main";
pub const FIXTURE_INPUT_AUTOMATION_ID: &str = "1001";
pub const FIXTURE_SUBMIT_AUTOMATION_ID: &str = "1002";
pub const FIXTURE_PASSWORD_AUTOMATION_ID: &str = "1003";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationHostError {
    Protocol(ProtocolError),
    InvalidRequest(&'static str),
    UnsupportedAction,
    TargetNotFound,
    TargetAmbiguous,
    TargetStale,
    ApplicationNotAllowed,
    AccessDenied,
    LaunchFailed,
    UnsupportedPattern,
    ProtectedControl,
    FocusChanged,
    PartialEntry,
    HostCrashed,
    LeaseExpired,
    DeadlineExpired,
    Adapter(String),
    Io(String),
}

impl fmt::Display for AutomationHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(formatter, "protocol error: {error}"),
            Self::InvalidRequest(field) => write!(formatter, "invalid host request: {field}"),
            Self::UnsupportedAction => formatter.write_str("automation action is unsupported"),
            Self::TargetNotFound => formatter.write_str("automation target was not found"),
            Self::TargetAmbiguous => formatter.write_str("automation target is ambiguous"),
            Self::TargetStale => formatter.write_str("automation target snapshot is stale"),
            Self::ApplicationNotAllowed => {
                formatter.write_str("application is not present in the launch allowlist")
            }
            Self::AccessDenied => formatter.write_str("operating system denied the action"),
            Self::LaunchFailed => formatter.write_str("application launch failed"),
            Self::UnsupportedPattern => {
                formatter.write_str("target does not support the requested semantic pattern")
            }
            Self::ProtectedControl => {
                formatter.write_str("text entry into a protected control is prohibited")
            }
            Self::FocusChanged => formatter.write_str("target window is not in the foreground"),
            Self::PartialEntry => {
                formatter.write_str("text entry could not be verified completely")
            }
            Self::HostCrashed => formatter.write_str("automation host exited without a result"),
            Self::LeaseExpired => formatter.write_str("execution lease expired before side effect"),
            Self::DeadlineExpired => {
                formatter.write_str("automation request deadline expired before side effect")
            }
            Self::Adapter(message) => write!(formatter, "automation adapter failed: {message}"),
            Self::Io(message) => write!(formatter, "automation host I/O failed: {message}"),
        }
    }
}

impl std::error::Error for AutomationHostError {}

impl From<ProtocolError> for AutomationHostError {
    fn from(value: ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

impl From<io::Error> for AutomationHostError {
    fn from(value: io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationHostRequest {
    pub request_id: String,
    pub sent_at_unix_ms: u64,
    pub deadline_unix_ms: u64,
    pub operation: AutomationHostOperation,
}

impl AutomationHostRequest {
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), AutomationHostError> {
        validate_identifier(&self.request_id, "request_id")?;
        if self.deadline_unix_ms <= self.sent_at_unix_ms || self.deadline_unix_ms <= now_unix_ms {
            return Err(AutomationHostError::InvalidRequest("deadline_unix_ms"));
        }
        match &self.operation {
            AutomationHostOperation::Health | AutomationHostOperation::Shutdown => Ok(()),
            AutomationHostOperation::Cancel { target_request_id } => {
                validate_identifier(target_request_id, "target_request_id")
            }
            AutomationHostOperation::Execute {
                command_id,
                lease_id,
                lease_expires_at_unix_ms,
                action,
            } => {
                validate_identifier(command_id, "command_id")?;
                validate_identifier(lease_id, "lease_id")?;
                if *lease_expires_at_unix_ms > self.deadline_unix_ms
                    || *lease_expires_at_unix_ms <= now_unix_ms
                {
                    return Err(AutomationHostError::InvalidRequest(
                        "lease_expires_at_unix_ms",
                    ));
                }
                action.validate()?;
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AutomationHostOperation {
    Health,
    Execute {
        command_id: String,
        lease_id: String,
        lease_expires_at_unix_ms: u64,
        action: Box<DesktopAction>,
    },
    Cancel {
        target_request_id: String,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationHostStatus {
    Ready,
    Succeeded,
    Rejected,
    Cancelled,
    TimedOut,
    Failed,
    ShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationHostResponse {
    pub request_id: String,
    pub status: AutomationHostStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl AutomationHostResponse {
    pub fn validate(&self) -> Result<(), AutomationHostError> {
        validate_identifier(&self.request_id, "request_id")?;
        let failed = matches!(
            self.status,
            AutomationHostStatus::Rejected
                | AutomationHostStatus::Cancelled
                | AutomationHostStatus::TimedOut
                | AutomationHostStatus::Failed
        );
        if failed != self.error_code.is_some() {
            return Err(AutomationHostError::InvalidRequest("error_code"));
        }
        if let Some(code) = &self.error_code {
            validate_text(code, "error_code", 128)?;
        }
        if let Some(message) = &self.error_message {
            validate_text(message, "error_message", 2_048)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationFixtureDescriptor {
    pub application_id: String,
    pub window_automation_id: String,
    pub input_automation_id: String,
    pub submit_automation_id: String,
    pub password_automation_id: String,
}

impl Default for AutomationFixtureDescriptor {
    fn default() -> Self {
        Self {
            application_id: "trigix.automation-fixture".to_owned(),
            window_automation_id: FIXTURE_WINDOW_AUTOMATION_ID.to_owned(),
            input_automation_id: FIXTURE_INPUT_AUTOMATION_ID.to_owned(),
            submit_automation_id: FIXTURE_SUBMIT_AUTOMATION_ID.to_owned(),
            password_automation_id: FIXTURE_PASSWORD_AUTOMATION_ID.to_owned(),
        }
    }
}

pub struct AutomationExecutionGuard<'a> {
    deadline_unix_ms: u64,
    lease_expires_at_unix_ms: u64,
    clock: &'a dyn Fn() -> u64,
}

impl AutomationExecutionGuard<'_> {
    pub fn ensure_active(&self) -> Result<(), AutomationHostError> {
        let now = (self.clock)();
        if now >= self.deadline_unix_ms {
            return Err(AutomationHostError::DeadlineExpired);
        }
        if now >= self.lease_expires_at_unix_ms {
            return Err(AutomationHostError::LeaseExpired);
        }
        Ok(())
    }
}

pub trait AutomationAdapter {
    fn execute(&mut self, action: &DesktopAction) -> Result<Value, AutomationHostError>;

    fn execute_guarded(
        &mut self,
        action: &DesktopAction,
        guard: &AutomationExecutionGuard<'_>,
    ) -> Result<Value, AutomationHostError> {
        guard.ensure_active()?;
        self.execute(action)
    }
}

#[derive(Debug)]
pub struct FixtureAutomationAdapter {
    snapshot_id: String,
    focused_window_id: String,
}

impl Default for FixtureAutomationAdapter {
    fn default() -> Self {
        Self {
            snapshot_id: "fixture-snapshot-1".to_owned(),
            focused_window_id: FIXTURE_WINDOW_AUTOMATION_ID.to_owned(),
        }
    }
}

impl AutomationAdapter for FixtureAutomationAdapter {
    fn execute(&mut self, action: &DesktopAction) -> Result<Value, AutomationHostError> {
        match action {
            DesktopAction::ReadSystemInformation => Ok(json!({
                "adapter": "fixture",
                "platform": std::env::consts::OS,
                "fixture": AutomationFixtureDescriptor::default(),
            })),
            DesktopAction::InspectTargets { request } => {
                serde_json::to_value(self.inspect(request)?)
                    .map_err(|error| AutomationHostError::Adapter(error.to_string()))
            }
            DesktopAction::FocusWindow { selector } => self.focus(selector),
            DesktopAction::LaunchApplication { application_id } => {
                if application_id.0 != "trigix.automation-fixture" {
                    return Err(AutomationHostError::ApplicationNotAllowed);
                }
                Ok(json!({
                    "application_id": application_id.0,
                    "launched": true,
                }))
            }
            DesktopAction::ClickElement { selector } => self.click(selector),
            DesktopAction::TypeText { selector, text } => self.type_text(selector, text),
        }
    }
}

impl FixtureAutomationAdapter {
    fn focus(&mut self, selector: &WindowSelector) -> Result<Value, AutomationHostError> {
        if selector
            .snapshot_id
            .as_deref()
            .is_some_and(|expected| expected != self.snapshot_id)
        {
            return Err(AutomationHostError::TargetStale);
        }
        let matches = fixture_windows()
            .into_iter()
            .filter(|window| window_matches(&window.selector, selector))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(AutomationHostError::TargetNotFound),
            [window] => {
                self.focused_window_id = window
                    .selector
                    .automation_id
                    .clone()
                    .ok_or(AutomationHostError::TargetNotFound)?;
                Ok(json!({
                    "focused": true,
                    "process_id": window.process_id,
                    "selector_strategy": "automation_id",
                }))
            }
            _ => Err(AutomationHostError::TargetAmbiguous),
        }
    }

    fn resolve_element(
        &self,
        selector: &ElementSelector,
    ) -> Result<InspectedElement, AutomationHostError> {
        if selector
            .window
            .snapshot_id
            .as_deref()
            .is_some_and(|expected| expected != self.snapshot_id)
        {
            return Err(AutomationHostError::TargetStale);
        }
        if selector.window.automation_id.as_deref() != Some(&self.focused_window_id) {
            return Err(AutomationHostError::FocusChanged);
        }
        let matches = fixture_windows()
            .into_iter()
            .filter(|window| window_matches(&window.selector, &selector.window))
            .flat_map(|window| window.elements)
            .filter(|element| element_matches(&element.selector, selector))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(AutomationHostError::TargetNotFound),
            [element] => Ok(element.clone()),
            _ => Err(AutomationHostError::TargetAmbiguous),
        }
    }

    fn click(&self, selector: &ElementSelector) -> Result<Value, AutomationHostError> {
        let element = self.resolve_element(selector)?;
        if !element
            .supported_patterns
            .contains(&AutomationPattern::Invoke)
        {
            return Err(AutomationHostError::UnsupportedPattern);
        }
        Ok(json!({
            "clicked": true,
            "semantic_pattern": "invoke",
        }))
    }

    fn type_text(
        &self,
        selector: &ElementSelector,
        text: &str,
    ) -> Result<Value, AutomationHostError> {
        let element = self.resolve_element(selector)?;
        if element.redaction == Some(RedactionReason::Password) {
            return Err(AutomationHostError::ProtectedControl);
        }
        if !element
            .supported_patterns
            .contains(&AutomationPattern::Value)
        {
            return Err(AutomationHostError::UnsupportedPattern);
        }
        Ok(json!({
            "entered": true,
            "characters_entered": text.chars().count(),
            "semantic_pattern": "value",
        }))
    }

    fn inspect(
        &self,
        request: &DesktopInspectionRequest,
    ) -> Result<DesktopInspectionResult, AutomationHostError> {
        if request
            .expected_snapshot_id
            .as_deref()
            .is_some_and(|expected| expected != self.snapshot_id)
        {
            return Err(AutomationHostError::TargetStale);
        }
        let mut windows = fixture_windows();
        if let Some(selector) = &request.window {
            windows.retain(|window| window_matches(&window.selector, selector));
        }
        if windows.is_empty() {
            return Err(AutomationHostError::TargetNotFound);
        }
        attach_snapshot(&mut windows, &self.snapshot_id);
        bound_result(
            DesktopInspectionResult {
                snapshot_id: self.snapshot_id.clone(),
                windows,
                truncated: false,
            },
            request,
        )
    }
}

fn attach_snapshot(windows: &mut [InspectedWindow], snapshot_id: &str) {
    for window in windows {
        window.selector.snapshot_id = Some(snapshot_id.to_owned());
        for element in &mut window.elements {
            element.selector.window.snapshot_id = Some(snapshot_id.to_owned());
        }
    }
}

fn fixture_windows() -> Vec<InspectedWindow> {
    let descriptor = AutomationFixtureDescriptor::default();
    let primary = WindowSelector {
        executable: Some("desktop-automation-fixture.exe".to_owned()),
        title: Some("Trigix 自动化测试".to_owned()),
        automation_id: Some(descriptor.window_automation_id.clone()),
        snapshot_id: None,
    };
    let elements = vec![
        InspectedElement {
            selector: ElementSelector {
                window: primary.clone(),
                automation_id: Some(descriptor.input_automation_id),
                name: Some("名称".to_owned()),
                control_type: Some("edit".to_owned()),
            },
            depth: 1,
            supported_patterns: vec![AutomationPattern::Value, AutomationPattern::Text],
            value: Some("fixture input".to_owned()),
            redaction: None,
        },
        InspectedElement {
            selector: ElementSelector {
                window: primary.clone(),
                automation_id: Some(descriptor.submit_automation_id),
                name: Some("提交".to_owned()),
                control_type: Some("button".to_owned()),
            },
            depth: 1,
            supported_patterns: vec![AutomationPattern::Invoke],
            value: None,
            redaction: None,
        },
        InspectedElement {
            selector: ElementSelector {
                window: primary.clone(),
                automation_id: Some(descriptor.password_automation_id),
                name: Some("密码".to_owned()),
                control_type: Some("edit".to_owned()),
            },
            depth: 1,
            supported_patterns: vec![AutomationPattern::Value],
            value: None,
            redaction: Some(RedactionReason::Password),
        },
    ];
    vec![
        InspectedWindow {
            selector: primary,
            process_id: 4_242,
            title_policy: WindowTitlePolicy::Exact,
            elements,
        },
        InspectedWindow {
            selector: WindowSelector {
                executable: Some("desktop-automation-fixture.exe".to_owned()),
                title: Some("Trigix Automation Fixture Secondary".to_owned()),
                automation_id: Some("Trigix.AutomationFixture.Secondary".to_owned()),
                snapshot_id: None,
            },
            process_id: 4_242,
            title_policy: WindowTitlePolicy::Exact,
            elements: Vec::new(),
        },
    ]
}

fn window_matches(candidate: &WindowSelector, query: &WindowSelector) -> bool {
    query.executable.as_ref().is_none_or(|value| {
        candidate
            .executable
            .as_ref()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(value))
    }) && query
        .title
        .as_ref()
        .is_none_or(|value| candidate.title.as_ref() == Some(value))
        && query
            .automation_id
            .as_ref()
            .is_none_or(|value| candidate.automation_id.as_ref() == Some(value))
}

fn element_matches(candidate: &ElementSelector, query: &ElementSelector) -> bool {
    window_matches(&candidate.window, &query.window)
        && query
            .automation_id
            .as_ref()
            .is_none_or(|value| candidate.automation_id.as_ref() == Some(value))
        && query
            .name
            .as_ref()
            .is_none_or(|value| candidate.name.as_ref() == Some(value))
        && query.control_type.as_ref().is_none_or(|value| {
            candidate
                .control_type
                .as_ref()
                .is_some_and(|actual| actual.eq_ignore_ascii_case(value))
        })
}

fn bound_result(
    mut result: DesktopInspectionResult,
    request: &DesktopInspectionRequest,
) -> Result<DesktopInspectionResult, AutomationHostError> {
    if result.windows.len() > request.max_windows as usize {
        result.windows.truncate(request.max_windows as usize);
        result.truncated = true;
    }
    let mut remaining = request.max_elements as usize;
    for window in &mut result.windows {
        window
            .elements
            .retain(|element| element.depth <= request.max_depth);
        if window.elements.len() > remaining {
            window.elements.truncate(remaining);
            result.truncated = true;
        }
        remaining = remaining.saturating_sub(window.elements.len());
    }
    while serde_json::to_vec(&result)
        .map_err(|error| AutomationHostError::Adapter(error.to_string()))?
        .len()
        > request.max_payload_bytes as usize
    {
        let Some(window) = result
            .windows
            .iter_mut()
            .rev()
            .find(|window| !window.elements.is_empty())
        else {
            if result.windows.len() > 1 {
                result.windows.pop();
                result.truncated = true;
                continue;
            }
            return Err(AutomationHostError::Adapter(
                "inspection metadata exceeds payload limit".to_owned(),
            ));
        };
        window.elements.pop();
        result.truncated = true;
    }
    result.validate(request)?;
    Ok(result)
}

#[cfg(windows)]
mod windows_adapter {
    use super::*;
    use serde::Deserialize;
    use std::collections::{hash_map::DefaultHasher, HashMap};
    use std::env;
    use std::hash::{Hash, Hasher};
    use std::path::Path;
    use std::process::Command;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassNameW, GetDlgCtrlID, GetForegroundWindow,
        GetWindowLongW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, SendMessageW, SetForegroundWindow, SetWindowTextW, ShowWindow, BM_CLICK,
        ES_PASSWORD, GWL_STYLE, SW_RESTORE,
    };

    const APPLICATION_ALLOWLIST_ENV: &str = "TRIGIX_DESKTOP_APPLICATION_ALLOWLIST";

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ApplicationRegistration {
        application_id: String,
        executable_path: String,
    }

    #[derive(Debug, Default)]
    pub struct WindowsAutomationAdapter {
        applications: HashMap<String, String>,
    }

    impl WindowsAutomationAdapter {
        pub fn from_environment() -> Result<Self, AutomationHostError> {
            let Some(value) = env::var_os(APPLICATION_ALLOWLIST_ENV) else {
                return Ok(Self::default());
            };
            let registrations: Vec<ApplicationRegistration> = serde_json::from_str(
                value
                    .to_str()
                    .ok_or(AutomationHostError::InvalidRequest("application_allowlist"))?,
            )
            .map_err(|_| AutomationHostError::InvalidRequest("application_allowlist"))?;
            let mut applications = HashMap::new();
            for registration in registrations {
                desktop_protocol::ApplicationIdentity::new(&registration.application_id)
                    .validate()?;
                let path = Path::new(&registration.executable_path);
                if !path.is_absolute()
                    || registration.executable_path.chars().any(char::is_control)
                    || applications
                        .insert(registration.application_id, registration.executable_path)
                        .is_some()
                {
                    return Err(AutomationHostError::InvalidRequest("application_allowlist"));
                }
            }
            Ok(Self { applications })
        }

        fn focus(
            &self,
            selector: &WindowSelector,
            guard: Option<&AutomationExecutionGuard<'_>>,
        ) -> Result<Value, AutomationHostError> {
            if let Some(snapshot_id) = &selector.snapshot_id {
                let mut request = DesktopInspectionRequest::bounded(None);
                request.expected_snapshot_id = Some(snapshot_id.clone());
                inspect_windows(&request)?;
            }
            let matches = matching_windows(selector);
            let [window] = matches.as_slice() else {
                return if matches.is_empty() {
                    Err(AutomationHostError::TargetNotFound)
                } else {
                    Err(AutomationHostError::TargetAmbiguous)
                };
            };
            let mut process_id = 0;
            if let Some(guard) = guard {
                guard.ensure_active()?;
            }
            unsafe {
                GetWindowThreadProcessId(*window, &mut process_id);
                if IsIconic(*window) != 0 {
                    ShowWindow(*window, SW_RESTORE);
                }
                if SetForegroundWindow(*window) == 0 {
                    return Err(AutomationHostError::AccessDenied);
                }
            }
            Ok(json!({
                "focused": true,
                "process_id": process_id,
                "selector_strategy": selector_strategy(selector),
            }))
        }

        fn launch(
            &self,
            application_id: &desktop_protocol::ApplicationIdentity,
            guard: Option<&AutomationExecutionGuard<'_>>,
        ) -> Result<Value, AutomationHostError> {
            let executable = self
                .applications
                .get(&application_id.0)
                .ok_or(AutomationHostError::ApplicationNotAllowed)?;
            if let Some(guard) = guard {
                guard.ensure_active()?;
            }
            let child = Command::new(executable)
                .spawn()
                .map_err(|_| AutomationHostError::LaunchFailed)?;
            Ok(json!({
                "application_id": application_id.0,
                "process_id": child.id(),
                "launched": true,
            }))
        }

        fn resolve_element(
            &self,
            selector: &ElementSelector,
        ) -> Result<ResolvedElement, AutomationHostError> {
            if let Some(snapshot_id) = &selector.window.snapshot_id {
                let mut request = DesktopInspectionRequest::bounded(None);
                request.expected_snapshot_id = Some(snapshot_id.clone());
                inspect_windows(&request)?;
            }
            let matches = matching_elements(selector);
            match matches.as_slice() {
                [] => Err(AutomationHostError::TargetNotFound),
                [element] => Ok(*element),
                _ => Err(AutomationHostError::TargetAmbiguous),
            }
        }

        fn click(
            &self,
            selector: &ElementSelector,
            guard: Option<&AutomationExecutionGuard<'_>>,
        ) -> Result<Value, AutomationHostError> {
            let element = self.resolve_element(selector)?;
            if let Some(guard) = guard {
                guard.ensure_active()?;
            }
            unsafe {
                if GetForegroundWindow() != element.root {
                    return Err(AutomationHostError::FocusChanged);
                }
                if element.control_type != "button" {
                    return Err(AutomationHostError::UnsupportedPattern);
                }
                SendMessageW(element.control, BM_CLICK, 0, 0);
            }
            Ok(json!({
                "clicked": true,
                "semantic_pattern": "invoke",
            }))
        }

        fn type_text(
            &self,
            selector: &ElementSelector,
            text: &str,
            guard: Option<&AutomationExecutionGuard<'_>>,
        ) -> Result<Value, AutomationHostError> {
            let element = self.resolve_element(selector)?;
            if element.password {
                return Err(AutomationHostError::ProtectedControl);
            }
            if element.control_type != "edit" {
                return Err(AutomationHostError::UnsupportedPattern);
            }
            let wide = text
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            if let Some(guard) = guard {
                guard.ensure_active()?;
            }
            unsafe {
                if GetForegroundWindow() != element.root {
                    return Err(AutomationHostError::FocusChanged);
                }
                if SetWindowTextW(element.control, wide.as_ptr()) == 0 {
                    return Err(AutomationHostError::AccessDenied);
                }
                if window_text(element.control).as_deref() != Some(text) {
                    return Err(AutomationHostError::PartialEntry);
                }
            }
            Ok(json!({
                "entered": true,
                "characters_entered": text.chars().count(),
                "semantic_pattern": "value",
            }))
        }
    }

    impl AutomationAdapter for WindowsAutomationAdapter {
        fn execute(&mut self, action: &DesktopAction) -> Result<Value, AutomationHostError> {
            match action {
                DesktopAction::ReadSystemInformation => Ok(json!({
                    "adapter": "windows",
                    "platform": "windows",
                })),
                DesktopAction::InspectTargets { request } => {
                    serde_json::to_value(inspect_windows(request)?)
                        .map_err(|error| AutomationHostError::Adapter(error.to_string()))
                }
                DesktopAction::FocusWindow { selector } => self.focus(selector, None),
                DesktopAction::LaunchApplication { application_id } => {
                    self.launch(application_id, None)
                }
                DesktopAction::ClickElement { selector } => self.click(selector, None),
                DesktopAction::TypeText { selector, text } => self.type_text(selector, text, None),
            }
        }

        fn execute_guarded(
            &mut self,
            action: &DesktopAction,
            guard: &AutomationExecutionGuard<'_>,
        ) -> Result<Value, AutomationHostError> {
            guard.ensure_active()?;
            match action {
                DesktopAction::ReadSystemInformation | DesktopAction::InspectTargets { .. } => {
                    self.execute(action)
                }
                DesktopAction::FocusWindow { selector } => self.focus(selector, Some(guard)),
                DesktopAction::LaunchApplication { application_id } => {
                    self.launch(application_id, Some(guard))
                }
                DesktopAction::ClickElement { selector } => self.click(selector, Some(guard)),
                DesktopAction::TypeText { selector, text } => {
                    self.type_text(selector, text, Some(guard))
                }
            }
        }
    }

    struct WindowEnumeration<'a> {
        request: &'a DesktopInspectionRequest,
        started: Instant,
        windows: Vec<InspectedWindow>,
        element_count: usize,
        truncated: bool,
    }

    fn inspect_windows(
        request: &DesktopInspectionRequest,
    ) -> Result<DesktopInspectionResult, AutomationHostError> {
        let mut context = WindowEnumeration {
            request,
            started: Instant::now(),
            windows: Vec::new(),
            element_count: 0,
            truncated: false,
        };
        unsafe {
            EnumWindows(
                Some(enumerate_window),
                (&mut context as *mut WindowEnumeration<'_>) as LPARAM,
            );
        }
        if context.windows.is_empty() {
            return Err(AutomationHostError::TargetNotFound);
        }
        let snapshot_id = snapshot_id(&context.windows)?;
        if request
            .expected_snapshot_id
            .as_deref()
            .is_some_and(|expected| expected != snapshot_id)
        {
            return Err(AutomationHostError::TargetStale);
        }
        attach_snapshot(&mut context.windows, &snapshot_id);
        bound_result(
            DesktopInspectionResult {
                snapshot_id,
                windows: context.windows,
                truncated: context.truncated,
            },
            request,
        )
    }

    unsafe extern "system" fn enumerate_window(window: HWND, parameter: LPARAM) -> i32 {
        let context = &mut *(parameter as *mut WindowEnumeration<'_>);
        if context.started.elapsed()
            >= Duration::from_millis(context.request.max_duration_ms as u64)
        {
            context.truncated = true;
            return 0;
        }
        if context.windows.len() >= context.request.max_windows as usize {
            context.truncated = true;
            return 0;
        }
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(window, &mut process_id);
        if process_id == 0 {
            return 1;
        }
        let class_name = window_class(window);
        if class_name.is_empty() {
            return 1;
        }
        let title = window_text(window);
        let title_sensitive = title.as_deref().is_some_and(is_credential_text);
        let selector = WindowSelector {
            executable: process_executable(process_id),
            title: if title_sensitive { None } else { title },
            automation_id: Some(class_name),
            snapshot_id: None,
        };
        if context
            .request
            .window
            .as_ref()
            .is_some_and(|query| !window_matches(&selector, query))
        {
            return 1;
        }
        let mut child_context = ChildEnumeration {
            window: selector.clone(),
            started: context.started,
            max_duration: Duration::from_millis(context.request.max_duration_ms as u64),
            max_elements: (context.request.max_elements as usize)
                .saturating_sub(context.element_count),
            elements: Vec::new(),
            truncated: false,
        };
        EnumChildWindows(
            window,
            Some(enumerate_child),
            (&mut child_context as *mut ChildEnumeration) as LPARAM,
        );
        context.element_count += child_context.elements.len();
        context.truncated |= child_context.truncated;
        context.windows.push(InspectedWindow {
            selector,
            process_id,
            title_policy: if title_sensitive {
                WindowTitlePolicy::Redacted
            } else {
                WindowTitlePolicy::Exact
            },
            elements: child_context.elements,
        });
        1
    }

    struct ChildEnumeration {
        window: WindowSelector,
        started: Instant,
        max_duration: Duration,
        max_elements: usize,
        elements: Vec<InspectedElement>,
        truncated: bool,
    }

    unsafe extern "system" fn enumerate_child(window: HWND, parameter: LPARAM) -> i32 {
        let context = &mut *(parameter as *mut ChildEnumeration);
        if context.started.elapsed() >= context.max_duration
            || context.elements.len() >= context.max_elements
        {
            context.truncated = true;
            return 0;
        }
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let class_name = window_class(window);
        let control_type = class_name.to_ascii_lowercase();
        let text = window_text(window);
        let style = GetWindowLongW(window, GWL_STYLE) as u32;
        let password = control_type == "edit" && style & ES_PASSWORD as u32 != 0;
        let credential = text.as_deref().is_some_and(is_credential_text);
        let oversized = text.as_ref().is_some_and(|value| value.len() > 512);
        let redaction = if password {
            Some(RedactionReason::Password)
        } else if credential {
            Some(RedactionReason::Credential)
        } else if oversized {
            Some(RedactionReason::Oversized)
        } else {
            None
        };
        let control_id = GetDlgCtrlID(window);
        let automation_id = (control_id > 0).then(|| control_id.to_string());
        let patterns = match control_type.as_str() {
            "button" => vec![AutomationPattern::Invoke],
            "edit" if password => vec![AutomationPattern::Value],
            "edit" => vec![AutomationPattern::Value, AutomationPattern::Text],
            "combobox" | "listbox" => vec![AutomationPattern::Selection],
            _ => Vec::new(),
        };
        context.elements.push(InspectedElement {
            selector: ElementSelector {
                window: context.window.clone(),
                automation_id,
                name: if redaction.is_some() {
                    None
                } else {
                    text.clone()
                },
                control_type: Some(control_type),
            },
            depth: 1,
            supported_patterns: patterns,
            value: if redaction.is_some() { None } else { text },
            redaction,
        });
        1
    }

    struct MatchingWindows<'a> {
        selector: &'a WindowSelector,
        windows: Vec<HWND>,
    }

    fn matching_windows(selector: &WindowSelector) -> Vec<HWND> {
        let mut context = MatchingWindows {
            selector,
            windows: Vec::new(),
        };
        unsafe {
            EnumWindows(
                Some(match_window),
                (&mut context as *mut MatchingWindows<'_>) as LPARAM,
            );
        }
        context.windows
    }

    unsafe extern "system" fn match_window(window: HWND, parameter: LPARAM) -> i32 {
        let context = &mut *(parameter as *mut MatchingWindows<'_>);
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(window, &mut process_id);
        let selector = WindowSelector {
            executable: process_executable(process_id),
            title: window_text(window).filter(|title| !is_credential_text(title)),
            automation_id: Some(window_class(window)),
            snapshot_id: None,
        };
        if window_matches(&selector, context.selector) {
            context.windows.push(window);
        }
        1
    }

    #[derive(Clone, Copy)]
    struct ResolvedElement {
        root: HWND,
        control: HWND,
        password: bool,
        control_type: &'static str,
    }

    struct MatchingElements<'a> {
        root: HWND,
        selector: &'a ElementSelector,
        elements: Vec<ResolvedElement>,
    }

    fn matching_elements(selector: &ElementSelector) -> Vec<ResolvedElement> {
        let mut matches = Vec::new();
        for root in matching_windows(&selector.window) {
            let mut context = MatchingElements {
                root,
                selector,
                elements: Vec::new(),
            };
            unsafe {
                EnumChildWindows(
                    root,
                    Some(match_element),
                    (&mut context as *mut MatchingElements<'_>) as LPARAM,
                );
            }
            matches.extend(context.elements);
        }
        matches
    }

    unsafe extern "system" fn match_element(window: HWND, parameter: LPARAM) -> i32 {
        let context = &mut *(parameter as *mut MatchingElements<'_>);
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let class_name = window_class(window).to_ascii_lowercase();
        let control_type = match class_name.as_str() {
            "button" => "button",
            "edit" => "edit",
            "combobox" => "combobox",
            "listbox" => "listbox",
            _ => "custom",
        };
        let control_id = GetDlgCtrlID(window);
        let candidate = ElementSelector {
            window: context.selector.window.clone(),
            automation_id: (control_id > 0).then(|| control_id.to_string()),
            name: window_text(window),
            control_type: Some(control_type.to_owned()),
        };
        if element_matches(&candidate, context.selector) {
            context.elements.push(ResolvedElement {
                root: context.root,
                control: window,
                password: control_type == "edit"
                    && GetWindowLongW(window, GWL_STYLE) as u32 & ES_PASSWORD as u32 != 0,
                control_type,
            });
        }
        1
    }

    fn selector_strategy(selector: &WindowSelector) -> &'static str {
        if selector.automation_id.is_some() {
            "automation_id"
        } else if selector.executable.is_some() && selector.title.is_some() {
            "executable_and_title"
        } else if selector.executable.is_some() {
            "executable"
        } else {
            "title"
        }
    }

    unsafe fn window_text(window: HWND) -> Option<String> {
        let length = GetWindowTextLengthW(window);
        if length <= 0 {
            return None;
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32);
        (copied > 0).then(|| String::from_utf16_lossy(&buffer[..copied as usize]))
    }

    unsafe fn window_class(window: HWND) -> String {
        let mut buffer = vec![0u16; 256];
        let copied = GetClassNameW(window, buffer.as_mut_ptr(), buffer.len() as i32);
        if copied <= 0 {
            String::new()
        } else {
            String::from_utf16_lossy(&buffer[..copied as usize])
        }
    }

    unsafe fn process_executable(process_id: u32) -> Option<String> {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 32_768];
        let mut length = buffer.len() as u32;
        let found = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) != 0;
        CloseHandle(process);
        if !found {
            return None;
        }
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    fn is_credential_text(value: &str) -> bool {
        let normalized = value.to_ascii_lowercase();
        ["password", "credential", "secret", "token", "密码", "口令"]
            .iter()
            .any(|marker| normalized.contains(marker))
    }

    fn snapshot_id(windows: &[InspectedWindow]) -> Result<String, AutomationHostError> {
        let serialized = serde_json::to_vec(windows)
            .map_err(|error| AutomationHostError::Adapter(error.to_string()))?;
        let mut hasher = DefaultHasher::new();
        serialized.hash(&mut hasher);
        Ok(format!("windows-{:016x}", hasher.finish()))
    }
}

#[cfg(windows)]
pub use windows_adapter::WindowsAutomationAdapter;

pub fn run_host<R, W, A, C>(
    mut reader: R,
    mut writer: W,
    mut adapter: A,
    clock: C,
) -> Result<(), AutomationHostError>
where
    R: BufRead,
    W: Write,
    A: AutomationAdapter,
    C: Fn() -> u64,
{
    loop {
        let Some(line) = read_bounded_line(&mut reader)? else {
            return Ok(());
        };
        let request: AutomationHostRequest = serde_json::from_slice(&line)
            .map_err(|_| AutomationHostError::InvalidRequest("json"))?;
        request.validate(clock())?;
        let should_shutdown = matches!(request.operation, AutomationHostOperation::Shutdown);
        let response = dispatch_request(&mut adapter, request, &clock);
        response.validate()?;
        serde_json::to_writer(&mut writer, &response)
            .map_err(|error| AutomationHostError::Io(error.to_string()))?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        if should_shutdown {
            return Ok(());
        }
    }
}

fn read_bounded_line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, AutomationHostError> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(AutomationHostError::InvalidRequest("message_size"))
            };
        }
        let consumed = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |index| index + 1);
        if line.len() + consumed > MAX_HOST_MESSAGE_BYTES as usize {
            return Err(AutomationHostError::InvalidRequest("message_size"));
        }
        let complete = buffer.get(consumed.saturating_sub(1)) == Some(&b'\n');
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if complete {
            return Ok(Some(line));
        }
    }
}

fn dispatch_request(
    adapter: &mut impl AutomationAdapter,
    request: AutomationHostRequest,
    clock: &dyn Fn() -> u64,
) -> AutomationHostResponse {
    let request_id = request.request_id;
    let now_unix_ms = clock();
    if request.deadline_unix_ms <= now_unix_ms {
        return failure_response(
            request_id,
            AutomationHostStatus::Rejected,
            "deadline_expired",
            "automation request deadline expired before execution",
        );
    }
    match request.operation {
        AutomationHostOperation::Health => response(request_id, AutomationHostStatus::Ready, None),
        AutomationHostOperation::Shutdown => {
            response(request_id, AutomationHostStatus::ShuttingDown, None)
        }
        AutomationHostOperation::Cancel { target_request_id } => failure_response(
            request_id,
            AutomationHostStatus::Rejected,
            "target_not_active",
            &format!("request {target_request_id} is not active"),
        ),
        AutomationHostOperation::Execute {
            lease_expires_at_unix_ms,
            ..
        } if lease_expires_at_unix_ms <= now_unix_ms => failure_response(
            request_id,
            AutomationHostStatus::Rejected,
            "lease_expired",
            "execution lease expired before the side effect",
        ),
        AutomationHostOperation::Execute {
            lease_expires_at_unix_ms,
            action,
            ..
        } => match adapter.execute_guarded(
            &action,
            &AutomationExecutionGuard {
                deadline_unix_ms: request.deadline_unix_ms,
                lease_expires_at_unix_ms,
                clock,
            },
        ) {
            Ok(output) => response(request_id, AutomationHostStatus::Succeeded, Some(output)),
            Err(AutomationHostError::UnsupportedAction) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "unsupported_action",
                "automation action is unsupported on this host",
            ),
            Err(AutomationHostError::TargetNotFound) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "target_not_found",
                "automation target was not found",
            ),
            Err(AutomationHostError::TargetAmbiguous) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "target_ambiguous",
                "automation target is ambiguous",
            ),
            Err(AutomationHostError::TargetStale) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "target_stale",
                "automation target snapshot is stale",
            ),
            Err(AutomationHostError::ApplicationNotAllowed) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "application_not_allowed",
                "application is not present in the launch allowlist",
            ),
            Err(AutomationHostError::AccessDenied) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "access_denied",
                "operating system denied the action",
            ),
            Err(AutomationHostError::LaunchFailed) => failure_response(
                request_id,
                AutomationHostStatus::Failed,
                "launch_failed",
                "application launch failed",
            ),
            Err(AutomationHostError::UnsupportedPattern) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "unsupported_pattern",
                "target does not support the requested semantic pattern",
            ),
            Err(AutomationHostError::ProtectedControl) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "protected_control",
                "text entry into a protected control is prohibited",
            ),
            Err(AutomationHostError::FocusChanged) => failure_response(
                request_id,
                AutomationHostStatus::Rejected,
                "focus_changed",
                "target window is not in the foreground",
            ),
            Err(AutomationHostError::PartialEntry) => failure_response(
                request_id,
                AutomationHostStatus::Failed,
                "partial_entry",
                "text entry could not be verified completely",
            ),
            Err(AutomationHostError::LeaseExpired) => failure_response(
                request_id,
                AutomationHostStatus::TimedOut,
                "lease_expired",
                "execution lease expired before the side effect",
            ),
            Err(AutomationHostError::DeadlineExpired) => failure_response(
                request_id,
                AutomationHostStatus::TimedOut,
                "deadline_expired",
                "automation request deadline expired before the side effect",
            ),
            Err(error) => failure_response(
                request_id,
                AutomationHostStatus::Failed,
                "adapter_failed",
                &error.to_string(),
            ),
        },
    }
}

fn response(
    request_id: String,
    status: AutomationHostStatus,
    output: Option<Value>,
) -> AutomationHostResponse {
    AutomationHostResponse {
        request_id,
        status,
        output,
        error_code: None,
        error_message: None,
    }
}

fn failure_response(
    request_id: String,
    status: AutomationHostStatus,
    code: &str,
    message: &str,
) -> AutomationHostResponse {
    AutomationHostResponse {
        request_id,
        status,
        output: None,
        error_code: Some(code.to_owned()),
        error_message: Some(message.to_owned()),
    }
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), AutomationHostError> {
    validate_text(value, field, 128)
}

fn validate_text(
    value: &str,
    field: &'static str,
    maximum_length: usize,
) -> Result<(), AutomationHostError> {
    if value.trim().is_empty()
        || value.len() > maximum_length
        || value.chars().any(char::is_control)
    {
        Err(AutomationHostError::InvalidRequest(field))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;

    fn request(operation: AutomationHostOperation) -> AutomationHostRequest {
        AutomationHostRequest {
            request_id: "request-1".to_owned(),
            sent_at_unix_ms: 1_000,
            deadline_unix_ms: 2_000,
            operation,
        }
    }

    #[test]
    fn host_executes_fixture_action_and_shuts_down() {
        let execute = request(AutomationHostOperation::Execute {
            command_id: "command-1".to_owned(),
            lease_id: "lease-1".to_owned(),
            lease_expires_at_unix_ms: 1_900,
            action: Box::new(DesktopAction::ReadSystemInformation),
        });
        let mut shutdown = request(AutomationHostOperation::Shutdown);
        shutdown.request_id = "request-2".to_owned();
        let input = format!(
            "{}\n{}\n",
            serde_json::to_string(&execute).unwrap(),
            serde_json::to_string(&shutdown).unwrap()
        );
        let mut output = Vec::new();

        run_host(
            BufReader::new(Cursor::new(input)),
            &mut output,
            FixtureAutomationAdapter::default(),
            || 1_500,
        )
        .unwrap();

        let responses = String::from_utf8(output).unwrap();
        let mut lines = responses.lines();
        let executed: AutomationHostResponse = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(executed.status, AutomationHostStatus::Succeeded);
        assert_eq!(executed.output.unwrap()["adapter"], "fixture");
        let shutdown: AutomationHostResponse = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(shutdown.status, AutomationHostStatus::ShuttingDown);
        assert!(lines.next().is_none());
    }

    #[test]
    fn unallowlisted_application_is_rejected_without_side_effect() {
        let response = dispatch_request(
            &mut FixtureAutomationAdapter::default(),
            request(AutomationHostOperation::Execute {
                command_id: "command-1".to_owned(),
                lease_id: "lease-1".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: Box::new(DesktopAction::LaunchApplication {
                    application_id: desktop_protocol::ApplicationIdentity::new("fixture"),
                }),
            }),
            &|| 1_500,
        );
        assert_eq!(response.status, AutomationHostStatus::Rejected);
        assert_eq!(
            response.error_code.as_deref(),
            Some("application_not_allowed")
        );
    }

    #[test]
    fn expired_and_oversized_requests_fail_closed() {
        let expired = request(AutomationHostOperation::Health);
        assert_eq!(
            expired.validate(2_000),
            Err(AutomationHostError::InvalidRequest("deadline_unix_ms"))
        );
        let lease_expired = request(AutomationHostOperation::Execute {
            command_id: "command-1".to_owned(),
            lease_id: "lease-1".to_owned(),
            lease_expires_at_unix_ms: 1_400,
            action: Box::new(DesktopAction::ReadSystemInformation),
        });
        assert_eq!(
            lease_expired.validate(1_500),
            Err(AutomationHostError::InvalidRequest(
                "lease_expires_at_unix_ms"
            ))
        );
        let rechecked = dispatch_request(
            &mut FixtureAutomationAdapter::default(),
            request(AutomationHostOperation::Execute {
                command_id: "command-1".to_owned(),
                lease_id: "lease-1".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: Box::new(DesktopAction::LaunchApplication {
                    application_id: desktop_protocol::ApplicationIdentity::new(
                        "trigix.automation-fixture",
                    ),
                }),
            }),
            &|| 1_900,
        );
        assert_eq!(rechecked.error_code.as_deref(), Some("lease_expired"));

        let input = vec![b'x'; MAX_HOST_MESSAGE_BYTES as usize + 1];
        assert_eq!(
            run_host(
                BufReader::new(Cursor::new(input)),
                Vec::new(),
                FixtureAutomationAdapter::default(),
                || 1_500,
            ),
            Err(AutomationHostError::InvalidRequest("message_size"))
        );
    }

    #[test]
    fn cancellation_of_inactive_request_is_explicitly_rejected() {
        let response = dispatch_request(
            &mut FixtureAutomationAdapter::default(),
            request(AutomationHostOperation::Cancel {
                target_request_id: "request-missing".to_owned(),
            }),
            &|| 1_500,
        );
        assert_eq!(response.status, AutomationHostStatus::Rejected);
        assert_eq!(response.error_code.as_deref(), Some("target_not_active"));
    }

    #[test]
    fn fixture_descriptor_has_stable_semantic_identifiers() {
        let descriptor = AutomationFixtureDescriptor::default();
        assert_eq!(
            descriptor.window_automation_id,
            FIXTURE_WINDOW_AUTOMATION_ID
        );
        assert_eq!(descriptor.input_automation_id, FIXTURE_INPUT_AUTOMATION_ID);
        assert_eq!(
            descriptor.submit_automation_id,
            FIXTURE_SUBMIT_AUTOMATION_ID
        );
        assert_eq!(
            descriptor.password_automation_id,
            FIXTURE_PASSWORD_AUTOMATION_ID
        );
    }

    #[test]
    fn inspection_preserves_stable_ids_and_redacts_sensitive_values() {
        let output = FixtureAutomationAdapter::default()
            .execute(&DesktopAction::InspectTargets {
                request: Box::new(DesktopInspectionRequest::bounded(None)),
            })
            .unwrap();
        let result: DesktopInspectionResult = serde_json::from_value(output).unwrap();

        assert_eq!(result.windows.len(), 2);
        let primary = &result.windows[0];
        assert_eq!(primary.process_id, 4_242);
        assert_eq!(
            primary.selector.automation_id.as_deref(),
            Some(FIXTURE_WINDOW_AUTOMATION_ID)
        );
        assert_eq!(primary.elements[1].selector.name.as_deref(), Some("提交"));
        let password = &primary.elements[2];
        assert_eq!(password.redaction, Some(RedactionReason::Password));
        assert!(password.value.is_none());
        assert_eq!(
            password.selector.automation_id.as_deref(),
            Some(FIXTURE_PASSWORD_AUTOMATION_ID)
        );
    }

    #[test]
    fn inspection_reports_missing_and_stale_targets_explicitly() {
        let mut missing = DesktopInspectionRequest::bounded(Some(WindowSelector {
            executable: None,
            title: None,
            automation_id: Some("missing-window".to_owned()),
            snapshot_id: None,
        }));
        let response = dispatch_request(
            &mut FixtureAutomationAdapter::default(),
            request(AutomationHostOperation::Execute {
                command_id: "command-1".to_owned(),
                lease_id: "lease-1".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: Box::new(DesktopAction::InspectTargets {
                    request: Box::new(missing.clone()),
                }),
            }),
            &|| 1_500,
        );
        assert_eq!(response.error_code.as_deref(), Some("target_not_found"));

        missing.window = None;
        missing.expected_snapshot_id = Some("old-snapshot".to_owned());
        let response = dispatch_request(
            &mut FixtureAutomationAdapter::default(),
            request(AutomationHostOperation::Execute {
                command_id: "command-2".to_owned(),
                lease_id: "lease-2".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: Box::new(DesktopAction::InspectTargets {
                    request: Box::new(missing),
                }),
            }),
            &|| 1_500,
        );
        assert_eq!(response.error_code.as_deref(), Some("target_stale"));
    }

    #[test]
    fn inspection_bounds_ambiguous_results_by_count_and_payload() {
        let mut limits = DesktopInspectionRequest::bounded(Some(WindowSelector {
            executable: Some("desktop-automation-fixture.exe".to_owned()),
            title: None,
            automation_id: None,
            snapshot_id: None,
        }));
        limits.max_elements = 1;
        let output = FixtureAutomationAdapter::default()
            .execute(&DesktopAction::InspectTargets {
                request: Box::new(limits),
            })
            .unwrap();
        let result: DesktopInspectionResult = serde_json::from_value(output).unwrap();
        assert_eq!(result.windows.len(), 2);
        assert_eq!(result.windows[0].elements.len(), 1);
        assert!(result.truncated);
    }

    #[test]
    fn fixture_focus_requires_a_unique_fresh_selector() {
        let adapter = &mut FixtureAutomationAdapter::default();
        let focused = adapter
            .execute(&DesktopAction::FocusWindow {
                selector: WindowSelector {
                    executable: Some("desktop-automation-fixture.exe".to_owned()),
                    title: None,
                    automation_id: Some(FIXTURE_WINDOW_AUTOMATION_ID.to_owned()),
                    snapshot_id: Some("fixture-snapshot-1".to_owned()),
                },
            })
            .unwrap();
        assert_eq!(focused["focused"], true);
        assert_eq!(focused["selector_strategy"], "automation_id");

        let ambiguous = adapter.execute(&DesktopAction::FocusWindow {
            selector: WindowSelector {
                executable: Some("desktop-automation-fixture.exe".to_owned()),
                title: None,
                automation_id: None,
                snapshot_id: None,
            },
        });
        assert_eq!(ambiguous, Err(AutomationHostError::TargetAmbiguous));

        let stale = adapter.execute(&DesktopAction::FocusWindow {
            selector: WindowSelector {
                executable: None,
                title: None,
                automation_id: Some(FIXTURE_WINDOW_AUTOMATION_ID.to_owned()),
                snapshot_id: Some("old-snapshot".to_owned()),
            },
        });
        assert_eq!(stale, Err(AutomationHostError::TargetStale));
    }

    #[test]
    fn fixture_launch_accepts_only_registered_application_identity() {
        let adapter = &mut FixtureAutomationAdapter::default();
        let launched = adapter
            .execute(&DesktopAction::LaunchApplication {
                application_id: desktop_protocol::ApplicationIdentity::new(
                    "trigix.automation-fixture",
                ),
            })
            .unwrap();
        assert_eq!(launched["launched"], true);
        assert!(launched.get("executable_path").is_none());

        let rejected = dispatch_request(
            adapter,
            request(AutomationHostOperation::Execute {
                command_id: "command-1".to_owned(),
                lease_id: "lease-1".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: Box::new(DesktopAction::LaunchApplication {
                    application_id: desktop_protocol::ApplicationIdentity::new("cmd.exe /c whoami"),
                }),
            }),
            &|| 1_500,
        );
        assert_eq!(
            rejected.error_code.as_deref(),
            Some("application_not_allowed")
        );
    }

    fn fixture_element(automation_id: &str, control_type: &str) -> ElementSelector {
        ElementSelector {
            window: WindowSelector {
                executable: Some("desktop-automation-fixture.exe".to_owned()),
                title: None,
                automation_id: Some(FIXTURE_WINDOW_AUTOMATION_ID.to_owned()),
                snapshot_id: Some("fixture-snapshot-1".to_owned()),
            },
            automation_id: Some(automation_id.to_owned()),
            name: None,
            control_type: Some(control_type.to_owned()),
        }
    }

    #[test]
    fn fixture_click_uses_invoke_and_rejects_unsupported_or_ambiguous_targets() {
        let adapter = &mut FixtureAutomationAdapter::default();
        let clicked = adapter
            .execute(&DesktopAction::ClickElement {
                selector: fixture_element(FIXTURE_SUBMIT_AUTOMATION_ID, "button"),
            })
            .unwrap();
        assert_eq!(clicked["semantic_pattern"], "invoke");

        let unsupported = adapter.execute(&DesktopAction::ClickElement {
            selector: fixture_element(FIXTURE_INPUT_AUTOMATION_ID, "edit"),
        });
        assert_eq!(unsupported, Err(AutomationHostError::UnsupportedPattern));

        let mut ambiguous = fixture_element(FIXTURE_INPUT_AUTOMATION_ID, "edit");
        ambiguous.automation_id = None;
        let ambiguous = adapter.execute(&DesktopAction::ClickElement {
            selector: ambiguous,
        });
        assert_eq!(ambiguous, Err(AutomationHostError::TargetAmbiguous));
    }

    #[test]
    fn fixture_text_entry_redacts_input_and_rejects_password_or_focus_change() {
        let adapter = &mut FixtureAutomationAdapter::default();
        let entered = adapter
            .execute(&DesktopAction::TypeText {
                selector: fixture_element(FIXTURE_INPUT_AUTOMATION_ID, "edit"),
                text: "private input".to_owned(),
            })
            .unwrap();
        assert_eq!(entered["characters_entered"], 13);
        assert!(!entered.to_string().contains("private input"));

        let protected = adapter.execute(&DesktopAction::TypeText {
            selector: fixture_element(FIXTURE_PASSWORD_AUTOMATION_ID, "edit"),
            text: "never-enter-this".to_owned(),
        });
        assert_eq!(protected, Err(AutomationHostError::ProtectedControl));

        adapter.focused_window_id = "Trigix.AutomationFixture.Secondary".to_owned();
        let focus_changed = adapter.execute(&DesktopAction::ClickElement {
            selector: fixture_element(FIXTURE_SUBMIT_AUTOMATION_ID, "button"),
        });
        assert_eq!(focus_changed, Err(AutomationHostError::FocusChanged));
    }

    #[test]
    fn lease_is_rechecked_after_resolution_and_before_side_effect() {
        struct ResolvingAdapter {
            clock: Arc<AtomicU64>,
            side_effect: Arc<AtomicBool>,
        }

        impl AutomationAdapter for ResolvingAdapter {
            fn execute(&mut self, _: &DesktopAction) -> Result<Value, AutomationHostError> {
                self.side_effect.store(true, Ordering::Release);
                Ok(json!({"executed": true}))
            }

            fn execute_guarded(
                &mut self,
                action: &DesktopAction,
                guard: &AutomationExecutionGuard<'_>,
            ) -> Result<Value, AutomationHostError> {
                guard.ensure_active()?;
                self.clock.store(1_900, Ordering::Release);
                guard.ensure_active()?;
                self.execute(action)
            }
        }

        let clock = Arc::new(AtomicU64::new(1_500));
        let side_effect = Arc::new(AtomicBool::new(false));
        let mut adapter = ResolvingAdapter {
            clock: Arc::clone(&clock),
            side_effect: Arc::clone(&side_effect),
        };
        let response = dispatch_request(
            &mut adapter,
            request(AutomationHostOperation::Execute {
                command_id: "command-guarded".to_owned(),
                lease_id: "lease-guarded".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: Box::new(DesktopAction::ReadSystemInformation),
            }),
            &|| clock.load(Ordering::Acquire),
        );
        assert_eq!(response.status, AutomationHostStatus::TimedOut);
        assert_eq!(response.error_code.as_deref(), Some("lease_expired"));
        assert!(!side_effect.load(Ordering::Acquire));
        response.validate().unwrap();
    }
}
