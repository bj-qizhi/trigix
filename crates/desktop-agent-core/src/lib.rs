use desktop_protocol::{
    CommandOutcome, DesktopAction, DesktopCommand, DesktopCommandResult, DeviceCapability,
    ProtocolError, RiskLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub mod connection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    Protocol(ProtocolError),
    DuplicateCommand(String),
    UnsupportedAction,
    Execution(String),
    Recovery(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(formatter, "protocol error: {error}"),
            Self::DuplicateCommand(command_id) => {
                write!(formatter, "command was already processed: {command_id}")
            }
            Self::UnsupportedAction => {
                formatter.write_str("action is not supported by this executor")
            }
            Self::Execution(message) => write!(formatter, "action execution failed: {message}"),
            Self::Recovery(message) => write!(formatter, "command recovery failed: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<ProtocolError> for CoreError {
    fn from(value: ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    RequireApproval { reason: String },
    Deny { reason: String },
}

#[derive(Debug, Clone)]
pub struct ExecutionPolicy {
    pub maximum_automatic_risk: RiskLevel,
    pub maximum_approvable_risk: RiskLevel,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            maximum_automatic_risk: RiskLevel::Low,
            maximum_approvable_risk: RiskLevel::High,
        }
    }
}

impl ExecutionPolicy {
    pub fn evaluate(&self, action: &DesktopAction) -> PolicyDecision {
        let risk = action.risk_level();
        if risk <= self.maximum_automatic_risk {
            PolicyDecision::Allow
        } else if risk <= self.maximum_approvable_risk {
            PolicyDecision::RequireApproval {
                reason: format!("{risk:?} risk desktop action requires Approval"),
            }
        } else {
            PolicyDecision::Deny {
                reason: format!("{risk:?} risk desktop action is prohibited by policy"),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalGrant {
    pub command_id: String,
    pub approved_by: String,
    pub expires_at_unix_ms: u64,
}

impl ApprovalGrant {
    fn authorizes(&self, command: &DesktopCommand, now_unix_ms: u64) -> bool {
        self.command_id == command.command_id
            && !self.approved_by.trim().is_empty()
            && self.expires_at_unix_ms > now_unix_ms
    }
}

pub trait ActionExecutor {
    fn execute(&mut self, action: &DesktopAction) -> Result<Value, CoreError>;
}

pub trait AuditSink {
    fn record(&mut self, event: AuditEvent);
}

const RECOVERY_SCHEMA_VERSION: u16 = 1;
const DEFAULT_SAFETY_WINDOW_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
const DEFAULT_MAX_RECOVERY_ENTRIES: usize = 10_000;
const DEFAULT_MAX_RECOVERY_FILE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryConfig {
    pub safety_window_ms: u64,
    pub max_entries: usize,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            safety_window_ms: DEFAULT_SAFETY_WINDOW_MS,
            max_entries: DEFAULT_MAX_RECOVERY_ENTRIES,
        }
    }
}

impl RecoveryConfig {
    fn validate(&self) -> Result<(), CoreError> {
        if self.safety_window_ms == 0 || self.max_entries == 0 {
            return Err(CoreError::Recovery(
                "safety window and maximum entries must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletedCommandState {
    pub command_id: String,
    pub execution_id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub actor_id: String,
    pub lease_id: String,
    pub lease_expires_at_unix_ms: u64,
    pub capability: DeviceCapability,
    pub risk_level: RiskLevel,
    pub retained_until_unix_ms: u64,
    pub pending_result: Option<DesktopCommandResult>,
}

impl CompletedCommandState {
    fn matches(&self, command: &DesktopCommand) -> bool {
        self.command_id == command.command_id
            && self.execution_id == command.execution_id
            && self.tenant_id == command.tenant_id
            && self.project_id == command.project_id
            && self.actor_id == command.requested_by
            && self.lease_id == command.lease.lease_id
            && self.lease_expires_at_unix_ms == command.lease.expires_at_unix_ms
            && self.capability == command.action.capability()
            && self.risk_level == command.action.risk_level()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InFlightCommandState {
    pub command_id: String,
    pub execution_id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub actor_id: String,
    pub lease_id: String,
    pub lease_expires_at_unix_ms: u64,
    pub capability: DeviceCapability,
    pub risk_level: RiskLevel,
    pub retry_safe: bool,
    pub started_at_unix_ms: u64,
}

impl InFlightCommandState {
    fn from_command(command: &DesktopCommand, now_unix_ms: u64) -> Self {
        Self {
            command_id: command.command_id.clone(),
            execution_id: command.execution_id.clone(),
            tenant_id: command.tenant_id.clone(),
            project_id: command.project_id.clone(),
            actor_id: command.requested_by.clone(),
            lease_id: command.lease.lease_id.clone(),
            lease_expires_at_unix_ms: command.lease.expires_at_unix_ms,
            capability: command.action.capability(),
            risk_level: command.action.risk_level(),
            retry_safe: action_is_retry_safe(&command.action),
            started_at_unix_ms: now_unix_ms,
        }
    }

    fn matches(&self, command: &DesktopCommand) -> bool {
        self.command_id == command.command_id
            && self.execution_id == command.execution_id
            && self.tenant_id == command.tenant_id
            && self.project_id == command.project_id
            && self.actor_id == command.requested_by
            && self.lease_id == command.lease.lease_id
            && self.lease_expires_at_unix_ms == command.lease.expires_at_unix_ms
            && self.capability == command.action.capability()
            && self.risk_level == command.action.risk_level()
            && self.retry_safe == action_is_retry_safe(&command.action)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedCommandState {
    pub schema_version: u16,
    pub completed: Vec<CompletedCommandState>,
    pub in_flight: Vec<InFlightCommandState>,
}

impl Default for PersistedCommandState {
    fn default() -> Self {
        Self {
            schema_version: RECOVERY_SCHEMA_VERSION,
            completed: Vec::new(),
            in_flight: Vec::new(),
        }
    }
}

pub trait CommandStateStore {
    fn load(&self) -> Result<Option<PersistedCommandState>, CoreError>;
    fn save(&mut self, state: &PersistedCommandState) -> Result<(), CoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct MemoryCommandStateStore {
    state: Arc<Mutex<Option<PersistedCommandState>>>,
}

impl CommandStateStore for MemoryCommandStateStore {
    fn load(&self) -> Result<Option<PersistedCommandState>, CoreError> {
        self.state
            .lock()
            .map_err(|_| CoreError::Recovery("memory state lock is poisoned".to_owned()))
            .map(|state| state.clone())
    }

    fn save(&mut self, state: &PersistedCommandState) -> Result<(), CoreError> {
        *self
            .state
            .lock()
            .map_err(|_| CoreError::Recovery("memory state lock is poisoned".to_owned()))? =
            Some(state.clone());
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileCommandStateStore {
    path: PathBuf,
    maximum_file_bytes: u64,
}

impl FileCommandStateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            maximum_file_bytes: DEFAULT_MAX_RECOVERY_FILE_BYTES,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn backup_path(&self) -> PathBuf {
        self.path.with_extension("recovery-backup")
    }

    fn temporary_path(&self) -> PathBuf {
        self.path.with_extension("recovery-new")
    }
}

impl CommandStateStore for FileCommandStateStore {
    fn load(&self) -> Result<Option<PersistedCommandState>, CoreError> {
        let backup = self.backup_path();
        let path = if self.path.exists() {
            self.path.clone()
        } else if backup.exists() {
            backup
        } else {
            return Ok(None);
        };
        let metadata = fs::metadata(&path).map_err(recovery_io_error)?;
        if metadata.len() > self.maximum_file_bytes {
            return Err(CoreError::Recovery(
                "state file exceeds size limit".to_owned(),
            ));
        }
        let bytes = fs::read(path).map_err(recovery_io_error)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| CoreError::Recovery(format!("state file is corrupt: {error}")))
    }

    fn save(&mut self, state: &PersistedCommandState) -> Result<(), CoreError> {
        let bytes = serde_json::to_vec(state)
            .map_err(|error| CoreError::Recovery(format!("state serialization failed: {error}")))?;
        if bytes.len() as u64 > self.maximum_file_bytes {
            return Err(CoreError::Recovery(
                "state file exceeds size limit".to_owned(),
            ));
        }
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(recovery_io_error)?;
        }
        let temporary = self.temporary_path();
        let backup = self.backup_path();
        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(recovery_io_error)?;
        file.write_all(&bytes).map_err(recovery_io_error)?;
        file.sync_all().map_err(recovery_io_error)?;
        drop(file);

        if backup.exists() && !self.path.exists() {
            fs::rename(&backup, &self.path).map_err(recovery_io_error)?;
        }
        if backup.exists() {
            fs::remove_file(&backup).map_err(recovery_io_error)?;
        }
        if self.path.exists() {
            fs::rename(&self.path, &backup).map_err(recovery_io_error)?;
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            if backup.exists() && !self.path.exists() {
                let _ = fs::rename(&backup, &self.path);
            }
            return Err(recovery_io_error(error));
        }
        if backup.exists() {
            fs::remove_file(backup).map_err(recovery_io_error)?;
        }
        Ok(())
    }
}

fn recovery_io_error(error: std::io::Error) -> CoreError {
    CoreError::Recovery(format!("state storage unavailable: {error}"))
}

fn action_is_retry_safe(action: &DesktopAction) -> bool {
    matches!(action, DesktopAction::ReadSystemInformation)
}

fn recovery_result(
    entry: &InFlightCommandState,
    now_unix_ms: u64,
    outcome: CommandOutcome,
) -> DesktopCommandResult {
    let timed_out = outcome == CommandOutcome::TimedOut;
    DesktopCommandResult {
        command_id: entry.command_id.clone(),
        execution_id: entry.execution_id.clone(),
        outcome,
        completed_at_unix_ms: now_unix_ms,
        output: None,
        error_code: Some(if timed_out {
            "lease_expired_during_recovery".to_owned()
        } else {
            "uncertain_side_effect_blocked".to_owned()
        }),
        error_message: Some(if timed_out {
            "command lease expired before recovery".to_owned()
        } else {
            "command may have started before restart and cannot be retried safely".to_owned()
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryHealthEvent {
    pub event_type: String,
    pub occurred_at_unix_ms: u64,
    pub command_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_type: String,
    pub command_id: String,
    pub execution_id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub actor_id: String,
    pub occurred_at_unix_ms: u64,
    pub risk_level: RiskLevel,
    pub outcome: CommandOutcome,
    pub details: Value,
}

#[derive(Debug, Default)]
pub struct InMemoryAuditSink {
    events: Vec<AuditEvent>,
}

impl InMemoryAuditSink {
    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }
}

impl AuditSink for InMemoryAuditSink {
    fn record(&mut self, event: AuditEvent) {
        self.events.push(event);
    }
}

pub struct CommandProcessor<E, A, S = MemoryCommandStateStore> {
    executor: E,
    audit_sink: A,
    policy: ExecutionPolicy,
    state_store: S,
    recovery_config: RecoveryConfig,
    state: PersistedCommandState,
    processed_commands: HashSet<String>,
    in_flight_commands: HashMap<String, InFlightCommandState>,
    recovery_health_events: Vec<RecoveryHealthEvent>,
}

impl<E: ActionExecutor, A: AuditSink> CommandProcessor<E, A, MemoryCommandStateStore> {
    pub fn new(executor: E, audit_sink: A, policy: ExecutionPolicy) -> Self {
        Self::with_recovery_store(
            executor,
            audit_sink,
            policy,
            MemoryCommandStateStore::default(),
            RecoveryConfig::default(),
            0,
        )
        .expect("the default in-memory recovery store must initialize")
    }
}

impl<E: ActionExecutor, A: AuditSink, S: CommandStateStore> CommandProcessor<E, A, S> {
    pub fn with_recovery_store(
        executor: E,
        audit_sink: A,
        policy: ExecutionPolicy,
        state_store: S,
        recovery_config: RecoveryConfig,
        now_unix_ms: u64,
    ) -> Result<Self, CoreError> {
        recovery_config.validate()?;
        let state = state_store.load()?.unwrap_or_default();
        validate_persisted_state(&state, &recovery_config)?;
        let mut processor = Self {
            executor,
            audit_sink,
            policy,
            state_store,
            recovery_config,
            state,
            processed_commands: HashSet::new(),
            in_flight_commands: HashMap::new(),
            recovery_health_events: Vec::new(),
        };
        processor.recover(now_unix_ms)?;
        Ok(processor)
    }

    pub fn process(
        &mut self,
        command: &DesktopCommand,
        now_unix_ms: u64,
        approval: Option<&ApprovalGrant>,
    ) -> Result<DesktopCommandResult, CoreError> {
        command.validate(now_unix_ms)?;
        self.prune_completed(now_unix_ms)?;
        if let Some(completed) = self
            .state
            .completed
            .iter()
            .find(|entry| entry.command_id == command.command_id)
        {
            if !completed.matches(command) {
                return Err(CoreError::Recovery(
                    "completed command identifier has conflicting metadata".to_owned(),
                ));
            }
            return completed
                .pending_result
                .clone()
                .ok_or_else(|| CoreError::DuplicateCommand(command.command_id.clone()));
        }
        if self.processed_commands.contains(&command.command_id) {
            return Err(CoreError::DuplicateCommand(command.command_id.clone()));
        }

        let recovering = match self.in_flight_commands.get(&command.command_id) {
            Some(in_flight) if in_flight.matches(command) && in_flight.retry_safe => true,
            Some(_) => {
                return Err(CoreError::Recovery(
                    "in-flight command metadata does not permit retry".to_owned(),
                ))
            }
            None => false,
        };

        let decision = self.policy.evaluate(&command.action);
        let result = match decision {
            PolicyDecision::Allow => self.execute_durably(command, now_unix_ms, recovering)?,
            PolicyDecision::RequireApproval { reason } => {
                if approval.is_some_and(|grant| grant.authorizes(command, now_unix_ms)) {
                    self.execute_durably(command, now_unix_ms, recovering)?
                } else {
                    DesktopCommandResult {
                        command_id: command.command_id.clone(),
                        execution_id: command.execution_id.clone(),
                        outcome: CommandOutcome::AwaitingApproval,
                        completed_at_unix_ms: now_unix_ms,
                        output: None,
                        error_code: Some("approval_required".to_owned()),
                        error_message: Some(reason),
                    }
                }
            }
            PolicyDecision::Deny { reason } => DesktopCommandResult {
                command_id: command.command_id.clone(),
                execution_id: command.execution_id.clone(),
                outcome: CommandOutcome::Rejected,
                completed_at_unix_ms: now_unix_ms,
                output: None,
                error_code: Some("policy_denied".to_owned()),
                error_message: Some(reason),
            },
        };

        if result.outcome != CommandOutcome::AwaitingApproval {
            self.persist_completed(command, &result, now_unix_ms)?;
        }
        let event_type = if result.outcome == CommandOutcome::AwaitingApproval {
            "desktop.command.awaiting_approval"
        } else {
            "desktop.command.processed"
        };
        self.audit_sink.record(AuditEvent {
            event_type: event_type.to_owned(),
            command_id: command.command_id.clone(),
            execution_id: command.execution_id.clone(),
            tenant_id: command.tenant_id.clone(),
            project_id: command.project_id.clone(),
            actor_id: command.requested_by.clone(),
            occurred_at_unix_ms: now_unix_ms,
            risk_level: command.action.risk_level(),
            outcome: result.outcome,
            details: json!({
                "capability": command.action.capability(),
                "lease_id": command.lease.lease_id,
                "approved_by": approval.map(|grant| grant.approved_by.as_str()),
            }),
        });
        Ok(result)
    }

    pub fn audit_sink(&self) -> &A {
        &self.audit_sink
    }

    pub fn recovery_health_events(&self) -> &[RecoveryHealthEvent] {
        &self.recovery_health_events
    }

    pub fn take_recovery_health_events(&mut self) -> Vec<RecoveryHealthEvent> {
        std::mem::take(&mut self.recovery_health_events)
    }

    pub fn confirm_result_delivery(&mut self, command_id: &str) -> Result<(), CoreError> {
        let mut next_state = self.state.clone();
        let completed = next_state
            .completed
            .iter_mut()
            .find(|entry| entry.command_id == command_id)
            .ok_or_else(|| CoreError::Recovery("completed command was not found".to_owned()))?;
        if completed.pending_result.is_none() {
            return Ok(());
        }
        completed.pending_result = None;
        self.state_store.save(&next_state)?;
        self.state = next_state;
        Ok(())
    }

    fn recover(&mut self, now_unix_ms: u64) -> Result<(), CoreError> {
        let original_state = self.state.clone();
        self.state
            .completed
            .retain(|entry| entry.retained_until_unix_ms > now_unix_ms);
        let recovered = std::mem::take(&mut self.state.in_flight);
        for entry in recovered {
            if entry.lease_expires_at_unix_ms <= now_unix_ms {
                self.record_recovery(&entry, now_unix_ms, "expired", CommandOutcome::TimedOut);
                let result = recovery_result(&entry, now_unix_ms, CommandOutcome::TimedOut);
                self.add_completed_state(&entry, now_unix_ms, Some(result))?;
            } else if entry.retry_safe {
                self.record_recovery(&entry, now_unix_ms, "retry_pending", CommandOutcome::Failed);
                self.in_flight_commands
                    .insert(entry.command_id.clone(), entry.clone());
                self.state.in_flight.push(entry);
            } else {
                self.record_recovery(
                    &entry,
                    now_unix_ms,
                    "uncertain_side_effect_blocked",
                    CommandOutcome::Rejected,
                );
                let result = recovery_result(&entry, now_unix_ms, CommandOutcome::Failed);
                self.add_completed_state(&entry, now_unix_ms, Some(result))?;
            }
        }
        self.processed_commands = self
            .state
            .completed
            .iter()
            .map(|entry| entry.command_id.clone())
            .collect();
        if self.state != original_state {
            self.state_store.save(&self.state)?;
        }
        Ok(())
    }

    fn record_recovery(
        &mut self,
        entry: &InFlightCommandState,
        now_unix_ms: u64,
        status: &str,
        outcome: CommandOutcome,
    ) {
        self.audit_sink.record(AuditEvent {
            event_type: "desktop.command.recovered".to_owned(),
            command_id: entry.command_id.clone(),
            execution_id: entry.execution_id.clone(),
            tenant_id: entry.tenant_id.clone(),
            project_id: entry.project_id.clone(),
            actor_id: entry.actor_id.clone(),
            occurred_at_unix_ms: now_unix_ms,
            risk_level: entry.risk_level,
            outcome,
            details: json!({
                "capability": entry.capability,
                "lease_id": entry.lease_id,
                "recovery_status": status,
            }),
        });
        self.recovery_health_events.push(RecoveryHealthEvent {
            event_type: "desktop.recovery.health".to_owned(),
            occurred_at_unix_ms: now_unix_ms,
            command_id: entry.command_id.clone(),
            status: status.to_owned(),
        });
    }

    fn execute_durably(
        &mut self,
        command: &DesktopCommand,
        now_unix_ms: u64,
        recovering: bool,
    ) -> Result<DesktopCommandResult, CoreError> {
        if !recovering {
            self.ensure_capacity()?;
            let in_flight = InFlightCommandState::from_command(command, now_unix_ms);
            self.state.in_flight.push(in_flight.clone());
            self.in_flight_commands
                .insert(command.command_id.clone(), in_flight);
            if let Err(error) = self.state_store.save(&self.state) {
                self.state
                    .in_flight
                    .retain(|entry| entry.command_id != command.command_id);
                self.in_flight_commands.remove(&command.command_id);
                return Err(error);
            }
        }
        Ok(self.execute(command, now_unix_ms))
    }

    fn persist_completed(
        &mut self,
        command: &DesktopCommand,
        result: &DesktopCommandResult,
        now_unix_ms: u64,
    ) -> Result<(), CoreError> {
        if !self.in_flight_commands.contains_key(&command.command_id) {
            self.ensure_capacity()?;
        }
        let mut next_state = self.state.clone();
        next_state
            .in_flight
            .retain(|entry| entry.command_id != command.command_id);
        let retained_until_unix_ms = now_unix_ms
            .checked_add(self.recovery_config.safety_window_ms)
            .ok_or_else(|| CoreError::Recovery("retention deadline overflow".to_owned()))?;
        next_state.completed.push(CompletedCommandState {
            command_id: command.command_id.clone(),
            execution_id: command.execution_id.clone(),
            tenant_id: command.tenant_id.clone(),
            project_id: command.project_id.clone(),
            actor_id: command.requested_by.clone(),
            lease_id: command.lease.lease_id.clone(),
            lease_expires_at_unix_ms: command.lease.expires_at_unix_ms,
            capability: command.action.capability(),
            risk_level: command.action.risk_level(),
            retained_until_unix_ms,
            pending_result: Some(result.clone()),
        });
        if let Err(error) = self.state_store.save(&next_state) {
            self.processed_commands.insert(command.command_id.clone());
            return Err(error);
        }
        self.state = next_state;
        self.in_flight_commands.remove(&command.command_id);
        self.processed_commands.insert(command.command_id.clone());
        Ok(())
    }

    fn add_completed_state(
        &mut self,
        entry: &InFlightCommandState,
        now_unix_ms: u64,
        pending_result: Option<DesktopCommandResult>,
    ) -> Result<(), CoreError> {
        let retained_until_unix_ms = now_unix_ms
            .checked_add(self.recovery_config.safety_window_ms)
            .ok_or_else(|| CoreError::Recovery("retention deadline overflow".to_owned()))?;
        self.state.completed.push(CompletedCommandState {
            command_id: entry.command_id.clone(),
            execution_id: entry.execution_id.clone(),
            tenant_id: entry.tenant_id.clone(),
            project_id: entry.project_id.clone(),
            actor_id: entry.actor_id.clone(),
            lease_id: entry.lease_id.clone(),
            lease_expires_at_unix_ms: entry.lease_expires_at_unix_ms,
            capability: entry.capability,
            risk_level: entry.risk_level,
            retained_until_unix_ms,
            pending_result,
        });
        Ok(())
    }

    fn prune_completed(&mut self, now_unix_ms: u64) -> Result<(), CoreError> {
        let previous_len = self.state.completed.len();
        self.state
            .completed
            .retain(|entry| entry.retained_until_unix_ms > now_unix_ms);
        if self.state.completed.len() != previous_len {
            self.processed_commands = self
                .state
                .completed
                .iter()
                .map(|entry| entry.command_id.clone())
                .collect();
            self.state_store.save(&self.state)?;
        }
        Ok(())
    }

    fn ensure_capacity(&self) -> Result<(), CoreError> {
        if self.state.completed.len() + self.state.in_flight.len()
            >= self.recovery_config.max_entries
        {
            Err(CoreError::Recovery(
                "recovery state capacity reached; refusing execution".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    fn execute(&mut self, command: &DesktopCommand, now_unix_ms: u64) -> DesktopCommandResult {
        match self.executor.execute(&command.action) {
            Ok(output) => DesktopCommandResult {
                command_id: command.command_id.clone(),
                execution_id: command.execution_id.clone(),
                outcome: CommandOutcome::Succeeded,
                completed_at_unix_ms: now_unix_ms,
                output: Some(output),
                error_code: None,
                error_message: None,
            },
            Err(error) => DesktopCommandResult {
                command_id: command.command_id.clone(),
                execution_id: command.execution_id.clone(),
                outcome: CommandOutcome::Failed,
                completed_at_unix_ms: now_unix_ms,
                output: None,
                error_code: Some("execution_failed".to_owned()),
                error_message: Some(error.to_string()),
            },
        }
    }
}

fn validate_persisted_state(
    state: &PersistedCommandState,
    config: &RecoveryConfig,
) -> Result<(), CoreError> {
    if state.schema_version != RECOVERY_SCHEMA_VERSION {
        return Err(CoreError::Recovery(format!(
            "unsupported recovery schema version {}",
            state.schema_version
        )));
    }
    if state.completed.len() + state.in_flight.len() > config.max_entries {
        return Err(CoreError::Recovery(
            "recovery state exceeds configured capacity".to_owned(),
        ));
    }
    let mut identifiers = HashSet::new();
    for (command_id, execution_id) in state
        .completed
        .iter()
        .map(|entry| (&entry.command_id, &entry.execution_id))
        .chain(
            state
                .in_flight
                .iter()
                .map(|entry| (&entry.command_id, &entry.execution_id)),
        )
    {
        if command_id.is_empty()
            || command_id.len() > 128
            || execution_id.is_empty()
            || execution_id.len() > 128
            || !identifiers.insert(command_id)
        {
            return Err(CoreError::Recovery(
                "recovery state contains invalid or duplicate identifiers".to_owned(),
            ));
        }
    }
    for entry in &state.in_flight {
        if entry.tenant_id.is_empty()
            || entry.project_id.is_empty()
            || entry.actor_id.is_empty()
            || entry.lease_id.is_empty()
            || entry.started_at_unix_ms > entry.lease_expires_at_unix_ms
        {
            return Err(CoreError::Recovery(
                "recovery state contains invalid in-flight metadata".to_owned(),
            ));
        }
    }
    for entry in &state.completed {
        if entry.tenant_id.is_empty()
            || entry.project_id.is_empty()
            || entry.actor_id.is_empty()
            || entry.lease_id.is_empty()
        {
            return Err(CoreError::Recovery(
                "recovery state contains invalid completed metadata".to_owned(),
            ));
        }
        if let Some(result) = &entry.pending_result {
            result.validate().map_err(CoreError::Protocol)?;
            if result.command_id != entry.command_id || result.execution_id != entry.execution_id {
                return Err(CoreError::Recovery(
                    "pending result does not match completed command metadata".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct SystemInformationExecutor;

impl ActionExecutor for SystemInformationExecutor {
    fn execute(&mut self, action: &DesktopAction) -> Result<Value, CoreError> {
        if action != &DesktopAction::ReadSystemInformation {
            return Err(CoreError::UnsupportedAction);
        }
        Ok(json!({
            "operating_system": std::env::consts::OS,
            "family": std::env::consts::FAMILY,
            "architecture": std::env::consts::ARCH,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_protocol::{ElementSelector, ExecutionLease, WindowSelector};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn command(action: DesktopAction) -> DesktopCommand {
        DesktopCommand {
            command_id: "command-1".to_owned(),
            execution_id: "execution-1".to_owned(),
            tenant_id: "tenant-1".to_owned(),
            project_id: "project-1".to_owned(),
            requested_by: "user-1".to_owned(),
            issued_at_unix_ms: 1_000,
            lease: ExecutionLease {
                lease_id: "lease-1".to_owned(),
                expires_at_unix_ms: 5_000,
            },
            approval: None,
            action,
        }
    }

    fn persisted_in_flight(
        action: DesktopAction,
        expires_at_unix_ms: u64,
    ) -> PersistedCommandState {
        let mut command = command(action);
        command.lease.expires_at_unix_ms = expires_at_unix_ms;
        PersistedCommandState {
            schema_version: RECOVERY_SCHEMA_VERSION,
            completed: Vec::new(),
            in_flight: vec![InFlightCommandState::from_command(&command, 2_000)],
        }
    }

    fn write_action() -> DesktopAction {
        DesktopAction::TypeText {
            selector: ElementSelector {
                window: WindowSelector {
                    executable: Some("fixture.exe".to_owned()),
                    title: None,
                    automation_id: None,
                },
                automation_id: Some("name-field".to_owned()),
                name: None,
                control_type: Some("Edit".to_owned()),
            },
            text: "sensitive fixture text".to_owned(),
        }
    }

    fn temporary_recovery_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "trigix-{label}-recovery-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn safe_information_action_executes_and_is_audited() {
        let mut processor = CommandProcessor::new(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
        );

        let result = processor
            .process(&command(DesktopAction::ReadSystemInformation), 2_000, None)
            .unwrap();

        assert_eq!(result.outcome, CommandOutcome::Succeeded);
        assert_eq!(processor.audit_sink().events().len(), 1);
        assert_eq!(
            processor.audit_sink().events()[0].event_type,
            "desktop.command.processed"
        );
    }

    #[test]
    fn write_action_waits_for_matching_approval() {
        let action = write_action();
        let mut processor = CommandProcessor::new(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
        );

        let result = processor.process(&command(action), 2_000, None).unwrap();

        assert_eq!(result.outcome, CommandOutcome::AwaitingApproval);
        assert_eq!(result.error_code.as_deref(), Some("approval_required"));
        assert_eq!(
            processor.audit_sink().events()[0].event_type,
            "desktop.command.awaiting_approval"
        );
    }

    #[test]
    fn duplicate_delivery_returns_pending_result_without_reexecution() {
        let mut processor = CommandProcessor::new(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
        );
        let command = command(DesktopAction::ReadSystemInformation);
        let first = processor.process(&command, 2_000, None).unwrap();

        assert_eq!(processor.process(&command, 2_001, None).unwrap(), first);
        processor
            .confirm_result_delivery(&command.command_id)
            .unwrap();
        assert_eq!(
            processor.process(&command, 2_002, None),
            Err(CoreError::DuplicateCommand("command-1".to_owned()))
        );
    }

    #[test]
    fn duplicate_identifier_with_conflicting_metadata_fails_closed() {
        let mut processor = CommandProcessor::new(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
        );
        let command = command(DesktopAction::ReadSystemInformation);
        processor.process(&command, 2_000, None).unwrap();
        let mut conflicting = command.clone();
        conflicting.project_id = "different-project".to_owned();

        assert!(matches!(
            processor.process(&conflicting, 2_001, None),
            Err(CoreError::Recovery(_))
        ));
    }

    #[test]
    fn expired_command_is_rejected_before_policy_or_execution() {
        let mut processor = CommandProcessor::new(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
        );

        assert_eq!(
            processor.process(&command(DesktopAction::ReadSystemInformation), 5_000, None),
            Err(CoreError::Protocol(ProtocolError::ExpiredLease))
        );
        assert!(processor.audit_sink().events().is_empty());
    }

    #[test]
    fn completed_command_remains_blocked_after_restart() {
        let store = MemoryCommandStateStore::default();
        let mut first = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store.clone(),
            RecoveryConfig::default(),
            1_000,
        )
        .unwrap();
        let command = command(DesktopAction::ReadSystemInformation);
        first.process(&command, 2_000, None).unwrap();
        first.confirm_result_delivery(&command.command_id).unwrap();
        drop(first);

        let mut restarted = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store,
            RecoveryConfig::default(),
            2_001,
        )
        .unwrap();
        assert_eq!(
            restarted.process(&command, 2_002, None),
            Err(CoreError::DuplicateCommand("command-1".to_owned()))
        );
    }

    #[test]
    fn pending_result_survives_restart_until_platform_delivery_is_confirmed() {
        let store = MemoryCommandStateStore::default();
        let executions = Arc::new(AtomicUsize::new(0));
        let command = command(DesktopAction::ReadSystemInformation);
        let mut first = CommandProcessor::with_recovery_store(
            CountingExecutor(executions.clone()),
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store.clone(),
            RecoveryConfig::default(),
            1_000,
        )
        .unwrap();
        let expected = first.process(&command, 2_000, None).unwrap();
        drop(first);

        let mut restarted = CommandProcessor::with_recovery_store(
            CountingExecutor(executions.clone()),
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store,
            RecoveryConfig::default(),
            2_001,
        )
        .unwrap();
        assert_eq!(restarted.process(&command, 2_002, None).unwrap(), expected);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        restarted
            .confirm_result_delivery(&command.command_id)
            .unwrap();
        assert_eq!(
            restarted.process(&command, 2_003, None),
            Err(CoreError::DuplicateCommand("command-1".to_owned()))
        );
    }

    #[test]
    fn file_store_recovers_pending_result_after_process_restart() {
        let path = temporary_recovery_path("restart");
        let command = command(DesktopAction::ReadSystemInformation);
        let mut first = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            FileCommandStateStore::new(&path),
            RecoveryConfig::default(),
            1_000,
        )
        .unwrap();
        let expected = first.process(&command, 2_000, None).unwrap();
        drop(first);

        let mut restarted = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            FileCommandStateStore::new(&path),
            RecoveryConfig::default(),
            2_001,
        )
        .unwrap();
        assert_eq!(restarted.process(&command, 2_002, None).unwrap(), expected);
        restarted
            .confirm_result_delivery(&command.command_id)
            .unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn restart_retries_only_idempotent_in_flight_action_with_valid_lease() {
        let mut store = MemoryCommandStateStore::default();
        store
            .save(&persisted_in_flight(
                DesktopAction::ReadSystemInformation,
                5_000,
            ))
            .unwrap();
        let mut processor = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store,
            RecoveryConfig::default(),
            3_000,
        )
        .unwrap();

        assert_eq!(
            processor.recovery_health_events()[0].status,
            "retry_pending"
        );
        let result = processor
            .process(&command(DesktopAction::ReadSystemInformation), 3_001, None)
            .unwrap();
        assert_eq!(result.outcome, CommandOutcome::Succeeded);
    }

    #[test]
    fn uncertain_write_after_crash_fails_closed_and_is_redacted() {
        let mut store = MemoryCommandStateStore::default();
        let persisted = persisted_in_flight(write_action(), 5_000);
        let serialized = serde_json::to_string(&persisted).unwrap();
        assert!(!serialized.contains("sensitive fixture text"));
        store.save(&persisted).unwrap();
        let mut processor = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store,
            RecoveryConfig::default(),
            3_000,
        )
        .unwrap();

        assert_eq!(
            processor.recovery_health_events()[0].status,
            "uncertain_side_effect_blocked"
        );
        let result = processor
            .process(&command(write_action()), 3_001, None)
            .unwrap();
        assert_eq!(result.outcome, CommandOutcome::Failed);
        assert_eq!(
            result.error_code.as_deref(),
            Some("uncertain_side_effect_blocked")
        );
        assert!(!serde_json::to_string(processor.audit_sink().events())
            .unwrap()
            .contains("sensitive fixture text"));
    }

    #[test]
    fn expired_in_flight_lease_never_executes_after_restart() {
        let mut store = MemoryCommandStateStore::default();
        store
            .save(&persisted_in_flight(
                DesktopAction::ReadSystemInformation,
                2_500,
            ))
            .unwrap();
        let mut processor = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store,
            RecoveryConfig::default(),
            3_000,
        )
        .unwrap();

        assert_eq!(processor.recovery_health_events()[0].status, "expired");
        assert_eq!(
            processor.state.completed[0]
                .pending_result
                .as_ref()
                .unwrap()
                .outcome,
            CommandOutcome::TimedOut
        );
        assert!(matches!(
            processor.process(&command(DesktopAction::ReadSystemInformation), 3_001, None,),
            Err(CoreError::Recovery(_))
        ));
    }

    #[test]
    fn bounded_store_refuses_execution_instead_of_dropping_replay_history() {
        let store = MemoryCommandStateStore::default();
        let config = RecoveryConfig {
            safety_window_ms: 10_000,
            max_entries: 1,
        };
        let mut processor = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            store,
            config,
            1_000,
        )
        .unwrap();
        processor
            .process(&command(DesktopAction::ReadSystemInformation), 2_000, None)
            .unwrap();
        let mut second = command(DesktopAction::ReadSystemInformation);
        second.command_id = "command-2".to_owned();
        second.lease.lease_id = "lease-2".to_owned();

        assert!(matches!(
            processor.process(&second, 2_001, None),
            Err(CoreError::Recovery(_))
        ));
    }

    #[test]
    fn completed_identifier_expires_after_configured_safety_window() {
        let executions = Arc::new(AtomicUsize::new(0));
        let command = command(DesktopAction::ReadSystemInformation);
        let mut processor = CommandProcessor::with_recovery_store(
            CountingExecutor(executions.clone()),
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            MemoryCommandStateStore::default(),
            RecoveryConfig {
                safety_window_ms: 1_000,
                max_entries: 1,
            },
            1_000,
        )
        .unwrap();
        processor.process(&command, 2_000, None).unwrap();
        processor
            .confirm_result_delivery(&command.command_id)
            .unwrap();

        processor.process(&command, 3_001, None).unwrap();
        assert_eq!(executions.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn corrupt_file_state_fails_closed() {
        let path = temporary_recovery_path("corrupt");
        fs::write(&path, b"{not-json").unwrap();
        let result = CommandProcessor::with_recovery_store(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            FileCommandStateStore::new(&path),
            RecoveryConfig::default(),
            1_000,
        );
        fs::remove_file(path).unwrap();
        assert!(matches!(result, Err(CoreError::Recovery(_))));
    }

    #[derive(Clone)]
    struct CountingExecutor(Arc<AtomicUsize>);

    impl ActionExecutor for CountingExecutor {
        fn execute(&mut self, _action: &DesktopAction) -> Result<Value, CoreError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(json!({"ok": true}))
        }
    }

    struct FailingSaveStore;

    impl CommandStateStore for FailingSaveStore {
        fn load(&self) -> Result<Option<PersistedCommandState>, CoreError> {
            Ok(None)
        }

        fn save(&mut self, _state: &PersistedCommandState) -> Result<(), CoreError> {
            Err(CoreError::Recovery("injected save failure".to_owned()))
        }
    }

    #[test]
    fn storage_failure_prevents_side_effect() {
        let executions = Arc::new(AtomicUsize::new(0));
        let mut processor = CommandProcessor::with_recovery_store(
            CountingExecutor(executions.clone()),
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            FailingSaveStore,
            RecoveryConfig::default(),
            1_000,
        )
        .unwrap();

        assert!(matches!(
            processor.process(&command(DesktopAction::ReadSystemInformation), 2_000, None),
            Err(CoreError::Recovery(_))
        ));
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}
