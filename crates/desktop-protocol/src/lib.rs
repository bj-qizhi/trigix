use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

pub const PROTOCOL_VERSION: &str = "desktop.v1";
pub const CURRENT_PROTOCOL_REVISION: u16 = 2;
pub const PREVIOUS_PROTOCOL_REVISION: u16 = 1;
pub const MAX_INSPECTION_DEPTH: u8 = 8;
pub const MAX_INSPECTION_WINDOWS: u16 = 64;
pub const MAX_INSPECTION_ELEMENTS: u16 = 256;
pub const MAX_INSPECTION_DURATION_MS: u32 = 5_000;
pub const MAX_INSPECTION_PAYLOAD_BYTES: u32 = 60 * 1024;
pub const MIN_VISUAL_SUGGESTION_CONFIDENCE_BPS: u16 = 9_000;
pub const MAX_VISUAL_SUGGESTION_AGE_MS: u64 = 30_000;
pub const VOICE_EVENT_SCHEMA_VERSION: u16 = 1;
pub const MAX_VOICE_TRANSCRIPT_BYTES: usize = 16_384;
pub const MAX_VOICE_LATENCY_MS: u32 = 60_000;

fn previous_protocol_revision() -> u16 {
    PREVIOUS_PROTOCOL_REVISION
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    UnsupportedVersion(String),
    UnsupportedRevision(u16),
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
            Self::UnsupportedRevision(revision) => {
                write!(formatter, "unsupported protocol revision: {revision}")
            }
            Self::MissingField(field) => write!(formatter, "missing required field: {field}"),
            Self::InvalidField(field) => write!(formatter, "invalid field: {field}"),
            Self::ExpiredLease => formatter.write_str("execution lease has expired"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    pub protocol_version: String,
    #[serde(default = "previous_protocol_revision")]
    pub protocol_revision: u16,
    pub message_id: String,
    pub sent_at_unix_ms: u64,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(message_id: impl Into<String>, sent_at_unix_ms: u64, payload: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_owned(),
            protocol_revision: CURRENT_PROTOCOL_REVISION,
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
        if !matches!(
            self.protocol_revision,
            PREVIOUS_PROTOCOL_REVISION | CURRENT_PROTOCOL_REVISION
        ) {
            return Err(ProtocolError::UnsupportedRevision(self.protocol_revision));
        }
        validate_identifier("message_id", &self.message_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct PairingAccepted {
    pub device_id: String,
    pub tenant_id: String,
    pub credential_reference: String,
    pub heartbeat_interval_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct Heartbeat {
    pub device_id: String,
    pub state: DeviceState,
    #[serde(default)]
    pub active_execution_id: Option<String>,
    pub agent_version: String,
    pub capabilities: Vec<DeviceCapability>,
    #[serde(default)]
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
#[serde(deny_unknown_fields)]
pub struct HeartbeatAccepted {
    pub device_id: String,
    pub session_id: String,
    pub state: DeviceState,
    pub server_time_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceSessionState {
    RequestingPermission,
    Listening,
    Processing,
    Speaking,
    Interrupted,
    Reconnecting,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceInterruptionReason {
    UserSpeech,
    UserStop,
    InputDeviceChanged,
    OutputDeviceChanged,
    SessionHidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceStopReason {
    UserStop,
    PermissionRevoked,
    InputDeviceEnded,
    SessionHidden,
    PageTeardown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceFailureCategory {
    PermissionDenied,
    InputUnavailable,
    OutputUnavailable,
    ProviderUnavailable,
    NetworkUnavailable,
    ProtocolViolation,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceSessionSnapshot {
    pub schema_version: u16,
    pub session_id: String,
    pub started_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub state: VoiceSessionState,
    pub last_sequence: u32,
}

impl VoiceSessionSnapshot {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != VOICE_EVENT_SCHEMA_VERSION {
            return Err(ProtocolError::InvalidField("voice_session.schema_version"));
        }
        validate_identifier("voice_session.session_id", &self.session_id)?;
        if self.started_at_unix_ms == 0 || self.updated_at_unix_ms < self.started_at_unix_ms {
            return Err(ProtocolError::InvalidField("voice_session.timestamp"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum VoiceConversationEventKind {
    PermissionRequested,
    PermissionGranted,
    ListeningStarted,
    SpeechStarted,
    SpeechEnded,
    TranscriptPartial { text: String },
    TranscriptFinal { text: String },
    ProcessingStarted,
    SpeakingStarted,
    Interrupted { reason: VoiceInterruptionReason },
    Reconnecting { attempt: u8, delay_ms: u32 },
    Stopped { reason: VoiceStopReason },
    Failed { category: VoiceFailureCategory },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceConversationEvent {
    pub schema_version: u16,
    pub session_id: String,
    pub sequence: u32,
    pub occurred_at_unix_ms: u64,
    pub event: VoiceConversationEventKind,
}

impl VoiceConversationEvent {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != VOICE_EVENT_SCHEMA_VERSION {
            return Err(ProtocolError::InvalidField("voice.schema_version"));
        }
        validate_identifier("voice.session_id", &self.session_id)?;
        if self.sequence == 0 {
            return Err(ProtocolError::InvalidField("voice.sequence"));
        }
        if self.occurred_at_unix_ms == 0 {
            return Err(ProtocolError::InvalidField("voice.occurred_at_unix_ms"));
        }
        match &self.event {
            VoiceConversationEventKind::TranscriptPartial { text }
            | VoiceConversationEventKind::TranscriptFinal { text } => {
                validate_text("voice.transcript", text, MAX_VOICE_TRANSCRIPT_BYTES)
            }
            VoiceConversationEventKind::Reconnecting { attempt, delay_ms } => {
                if !(1..=8).contains(attempt) || !(100..=30_000).contains(delay_ms) {
                    return Err(ProtocolError::InvalidField("voice.reconnect"));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn validate_after(&self, previous: &Self) -> Result<(), ProtocolError> {
        previous.validate()?;
        self.validate()?;
        if self.session_id != previous.session_id {
            return Err(ProtocolError::InvalidField("voice.session_id"));
        }
        if previous.sequence.checked_add(1) != Some(self.sequence) {
            return Err(ProtocolError::InvalidField("voice.sequence"));
        }
        if self.occurred_at_unix_ms < previous.occurred_at_unix_ms {
            return Err(ProtocolError::InvalidField("voice.occurred_at_unix_ms"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceLatencyTelemetry {
    pub schema_version: u16,
    pub session_id: String,
    pub sequence: u32,
    pub captured_at_unix_ms: u64,
    pub state: VoiceSessionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech_start_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_transcript_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_audio_ms: Option<u32>,
}

impl VoiceLatencyTelemetry {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != VOICE_EVENT_SCHEMA_VERSION {
            return Err(ProtocolError::InvalidField(
                "voice_telemetry.schema_version",
            ));
        }
        validate_identifier("voice_telemetry.session_id", &self.session_id)?;
        if self.sequence == 0 {
            return Err(ProtocolError::InvalidField("voice_telemetry.sequence"));
        }
        if self.captured_at_unix_ms == 0 {
            return Err(ProtocolError::InvalidField(
                "voice_telemetry.captured_at_unix_ms",
            ));
        }
        if [
            self.speech_start_ms,
            self.final_transcript_ms,
            self.first_audio_ms,
        ]
        .into_iter()
        .flatten()
        .any(|latency| latency > MAX_VOICE_LATENCY_MS)
        {
            return Err(ProtocolError::InvalidField("voice_telemetry.latency_ms"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardModifier {
    Control,
    Alt,
    Shift,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

fn default_click_count() -> u8 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesktopAction {
    ReadSystemInformation,
    InspectTargets {
        request: Box<DesktopInspectionRequest>,
    },
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
    PressKey {
        selector: WindowSelector,
        key: String,
        #[serde(default)]
        modifiers: Vec<KeyboardModifier>,
    },
    PointerClick {
        selector: ElementSelector,
        button: PointerButton,
        #[serde(default = "default_click_count")]
        click_count: u8,
    },
    LaunchApplication {
        application_id: ApplicationIdentity,
    },
}

impl DesktopAction {
    pub fn risk_level(&self) -> RiskLevel {
        match self {
            Self::ReadSystemInformation | Self::InspectTargets { .. } => RiskLevel::Low,
            Self::FocusWindow { .. } | Self::ClickElement { .. } => RiskLevel::Medium,
            Self::TypeText { .. }
            | Self::PressKey { .. }
            | Self::PointerClick { .. }
            | Self::LaunchApplication { .. } => RiskLevel::High,
        }
    }

    pub fn capability(&self) -> DeviceCapability {
        match self {
            Self::ReadSystemInformation => DeviceCapability::SystemInformation,
            Self::InspectTargets { .. } => DeviceCapability::UiAutomation,
            Self::FocusWindow { .. } | Self::LaunchApplication { .. } => {
                DeviceCapability::WindowManagement
            }
            Self::ClickElement { .. } | Self::TypeText { .. } => DeviceCapability::UiAutomation,
            Self::PressKey { .. } => DeviceCapability::KeyboardInput,
            Self::PointerClick { .. } => DeviceCapability::PointerInput,
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::ReadSystemInformation => Ok(()),
            Self::InspectTargets { request } => request.validate(),
            Self::FocusWindow { selector } => selector.validate(),
            Self::ClickElement { selector } => selector.validate(),
            Self::TypeText { selector, text } => {
                selector.validate()?;
                validate_text("action.text", text, 16_384)
            }
            Self::PressKey {
                selector,
                key,
                modifiers,
            } => {
                selector.validate()?;
                validate_keyboard_key(key)?;
                if modifiers.len() > 4 {
                    return Err(ProtocolError::InvalidField("action.modifiers"));
                }
                for (index, modifier) in modifiers.iter().enumerate() {
                    if modifiers[..index].contains(modifier) {
                        return Err(ProtocolError::InvalidField("action.modifiers"));
                    }
                }
                Ok(())
            }
            Self::PointerClick {
                selector,
                click_count,
                ..
            } => {
                selector.validate()?;
                if !(1..=2).contains(click_count) {
                    return Err(ProtocolError::InvalidField("action.click_count"));
                }
                Ok(())
            }
            Self::LaunchApplication { application_id } => application_id.validate(),
        }
    }
}

fn validate_keyboard_key(key: &str) -> Result<(), ProtocolError> {
    let named = matches!(
        key,
        "enter"
            | "escape"
            | "tab"
            | "backspace"
            | "delete"
            | "space"
            | "home"
            | "end"
            | "page_up"
            | "page_down"
            | "arrow_up"
            | "arrow_down"
            | "arrow_left"
            | "arrow_right"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    );
    let single_ascii = key.len() == 1
        && key
            .as_bytes()
            .first()
            .is_some_and(|value| value.is_ascii_lowercase() || value.is_ascii_digit());
    if named || single_ascii {
        Ok(())
    } else {
        Err(ProtocolError::InvalidField("action.key"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSelector {
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub automation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
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
        )?;
        validate_optional_text(
            "action.window.snapshot_id",
            self.snapshot_id.as_deref(),
            128,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApplicationIdentity(pub String);

impl ApplicationIdentity {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_text("action.application_id", &self.0, 128)?;
        if !self.0.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        }) {
            return Err(ProtocolError::InvalidField("action.application_id"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementSelector {
    pub window: WindowSelector,
    #[serde(default)]
    pub automation_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub control_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticSelectorStrategy {
    WindowAutomationId,
    ExecutableAndTitle,
    Executable,
    Title,
    AutomationId,
    ControlTypeAndName,
    ControlType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectorResolutionTelemetry {
    pub strategy: SemanticSelectorStrategy,
    pub fallback_depth: u8,
    pub fallback_used: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualSelectorSuggestion {
    pub selector: ElementSelector,
    pub snapshot_id: String,
    pub confidence_basis_points: u16,
    pub candidate_count: u8,
    pub observed_at_unix_ms: u64,
}

impl VisualSelectorSuggestion {
    pub fn validate_at(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.selector.validate()?;
        validate_identifier("action.visual_suggestion.snapshot_id", &self.snapshot_id)?;
        if self.selector.window.snapshot_id.as_deref() != Some(&self.snapshot_id) {
            return Err(ProtocolError::InvalidField(
                "action.visual_suggestion.snapshot_id",
            ));
        }
        if !(MIN_VISUAL_SUGGESTION_CONFIDENCE_BPS..=10_000).contains(&self.confidence_basis_points)
        {
            return Err(ProtocolError::InvalidField(
                "action.visual_suggestion.confidence_basis_points",
            ));
        }
        if self.candidate_count != 1 {
            return Err(ProtocolError::InvalidField(
                "action.visual_suggestion.candidate_count",
            ));
        }
        if self.observed_at_unix_ms > now_unix_ms
            || now_unix_ms.saturating_sub(self.observed_at_unix_ms) > MAX_VISUAL_SUGGESTION_AGE_MS
        {
            return Err(ProtocolError::InvalidField(
                "action.visual_suggestion.observed_at_unix_ms",
            ));
        }
        Ok(())
    }
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
#[serde(deny_unknown_fields)]
pub struct DesktopInspectionRequest {
    #[serde(default)]
    pub window: Option<WindowSelector>,
    #[serde(default)]
    pub expected_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_suggestion: Option<VisualSelectorSuggestion>,
    pub max_depth: u8,
    pub max_windows: u16,
    pub max_elements: u16,
    pub max_duration_ms: u32,
    pub max_payload_bytes: u32,
}

impl DesktopInspectionRequest {
    pub fn bounded(window: Option<WindowSelector>) -> Self {
        Self {
            window,
            expected_snapshot_id: None,
            visual_suggestion: None,
            max_depth: MAX_INSPECTION_DEPTH,
            max_windows: MAX_INSPECTION_WINDOWS,
            max_elements: MAX_INSPECTION_ELEMENTS,
            max_duration_ms: MAX_INSPECTION_DURATION_MS,
            max_payload_bytes: MAX_INSPECTION_PAYLOAD_BYTES,
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if let Some(window) = &self.window {
            window.validate()?;
        }
        validate_optional_text(
            "action.inspection.expected_snapshot_id",
            self.expected_snapshot_id.as_deref(),
            128,
        )?;
        if let Some(suggestion) = &self.visual_suggestion {
            suggestion.selector.validate()?;
            validate_identifier(
                "action.visual_suggestion.snapshot_id",
                &suggestion.snapshot_id,
            )?;
            if suggestion.selector.window.snapshot_id.as_deref() != Some(&suggestion.snapshot_id)
                || self.window.is_some()
                || self
                    .expected_snapshot_id
                    .as_ref()
                    .is_some_and(|expected| expected != &suggestion.snapshot_id)
                || !(MIN_VISUAL_SUGGESTION_CONFIDENCE_BPS..=10_000)
                    .contains(&suggestion.confidence_basis_points)
                || suggestion.candidate_count != 1
            {
                return Err(ProtocolError::InvalidField("action.visual_suggestion"));
            }
        }
        if self.max_depth == 0 || self.max_depth > MAX_INSPECTION_DEPTH {
            return Err(ProtocolError::InvalidField("action.inspection.max_depth"));
        }
        if self.max_windows == 0 || self.max_windows > MAX_INSPECTION_WINDOWS {
            return Err(ProtocolError::InvalidField("action.inspection.max_windows"));
        }
        if self.max_elements == 0 || self.max_elements > MAX_INSPECTION_ELEMENTS {
            return Err(ProtocolError::InvalidField(
                "action.inspection.max_elements",
            ));
        }
        if self.max_duration_ms == 0 || self.max_duration_ms > MAX_INSPECTION_DURATION_MS {
            return Err(ProtocolError::InvalidField(
                "action.inspection.max_duration_ms",
            ));
        }
        if self.max_payload_bytes == 0 || self.max_payload_bytes > MAX_INSPECTION_PAYLOAD_BYTES {
            return Err(ProtocolError::InvalidField(
                "action.inspection.max_payload_bytes",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowTitlePolicy {
    Exact,
    Redacted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationPattern {
    Invoke,
    Value,
    Text,
    Selection,
    Toggle,
    ExpandCollapse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionReason {
    Password,
    Credential,
    Oversized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectedElement {
    pub selector: ElementSelector,
    pub depth: u8,
    pub supported_patterns: Vec<AutomationPattern>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redaction: Option<RedactionReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectedWindow {
    pub selector: WindowSelector,
    pub process_id: u32,
    pub title_policy: WindowTitlePolicy,
    pub elements: Vec<InspectedElement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopInspectionResult {
    pub snapshot_id: String,
    pub windows: Vec<InspectedWindow>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector_resolution: Option<SelectorResolutionTelemetry>,
}

impl DesktopInspectionResult {
    pub fn validate(&self, request: &DesktopInspectionRequest) -> Result<(), ProtocolError> {
        request.validate()?;
        validate_identifier("inspection.snapshot_id", &self.snapshot_id)?;
        if self
            .selector_resolution
            .as_ref()
            .is_some_and(|resolution| resolution.fallback_used != (resolution.fallback_depth > 0))
        {
            return Err(ProtocolError::InvalidField(
                "inspection.selector_resolution",
            ));
        }
        match (&request.visual_suggestion, &self.selector_resolution) {
            (None, None) => {}
            (Some(suggestion), Some(resolution)) => {
                if self.snapshot_id != suggestion.snapshot_id
                    || self.windows.len() != 1
                    || self.windows[0].elements.len() != 1
                {
                    return Err(ProtocolError::InvalidField("inspection.visual_suggestion"));
                }
                let window = &self.windows[0].selector;
                let element = &self.windows[0].elements[0].selector;
                let matches_strategy = match resolution.strategy {
                    SemanticSelectorStrategy::WindowAutomationId => {
                        window.automation_id == suggestion.selector.window.automation_id
                    }
                    SemanticSelectorStrategy::ExecutableAndTitle => {
                        window.executable == suggestion.selector.window.executable
                            && window.title == suggestion.selector.window.title
                    }
                    SemanticSelectorStrategy::Executable => {
                        window.executable == suggestion.selector.window.executable
                    }
                    SemanticSelectorStrategy::Title => {
                        window.title == suggestion.selector.window.title
                    }
                    SemanticSelectorStrategy::AutomationId => {
                        element.automation_id == suggestion.selector.automation_id
                    }
                    SemanticSelectorStrategy::ControlTypeAndName => {
                        element.control_type == suggestion.selector.control_type
                            && element.name == suggestion.selector.name
                    }
                    SemanticSelectorStrategy::ControlType => {
                        element.control_type == suggestion.selector.control_type
                    }
                };
                if !matches_strategy {
                    return Err(ProtocolError::InvalidField(
                        "inspection.selector_resolution",
                    ));
                }
            }
            _ => {
                return Err(ProtocolError::InvalidField(
                    "inspection.selector_resolution",
                ));
            }
        }
        if self.windows.len() > request.max_windows as usize {
            return Err(ProtocolError::InvalidField("inspection.windows"));
        }
        let mut element_count = 0usize;
        for window in &self.windows {
            window.selector.validate()?;
            if window.process_id == 0 {
                return Err(ProtocolError::InvalidField("inspection.process_id"));
            }
            if window.title_policy == WindowTitlePolicy::Redacted && window.selector.title.is_some()
            {
                return Err(ProtocolError::InvalidField("inspection.window.title"));
            }
            for element in &window.elements {
                element_count += 1;
                element.selector.validate()?;
                if element.depth == 0 || element.depth > request.max_depth {
                    return Err(ProtocolError::InvalidField("inspection.element.depth"));
                }
                if element.value.is_some() && element.redaction.is_some() {
                    return Err(ProtocolError::InvalidField("inspection.element.value"));
                }
                validate_optional_text("inspection.element.value", element.value.as_deref(), 512)?;
                if element.supported_patterns.len() > 16 {
                    return Err(ProtocolError::InvalidField(
                        "inspection.element.supported_patterns",
                    ));
                }
                for (index, pattern) in element.supported_patterns.iter().enumerate() {
                    if element.supported_patterns[..index].contains(pattern) {
                        return Err(ProtocolError::InvalidField(
                            "inspection.element.supported_patterns",
                        ));
                    }
                }
            }
        }
        if element_count > request.max_elements as usize {
            return Err(ProtocolError::InvalidField("inspection.elements"));
        }
        let payload_size = serde_json::to_vec(self)
            .map_err(|_| ProtocolError::InvalidField("inspection.payload"))?
            .len();
        if payload_size > request.max_payload_bytes as usize {
            return Err(ProtocolError::InvalidField("inspection.payload"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLease {
    pub lease_id: String,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopCommand {
    pub command_id: String,
    pub execution_id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub requested_by: String,
    pub issued_at_unix_ms: u64,
    pub lease: ExecutionLease,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<DesktopCommandApproval>,
    pub action: DesktopAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopCommandApproval {
    pub approved_by: String,
    pub expires_at_unix_ms: u64,
}

impl DesktopCommandApproval {
    pub fn validate(
        &self,
        now_unix_ms: u64,
        lease_expires_at_unix_ms: u64,
    ) -> Result<(), ProtocolError> {
        validate_identifier("approval.approved_by", &self.approved_by)?;
        if self.expires_at_unix_ms <= now_unix_ms
            || self.expires_at_unix_ms > lease_expires_at_unix_ms
        {
            return Err(ProtocolError::InvalidField("approval.expires_at_unix_ms"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopCommandAcknowledgement {
    pub command_id: String,
    pub execution_id: String,
    pub lease_id: String,
    pub acknowledged_at_unix_ms: u64,
}

impl DesktopCommandAcknowledgement {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_identifier("command_id", &self.command_id)?;
        validate_identifier("execution_id", &self.execution_id)?;
        validate_identifier("lease_id", &self.lease_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopCommandCancellation {
    pub command_id: String,
    pub execution_id: String,
    pub reason: String,
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
        if let Some(approval) = &self.approval {
            approval.validate(now_unix_ms, self.lease.expires_at_unix_ms)?;
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
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopCommandResult {
    pub command_id: String,
    pub execution_id: String,
    pub outcome: CommandOutcome,
    pub completed_at_unix_ms: u64,
    #[serde(default)]
    pub output: Option<Value>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopProtocolFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
}

impl DesktopProtocolFailure {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_text("error.code", &self.code, 128)?;
        validate_text("error.message", &self.message, 2048)?;
        if let Some(command_id) = &self.command_id {
            validate_identifier("error.command_id", command_id)?;
        }
        Ok(())
    }
}

impl DesktopCommandResult {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_identifier("command_id", &self.command_id)?;
        validate_identifier("execution_id", &self.execution_id)?;
        validate_optional_text("error_code", self.error_code.as_deref(), 128)?;
        validate_optional_text("error_message", self.error_message.as_deref(), 2048)?;
        if matches!(self.outcome, CommandOutcome::Succeeded) && self.error_code.is_some() {
            return Err(ProtocolError::InvalidField("error_code"));
        }
        if matches!(
            self.outcome,
            CommandOutcome::Failed | CommandOutcome::Rejected
        ) && self.error_code.is_none()
        {
            return Err(ProtocolError::MissingField("error_code"));
        }
        Ok(())
    }
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
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;

    const CURRENT_PAIRING: &str = include_str!("../fixtures/current/pairing.json");
    const CURRENT_HEARTBEAT: &str = include_str!("../fixtures/current/heartbeat.json");
    const CURRENT_COMMAND: &str = include_str!("../fixtures/current/command.json");
    const CURRENT_RESULT: &str = include_str!("../fixtures/current/result.json");
    const CURRENT_ERROR: &str = include_str!("../fixtures/current/error.json");
    const CURRENT_APPROVAL: &str = include_str!("../fixtures/current/approval.json");
    const PREVIOUS_PAIRING: &str = include_str!("../fixtures/previous/pairing.json");
    const PREVIOUS_HEARTBEAT: &str = include_str!("../fixtures/previous/heartbeat.json");
    const PREVIOUS_COMMAND: &str = include_str!("../fixtures/previous/command.json");
    const PREVIOUS_RESULT: &str = include_str!("../fixtures/previous/result.json");
    const PREVIOUS_ERROR: &str = include_str!("../fixtures/previous/error.json");
    const PREVIOUS_APPROVAL: &str = include_str!("../fixtures/previous/approval.json");

    fn current_fixture<T>(source: &str) -> Envelope<T>
    where
        T: DeserializeOwned + Serialize + PartialEq + Debug,
    {
        let envelope: Envelope<T> = serde_json::from_str(source).unwrap();
        envelope.validate().unwrap();
        assert_eq!(envelope.protocol_revision, CURRENT_PROTOCOL_REVISION);
        assert_eq!(
            format!("{}\n", serde_json::to_string_pretty(&envelope).unwrap()),
            source
        );
        envelope
    }

    fn previous_fixture<T>(source: &str) -> Envelope<T>
    where
        T: DeserializeOwned + Serialize + PartialEq + Debug,
    {
        let envelope: Envelope<T> = serde_json::from_str(source).unwrap();
        envelope.validate().unwrap();
        assert_eq!(envelope.protocol_revision, PREVIOUS_PROTOCOL_REVISION);
        let round_trip: Envelope<T> =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(round_trip, envelope);
        envelope
    }

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
            approval: None,
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

    fn voice_event(event: VoiceConversationEventKind) -> VoiceConversationEvent {
        VoiceConversationEvent {
            schema_version: VOICE_EVENT_SCHEMA_VERSION,
            session_id: "voice-session-1".to_owned(),
            sequence: 1,
            occurred_at_unix_ms: 1_000,
            event,
        }
    }

    #[test]
    fn voice_events_are_versioned_provider_neutral_and_bounded() {
        let mut session = VoiceSessionSnapshot {
            schema_version: VOICE_EVENT_SCHEMA_VERSION,
            session_id: "voice-session-1".to_owned(),
            started_at_unix_ms: 1_000,
            updated_at_unix_ms: 1_000,
            state: VoiceSessionState::RequestingPermission,
            last_sequence: 0,
        };
        session.validate().unwrap();
        session.updated_at_unix_ms = 999;
        assert_eq!(
            session.validate(),
            Err(ProtocolError::InvalidField("voice_session.timestamp"))
        );

        let events = [
            VoiceConversationEventKind::PermissionRequested,
            VoiceConversationEventKind::PermissionGranted,
            VoiceConversationEventKind::ListeningStarted,
            VoiceConversationEventKind::SpeechStarted,
            VoiceConversationEventKind::SpeechEnded,
            VoiceConversationEventKind::ProcessingStarted,
            VoiceConversationEventKind::SpeakingStarted,
            VoiceConversationEventKind::Interrupted {
                reason: VoiceInterruptionReason::UserSpeech,
            },
            VoiceConversationEventKind::Reconnecting {
                attempt: 1,
                delay_ms: 100,
            },
            VoiceConversationEventKind::Stopped {
                reason: VoiceStopReason::UserStop,
            },
            VoiceConversationEventKind::Failed {
                category: VoiceFailureCategory::PermissionDenied,
            },
        ];
        for event in events {
            voice_event(event).validate().unwrap();
        }

        let previous = voice_event(VoiceConversationEventKind::PermissionRequested);
        let mut next = voice_event(VoiceConversationEventKind::PermissionGranted);
        next.sequence = 2;
        next.occurred_at_unix_ms = 1_001;
        next.validate_after(&previous).unwrap();
        next.sequence = 3;
        assert_eq!(
            next.validate_after(&previous),
            Err(ProtocolError::InvalidField("voice.sequence"))
        );
        next.sequence = 2;
        next.occurred_at_unix_ms = 999;
        assert_eq!(
            next.validate_after(&previous),
            Err(ProtocolError::InvalidField("voice.occurred_at_unix_ms"))
        );

        let mut reconnecting = voice_event(VoiceConversationEventKind::Reconnecting {
            attempt: 9,
            delay_ms: 100,
        });
        assert_eq!(
            reconnecting.validate(),
            Err(ProtocolError::InvalidField("voice.reconnect"))
        );
        reconnecting.event = VoiceConversationEventKind::TranscriptFinal {
            text: "x".repeat(MAX_VOICE_TRANSCRIPT_BYTES + 1),
        };
        assert_eq!(
            reconnecting.validate(),
            Err(ProtocolError::InvalidField("voice.transcript"))
        );
    }

    #[test]
    fn voice_telemetry_excludes_content_and_rejects_unbounded_latency() {
        let mut telemetry = VoiceLatencyTelemetry {
            schema_version: VOICE_EVENT_SCHEMA_VERSION,
            session_id: "voice-session-1".to_owned(),
            sequence: 2,
            captured_at_unix_ms: 1_500,
            state: VoiceSessionState::Listening,
            speech_start_ms: Some(120),
            final_transcript_ms: None,
            first_audio_ms: None,
        };
        telemetry.validate().unwrap();
        let serialized = serde_json::to_value(&telemetry).unwrap();
        assert!(serialized.get("text").is_none());
        assert!(serialized.get("transcript").is_none());
        assert!(serialized.get("audio").is_none());

        telemetry.first_audio_ms = Some(MAX_VOICE_LATENCY_MS + 1);
        assert_eq!(
            telemetry.validate(),
            Err(ProtocolError::InvalidField("voice_telemetry.latency_ms"))
        );
    }

    #[test]
    fn voice_event_cannot_embed_desktop_execution_authority() {
        let value = serde_json::json!({
            "schema_version": VOICE_EVENT_SCHEMA_VERSION,
            "session_id": "voice-session-1",
            "sequence": 1,
            "occurred_at_unix_ms": 1_000,
            "event": {
                "kind": "listening_started",
                "desktop_action": { "kind": "read_system_information" }
            }
        });
        assert!(serde_json::from_value::<VoiceConversationEvent>(value).is_err());
    }

    #[test]
    fn command_rejects_expired_leases() {
        assert_eq!(command().validate(2_000), Err(ProtocolError::ExpiredLease));
    }

    #[test]
    fn command_approval_must_be_bounded_by_the_lease() {
        let mut value = command();
        value.approval = Some(DesktopCommandApproval {
            approved_by: "admin-1".to_string(),
            expires_at_unix_ms: 2_001,
        });
        assert_eq!(
            value.validate(1_500),
            Err(ProtocolError::InvalidField("approval.expires_at_unix_ms"))
        );
    }

    #[test]
    fn acknowledgement_and_result_require_consistent_bounded_fields() {
        let mut acknowledgement = DesktopCommandAcknowledgement {
            command_id: "command-1".to_string(),
            execution_id: "execution-1".to_string(),
            lease_id: "lease-1".to_string(),
            acknowledged_at_unix_ms: 1_500,
        };
        assert!(acknowledgement.validate().is_ok());
        acknowledgement.lease_id.clear();
        assert_eq!(
            acknowledgement.validate(),
            Err(ProtocolError::MissingField("lease_id"))
        );

        let failed = DesktopCommandResult {
            command_id: "command-1".to_string(),
            execution_id: "execution-1".to_string(),
            outcome: CommandOutcome::Failed,
            completed_at_unix_ms: 1_600,
            output: None,
            error_code: None,
            error_message: Some("failed".to_string()),
        };
        assert_eq!(
            failed.validate(),
            Err(ProtocolError::MissingField("error_code"))
        );
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
                application_id: ApplicationIdentity::new("calculator")
            }
            .risk_level(),
            RiskLevel::High
        );
        assert_eq!(
            DesktopAction::PressKey {
                selector: WindowSelector {
                    executable: Some("fixture.exe".to_owned()),
                    title: None,
                    automation_id: None,
                    snapshot_id: None,
                },
                key: "enter".to_owned(),
                modifiers: vec![],
            }
            .capability(),
            DeviceCapability::KeyboardInput
        );
    }

    #[test]
    fn input_actions_reject_unbounded_or_ambiguous_values() {
        let window = WindowSelector {
            executable: Some("fixture.exe".to_owned()),
            title: None,
            automation_id: None,
            snapshot_id: None,
        };
        let mut key = DesktopAction::PressKey {
            selector: window.clone(),
            key: "enter".to_owned(),
            modifiers: vec![KeyboardModifier::Control],
        };
        assert!(key.validate().is_ok());
        if let DesktopAction::PressKey { key, modifiers, .. } = &mut key {
            *key = "unbounded_key_name".to_owned();
            assert_eq!(
                DesktopAction::PressKey {
                    selector: window.clone(),
                    key: key.clone(),
                    modifiers: modifiers.clone(),
                }
                .validate(),
                Err(ProtocolError::InvalidField("action.key"))
            );
            *key = "a".to_owned();
            modifiers.push(KeyboardModifier::Control);
        }
        assert_eq!(
            key.validate(),
            Err(ProtocolError::InvalidField("action.modifiers"))
        );

        let mut pointer = DesktopAction::PointerClick {
            selector: ElementSelector {
                window,
                automation_id: Some("submit".to_owned()),
                name: None,
                control_type: Some("button".to_owned()),
            },
            button: PointerButton::Left,
            click_count: 2,
        };
        assert!(pointer.validate().is_ok());
        if let DesktopAction::PointerClick { click_count, .. } = &mut pointer {
            *click_count = 3;
        }
        assert_eq!(
            pointer.validate(),
            Err(ProtocolError::InvalidField("action.click_count"))
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
                    snapshot_id: None,
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

    #[test]
    fn visual_suggestions_are_fresh_unique_semantic_references() {
        let suggestion = VisualSelectorSuggestion {
            selector: ElementSelector {
                window: WindowSelector {
                    executable: Some("fixture.exe".to_owned()),
                    title: Some("Fixture".to_owned()),
                    automation_id: Some("Fixture.Main".to_owned()),
                    snapshot_id: Some("snapshot-1".to_owned()),
                },
                automation_id: Some("missing-primary".to_owned()),
                name: Some("Submit".to_owned()),
                control_type: Some("button".to_owned()),
            },
            snapshot_id: "snapshot-1".to_owned(),
            confidence_basis_points: MIN_VISUAL_SUGGESTION_CONFIDENCE_BPS,
            candidate_count: 1,
            observed_at_unix_ms: 1_000,
        };
        assert!(suggestion.validate_at(1_001).is_ok());

        let mut invalid = suggestion.clone();
        invalid.candidate_count = 2;
        assert_eq!(
            invalid.validate_at(1_001),
            Err(ProtocolError::InvalidField(
                "action.visual_suggestion.candidate_count"
            ))
        );

        assert_eq!(
            suggestion.validate_at(1_000 + MAX_VISUAL_SUGGESTION_AGE_MS + 1),
            Err(ProtocolError::InvalidField(
                "action.visual_suggestion.observed_at_unix_ms"
            ))
        );

        let mut encoded = serde_json::to_value(&suggestion).unwrap();
        encoded["x"] = serde_json::json!(100);
        encoded["y"] = serde_json::json!(200);
        assert!(serde_json::from_value::<VisualSelectorSuggestion>(encoded).is_err());
    }

    #[test]
    fn inspection_requests_enforce_all_resource_limits() {
        let mut request = DesktopInspectionRequest::bounded(None);
        assert!(request.validate().is_ok());
        request.max_depth = MAX_INSPECTION_DEPTH + 1;
        assert_eq!(
            request.validate(),
            Err(ProtocolError::InvalidField("action.inspection.max_depth"))
        );
        request.max_depth = 1;
        request.max_windows = 0;
        assert_eq!(
            request.validate(),
            Err(ProtocolError::InvalidField("action.inspection.max_windows"))
        );
        request.max_windows = 1;
        request.max_elements = 0;
        assert_eq!(
            request.validate(),
            Err(ProtocolError::InvalidField(
                "action.inspection.max_elements"
            ))
        );
        request.max_elements = 1;
        request.max_duration_ms = MAX_INSPECTION_DURATION_MS + 1;
        assert_eq!(
            request.validate(),
            Err(ProtocolError::InvalidField(
                "action.inspection.max_duration_ms"
            ))
        );
        request.max_duration_ms = 1;
        request.max_payload_bytes = MAX_INSPECTION_PAYLOAD_BYTES + 1;
        assert_eq!(
            request.validate(),
            Err(ProtocolError::InvalidField(
                "action.inspection.max_payload_bytes"
            ))
        );
    }

    #[test]
    fn application_identity_cannot_encode_commands_or_arguments() {
        assert!(ApplicationIdentity::new("trigix.calculator")
            .validate()
            .is_ok());
        assert_eq!(
            ApplicationIdentity::new("cmd.exe /c whoami").validate(),
            Err(ProtocolError::InvalidField("action.application_id"))
        );
        assert_eq!(
            ApplicationIdentity::new("C:\\Windows\\System32\\calc.exe").validate(),
            Err(ProtocolError::InvalidField("action.application_id"))
        );
    }

    #[test]
    fn current_release_fixtures_are_byte_stable_and_valid() {
        current_fixture::<PairingRequest>(CURRENT_PAIRING)
            .payload
            .validate()
            .unwrap();
        current_fixture::<Heartbeat>(CURRENT_HEARTBEAT)
            .payload
            .validate()
            .unwrap();
        current_fixture::<DesktopCommand>(CURRENT_COMMAND)
            .payload
            .validate(1_700_000_002_500)
            .unwrap();
        current_fixture::<DesktopCommandResult>(CURRENT_RESULT)
            .payload
            .validate()
            .unwrap();
        current_fixture::<DesktopProtocolFailure>(CURRENT_ERROR)
            .payload
            .validate()
            .unwrap();
        current_fixture::<DesktopCommandApproval>(CURRENT_APPROVAL)
            .payload
            .validate(1_700_000_002_500, 1_700_000_060_000)
            .unwrap();
    }

    #[test]
    fn previous_release_fixtures_remain_semantically_compatible() {
        previous_fixture::<PairingRequest>(PREVIOUS_PAIRING)
            .payload
            .validate()
            .unwrap();
        let heartbeat = previous_fixture::<Heartbeat>(PREVIOUS_HEARTBEAT).payload;
        heartbeat.validate().unwrap();
        assert!(heartbeat.active_execution_id.is_none());
        assert!(heartbeat.health_detail.is_none());
        let command = previous_fixture::<DesktopCommand>(PREVIOUS_COMMAND).payload;
        command.validate(1_699_990_002_500).unwrap();
        assert!(command.approval.is_none());
        let result = previous_fixture::<DesktopCommandResult>(PREVIOUS_RESULT).payload;
        result.validate().unwrap();
        assert!(result.error_code.is_none());
        assert!(result.error_message.is_none());
        previous_fixture::<DesktopProtocolFailure>(PREVIOUS_ERROR)
            .payload
            .validate()
            .unwrap();
        previous_fixture::<DesktopCommandApproval>(PREVIOUS_APPROVAL)
            .payload
            .validate(1_699_990_002_500, 1_699_990_060_000)
            .unwrap();
    }

    #[test]
    fn unknown_revision_action_and_fields_fail_closed() {
        let mut envelope: Envelope<DesktopCommand> = serde_json::from_str(CURRENT_COMMAND).unwrap();
        envelope.protocol_revision = CURRENT_PROTOCOL_REVISION + 1;
        assert_eq!(
            envelope.validate(),
            Err(ProtocolError::UnsupportedRevision(3))
        );

        let mut unknown_action: Value = serde_json::from_str(CURRENT_COMMAND).unwrap();
        unknown_action["payload"]["action"]["kind"] = Value::String("run_script".to_owned());
        assert!(serde_json::from_value::<Envelope<DesktopCommand>>(unknown_action).is_err());

        let mut unsafe_extra_field: Value = serde_json::from_str(CURRENT_COMMAND).unwrap();
        unsafe_extra_field["payload"]["action"]["unrestricted_command"] =
            Value::String("fixture".to_owned());
        assert!(serde_json::from_value::<Envelope<DesktopCommand>>(unsafe_extra_field).is_err());

        let mut unknown_envelope_field: Value = serde_json::from_str(CURRENT_PAIRING).unwrap();
        unknown_envelope_field["credential"] = Value::String("fixture".to_owned());
        assert!(
            serde_json::from_value::<Envelope<PairingRequest>>(unknown_envelope_field).is_err()
        );
    }

    #[test]
    fn unsafe_fixture_values_fail_validation() {
        let mut failure = current_fixture::<DesktopProtocolFailure>(CURRENT_ERROR).payload;
        failure.message = "unsafe\nmessage".to_owned();
        assert_eq!(
            failure.validate(),
            Err(ProtocolError::InvalidField("error.message"))
        );

        let mut pairing = current_fixture::<PairingRequest>(CURRENT_PAIRING).payload;
        pairing.device.display_name = "unsafe\u{0000}name".to_owned();
        assert_eq!(
            pairing.validate(),
            Err(ProtocolError::InvalidField("display_name"))
        );
    }
}
