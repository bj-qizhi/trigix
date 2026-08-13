use desktop_protocol::{
    AutomationPattern, DesktopAction, DesktopInspectionRequest, DesktopInspectionResult,
    ElementSelector, InspectedElement, InspectedWindow, ProtocolError, RedactionReason,
    WindowSelector, WindowTitlePolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::io::{self, BufRead, Write};

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
        action: DesktopAction,
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
            AutomationHostStatus::Rejected | AutomationHostStatus::Failed
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

pub trait AutomationAdapter {
    fn execute(&mut self, action: &DesktopAction) -> Result<Value, AutomationHostError>;
}

#[derive(Debug)]
pub struct FixtureAutomationAdapter {
    snapshot_id: String,
}

impl Default for FixtureAutomationAdapter {
    fn default() -> Self {
        Self {
            snapshot_id: "fixture-snapshot-1".to_owned(),
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
            _ => Err(AutomationHostError::UnsupportedAction),
        }
    }
}

impl FixtureAutomationAdapter {
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

fn fixture_windows() -> Vec<InspectedWindow> {
    let descriptor = AutomationFixtureDescriptor::default();
    let primary = WindowSelector {
        executable: Some("desktop-automation-fixture.exe".to_owned()),
        title: Some("Trigix 自动化测试".to_owned()),
        automation_id: Some(descriptor.window_automation_id.clone()),
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::path::Path;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassNameW, GetDlgCtrlID, GetWindowLongW,
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
        ES_PASSWORD, GWL_STYLE,
    };

    #[derive(Debug, Default)]
    pub struct WindowsAutomationAdapter;

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
                _ => Err(AutomationHostError::UnsupportedAction),
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
        let response = dispatch_request(&mut adapter, request);
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
) -> AutomationHostResponse {
    let request_id = request.request_id;
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
        AutomationHostOperation::Execute { action, .. } => match adapter.execute(&action) {
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
            action: DesktopAction::ReadSystemInformation,
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
    fn unsupported_action_is_rejected_without_side_effect() {
        let response = dispatch_request(
            &mut FixtureAutomationAdapter::default(),
            request(AutomationHostOperation::Execute {
                command_id: "command-1".to_owned(),
                lease_id: "lease-1".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: DesktopAction::LaunchApplication {
                    application_id: "fixture".to_owned(),
                },
            }),
        );
        assert_eq!(response.status, AutomationHostStatus::Rejected);
        assert_eq!(response.error_code.as_deref(), Some("unsupported_action"));
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
            action: DesktopAction::ReadSystemInformation,
        });
        assert_eq!(
            lease_expired.validate(1_500),
            Err(AutomationHostError::InvalidRequest(
                "lease_expires_at_unix_ms"
            ))
        );

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
                request: DesktopInspectionRequest::bounded(None),
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
        }));
        let response = dispatch_request(
            &mut FixtureAutomationAdapter::default(),
            request(AutomationHostOperation::Execute {
                command_id: "command-1".to_owned(),
                lease_id: "lease-1".to_owned(),
                lease_expires_at_unix_ms: 1_900,
                action: DesktopAction::InspectTargets {
                    request: missing.clone(),
                },
            }),
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
                action: DesktopAction::InspectTargets { request: missing },
            }),
        );
        assert_eq!(response.error_code.as_deref(), Some("target_stale"));
    }

    #[test]
    fn inspection_bounds_ambiguous_results_by_count_and_payload() {
        let mut limits = DesktopInspectionRequest::bounded(Some(WindowSelector {
            executable: Some("desktop-automation-fixture.exe".to_owned()),
            title: None,
            automation_id: None,
        }));
        limits.max_elements = 1;
        let output = FixtureAutomationAdapter::default()
            .execute(&DesktopAction::InspectTargets { request: limits })
            .unwrap();
        let result: DesktopInspectionResult = serde_json::from_value(output).unwrap();
        assert_eq!(result.windows.len(), 2);
        assert_eq!(result.windows[0].elements.len(), 1);
        assert!(result.truncated);
    }
}
