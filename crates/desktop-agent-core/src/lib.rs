use desktop_protocol::{
    CommandOutcome, DesktopAction, DesktopCommand, DesktopCommandResult, ProtocolError, RiskLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    Protocol(ProtocolError),
    DuplicateCommand(String),
    UnsupportedAction,
    Execution(String),
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

pub struct CommandProcessor<E, A> {
    executor: E,
    audit_sink: A,
    policy: ExecutionPolicy,
    processed_commands: HashSet<String>,
}

impl<E: ActionExecutor, A: AuditSink> CommandProcessor<E, A> {
    pub fn new(executor: E, audit_sink: A, policy: ExecutionPolicy) -> Self {
        Self {
            executor,
            audit_sink,
            policy,
            processed_commands: HashSet::new(),
        }
    }

    pub fn process(
        &mut self,
        command: &DesktopCommand,
        now_unix_ms: u64,
        approval: Option<&ApprovalGrant>,
    ) -> Result<DesktopCommandResult, CoreError> {
        command.validate(now_unix_ms)?;
        if self.processed_commands.contains(&command.command_id) {
            return Err(CoreError::DuplicateCommand(command.command_id.clone()));
        }

        let decision = self.policy.evaluate(&command.action);
        let result = match decision {
            PolicyDecision::Allow => self.execute(command, now_unix_ms),
            PolicyDecision::RequireApproval { reason } => {
                if approval.is_some_and(|grant| grant.authorizes(command, now_unix_ms)) {
                    self.execute(command, now_unix_ms)
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
            self.processed_commands.insert(command.command_id.clone());
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
            action,
        }
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
        let action = DesktopAction::TypeText {
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
            text: "approved text".to_owned(),
        };
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
    fn processed_command_cannot_be_replayed() {
        let mut processor = CommandProcessor::new(
            SystemInformationExecutor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
        );
        let command = command(DesktopAction::ReadSystemInformation);
        processor.process(&command, 2_000, None).unwrap();

        assert_eq!(
            processor.process(&command, 2_001, None),
            Err(CoreError::DuplicateCommand("command-1".to_owned()))
        );
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
}
