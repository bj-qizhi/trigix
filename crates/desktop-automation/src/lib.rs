use desktop_protocol::{DesktopAction, ProtocolError};
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
    Adapter(String),
    Io(String),
}

impl fmt::Display for AutomationHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(formatter, "protocol error: {error}"),
            Self::InvalidRequest(field) => write!(formatter, "invalid host request: {field}"),
            Self::UnsupportedAction => formatter.write_str("automation action is unsupported"),
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

#[derive(Debug, Default)]
pub struct FixtureAutomationAdapter;

impl AutomationAdapter for FixtureAutomationAdapter {
    fn execute(&mut self, action: &DesktopAction) -> Result<Value, AutomationHostError> {
        if action != &DesktopAction::ReadSystemInformation {
            return Err(AutomationHostError::UnsupportedAction);
        }
        Ok(json!({
            "adapter": "fixture",
            "platform": std::env::consts::OS,
            "fixture": AutomationFixtureDescriptor::default(),
        }))
    }
}

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
            FixtureAutomationAdapter,
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
            &mut FixtureAutomationAdapter,
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
                FixtureAutomationAdapter,
                || 1_500,
            ),
            Err(AutomationHostError::InvalidRequest("message_size"))
        );
    }

    #[test]
    fn cancellation_of_inactive_request_is_explicitly_rejected() {
        let response = dispatch_request(
            &mut FixtureAutomationAdapter,
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
}
