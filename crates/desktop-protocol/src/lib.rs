use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

pub const PROTOCOL_VERSION: &str = "desktop.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    UnsupportedVersion(String),
    MissingField(&'static str),
    InvalidField(&'static str),
    ExpiredLease,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported protocol version: {version}")
            }
            Self::MissingField(field) => write!(formatter, "missing required field: {field}"),
            Self::InvalidField(field) => write!(formatter, "invalid field: {field}"),
            Self::ExpiredLease => formatter.write_str("execution lease has expired"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub protocol_version: String,
    pub message_id: String,
    pub sent_at_unix_ms: u64,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(message_id: impl Into<String>, sent_at_unix_ms: u64, payload: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_owned(),
            message_id: message_id.into(),
            sent_at_unix_ms,
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(
                self.protocol_version.clone(),
            ));
        }
        validate_identifier("message_id", &self.message_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDescriptor {
    pub device_id: String,
    pub display_name: String,
    pub operating_system: String,
    pub agent_version: String,
    pub capabilities: Vec<DeviceCapability>,
}

impl DeviceDescriptor {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_identifier("device_id", &self.device_id)?;
        validate_text("display_name", &self.display_name, 128)?;
        validate_text("operating_system", &self.operating_system, 64)?;
        validate_text("agent_version", &self.agent_version, 32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceCapability {
    SystemInformation,
    WindowManagement,
    UiAutomation,
    KeyboardInput,
    PointerInput,
    VoiceConversation,
    AvatarRendering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingRequest {
    pub pairing_code: String,
    pub device: DeviceDescriptor,
    pub device_public_key: String,
}

impl PairingRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.pairing_code.len() != 8
            || !self
                .pairing_code
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err(ProtocolError::InvalidField("pairing_code"));
        }
        validate_text("device_public_key", &self.device_public_key, 4096)?;
        self.device.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingAccepted {
    pub device_id: String,
    pub tenant_id: String,
    pub credential_reference: String,
    pub heartbeat_interval_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConnectionAccepted {
    pub device_id: String,
    pub session_id: String,
    pub server_time_unix_ms: u64,
    pub heartbeat_interval_seconds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    Online,
    Busy,
    AwaitingApproval,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    pub device_id: String,
    pub state: DeviceState,
    pub active_execution_id: Option<String>,
    pub agent_version: String,
    pub capabilities: Vec<DeviceCapability>,
    pub health_detail: Option<String>,
}

impl Heartbeat {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_identifier("device_id", &self.device_id)?;
        validate_text("agent_version", &self.agent_version, 32)?;
        if let Some(execution_id) = &self.active_execution_id {
            validate_identifier("active_execution_id", execution_id)?;
        }
        if self.capabilities.len() > 32 {
            return Err(ProtocolError::InvalidField("capabilities"));
        }
        for (index, capability) in self.capabilities.iter().enumerate() {
            if self.capabilities[..index].contains(capability) {
                return Err(ProtocolError::InvalidField("capabilities"));
            }
        }
        validate_optional_text("health_detail", self.health_detail.as_deref(), 256)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatAccepted {
    pub device_id: String,
    pub session_id: String,
    pub state: DeviceState,
    pub server_time_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopAction {
    ReadSystemInformation,
    FocusWindow {
        selector: WindowSelector,
    },
    ClickElement {
        selector: ElementSelector,
    },
    TypeText {
        selector: ElementSelector,
        text: String,
    },
    LaunchApplication {
        application_id: String,
    },
}

impl DesktopAction {
    pub fn risk_level(&self) -> RiskLevel {
        match self {
            Self::ReadSystemInformation => RiskLevel::Low,
            Self::FocusWindow { .. } | Self::ClickElement { .. } => RiskLevel::Medium,
            Self::TypeText { .. } | Self::LaunchApplication { .. } => RiskLevel::High,
        }
    }

    pub fn capability(&self) -> DeviceCapability {
        match self {
            Self::ReadSystemInformation => DeviceCapability::SystemInformation,
            Self::FocusWindow { .. } | Self::LaunchApplication { .. } => {
                DeviceCapability::WindowManagement
            }
            Self::ClickElement { .. } | Self::TypeText { .. } => DeviceCapability::UiAutomation,
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::ReadSystemInformation => Ok(()),
            Self::FocusWindow { selector } => selector.validate(),
            Self::ClickElement { selector } => selector.validate(),
            Self::TypeText { selector, text } => {
                selector.validate()?;
                validate_text("action.text", text, 16_384)
            }
            Self::LaunchApplication { application_id } => {
                validate_text("action.application_id", application_id, 256)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSelector {
    pub executable: Option<String>,
    pub title: Option<String>,
    pub automation_id: Option<String>,
}

impl WindowSelector {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.executable.is_none() && self.title.is_none() && self.automation_id.is_none() {
            return Err(ProtocolError::MissingField("action.window_selector"));
        }
        validate_optional_text("action.window.executable", self.executable.as_deref(), 512)?;
        validate_optional_text("action.window.title", self.title.as_deref(), 512)?;
        validate_optional_text(
            "action.window.automation_id",
            self.automation_id.as_deref(),
            256,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementSelector {
    pub window: WindowSelector,
    pub automation_id: Option<String>,
    pub name: Option<String>,
    pub control_type: Option<String>,
}

impl ElementSelector {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.window.validate()?;
        if self.automation_id.is_none() && self.name.is_none() && self.control_type.is_none() {
            return Err(ProtocolError::MissingField("action.element_selector"));
        }
        validate_optional_text(
            "action.element.automation_id",
            self.automation_id.as_deref(),
            256,
        )?;
        validate_optional_text("action.element.name", self.name.as_deref(), 512)?;
        validate_optional_text(
            "action.element.control_type",
            self.control_type.as_deref(),
            128,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLease {
    pub lease_id: String,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopCommand {
    pub command_id: String,
    pub execution_id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub requested_by: String,
    pub issued_at_unix_ms: u64,
    pub lease: ExecutionLease,
    pub action: DesktopAction,
}

impl DesktopCommand {
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        validate_identifier("command_id", &self.command_id)?;
        validate_identifier("execution_id", &self.execution_id)?;
        validate_identifier("tenant_id", &self.tenant_id)?;
        validate_identifier("project_id", &self.project_id)?;
        validate_identifier("requested_by", &self.requested_by)?;
        validate_identifier("lease_id", &self.lease.lease_id)?;
        if self.lease.expires_at_unix_ms <= self.issued_at_unix_ms {
            return Err(ProtocolError::InvalidField("lease.expires_at_unix_ms"));
        }
        if self.lease.expires_at_unix_ms <= now_unix_ms {
            return Err(ProtocolError::ExpiredLease);
        }
        self.action.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutcome {
    Succeeded,
    Failed,
    Rejected,
    AwaitingApproval,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopCommandResult {
    pub command_id: String,
    pub execution_id: String,
    pub outcome: CommandOutcome,
    pub completed_at_unix_ms: u64,
    pub output: Option<Value>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), ProtocolError> {
    validate_text(field, value, 128)
}

fn validate_text(
    field: &'static str,
    value: &str,
    maximum_length: usize,
) -> Result<(), ProtocolError> {
    if value.trim().is_empty() {
        return Err(ProtocolError::MissingField(field));
    }
    if value.len() > maximum_length || value.chars().any(char::is_control) {
        return Err(ProtocolError::InvalidField(field));
    }
    Ok(())
}

fn validate_optional_text(
    field: &'static str,
    value: Option<&str>,
    maximum_length: usize,
) -> Result<(), ProtocolError> {
    match value {
        Some(value) => validate_text(field, value, maximum_length),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> DesktopCommand {
        DesktopCommand {
            command_id: "command-1".to_owned(),
            execution_id: "execution-1".to_owned(),
            tenant_id: "tenant-1".to_owned(),
            project_id: "project-1".to_owned(),
            requested_by: "user-1".to_owned(),
            issued_at_unix_ms: 1_000,
            lease: ExecutionLease {
                lease_id: "lease-1".to_owned(),
                expires_at_unix_ms: 2_000,
            },
            action: DesktopAction::ReadSystemInformation,
        }
    }

    #[test]
    fn envelope_rejects_unknown_protocol_versions() {
        let mut envelope = Envelope::new("message-1", 1_000, command());
        envelope.protocol_version = "desktop.v2".to_owned();

        assert_eq!(
            envelope.validate(),
            Err(ProtocolError::UnsupportedVersion("desktop.v2".to_owned()))
        );
    }

    #[test]
    fn heartbeat_rejects_duplicate_capabilities_and_unsafe_health_detail() {
        let mut heartbeat = Heartbeat {
            device_id: "device-1".to_owned(),
            state: DeviceState::Online,
            active_execution_id: None,
            agent_version: "1.0.0".to_owned(),
            capabilities: vec![
                DeviceCapability::SystemInformation,
                DeviceCapability::SystemInformation,
            ],
            health_detail: None,
        };
        assert_eq!(
            heartbeat.validate(),
            Err(ProtocolError::InvalidField("capabilities"))
        );

        heartbeat.capabilities.truncate(1);
        heartbeat.health_detail = Some("x".repeat(257));
        assert_eq!(
            heartbeat.validate(),
            Err(ProtocolError::InvalidField("health_detail"))
        );
    }

    #[test]
    fn command_rejects_expired_leases() {
        assert_eq!(command().validate(2_000), Err(ProtocolError::ExpiredLease));
    }

    #[test]
    fn action_serialization_is_stable_and_tagged() {
        let value = serde_json::to_value(DesktopAction::ReadSystemInformation).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "kind": "read_system_information" })
        );
    }

    #[test]
    fn risk_classification_is_explicit() {
        assert_eq!(
            DesktopAction::ReadSystemInformation.risk_level(),
            RiskLevel::Low
        );
        assert_eq!(
            DesktopAction::LaunchApplication {
                application_id: "calculator".to_owned()
            }
            .risk_level(),
            RiskLevel::High
        );
    }

    #[test]
    fn selectors_must_be_bounded_and_semantic() {
        let mut command = command();
        command.action = DesktopAction::ClickElement {
            selector: ElementSelector {
                window: WindowSelector {
                    executable: None,
                    title: None,
                    automation_id: None,
                },
                automation_id: None,
                name: None,
                control_type: None,
            },
        };

        assert_eq!(
            command.validate(1_500),
            Err(ProtocolError::MissingField("action.window_selector"))
        );
    }
}
