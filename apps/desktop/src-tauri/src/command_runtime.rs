use desktop_agent_core::{
    ApprovalGrant, CommandProcessor, ExecutionPolicy, FileCommandStateStore, InMemoryAuditSink,
    RecoveryConfig,
};
use desktop_protocol::{CommandOutcome, DesktopCommand, DesktopCommandResult, DeviceCapability};
use std::path::PathBuf;
use std::sync::Mutex;
use trigix_desktop_automation::{
    AutomationCancellation, AutomationHostOperation, AutomationHostRequest, AutomationHostStatus,
    AutomationHostSupervisor, SupervisedActionExecutor, SupervisedActionExecutorHandle,
    SupervisorConfig,
};

type NativeProcessor =
    CommandProcessor<SupervisedActionExecutor, InMemoryAuditSink, FileCommandStateStore>;

pub(crate) struct CommandRuntime {
    processor: Mutex<NativeProcessor>,
    executor_handle: SupervisedActionExecutorHandle,
    active_command: Mutex<Option<ActiveCommand>>,
}

struct ActiveCommand {
    command_id: String,
    execution_id: String,
}

impl CommandRuntime {
    pub fn initialize(
        host_executable: PathBuf,
        recovery_path: PathBuf,
        now_unix_ms: u64,
    ) -> Result<Self, RuntimeError> {
        if !host_executable.is_file() {
            return Err(RuntimeError::HostUnavailable);
        }
        let supervisor =
            AutomationHostSupervisor::new(host_executable, SupervisorConfig::default())
                .map_err(|_| RuntimeError::HostUnavailable)?;
        let health = supervisor
            .execute(
                AutomationHostRequest {
                    request_id: format!("desktop-host-health-{now_unix_ms}"),
                    sent_at_unix_ms: now_unix_ms,
                    deadline_unix_ms: now_unix_ms.saturating_add(5_000),
                    operation: AutomationHostOperation::Health,
                },
                &AutomationCancellation::default(),
            )
            .map_err(|_| RuntimeError::HostUnavailable)?;
        if health.status != AutomationHostStatus::Ready {
            return Err(RuntimeError::HostUnavailable);
        }
        let (executor, executor_handle) = SupervisedActionExecutor::new(supervisor);
        let processor = CommandProcessor::with_recovery_store(
            executor,
            InMemoryAuditSink::default(),
            ExecutionPolicy::default(),
            FileCommandStateStore::new(recovery_path),
            RecoveryConfig::default(),
            now_unix_ms,
        )
        .map_err(|_| RuntimeError::RecoveryUnavailable)?;
        Ok(Self {
            processor: Mutex::new(processor),
            executor_handle,
            active_command: Mutex::new(None),
        })
    }

    pub fn capabilities(&self) -> Vec<DeviceCapability> {
        vec![
            DeviceCapability::SystemInformation,
            DeviceCapability::WindowManagement,
            DeviceCapability::UiAutomation,
            DeviceCapability::KeyboardInput,
            DeviceCapability::PointerInput,
        ]
    }

    pub fn execute(
        &self,
        command: &DesktopCommand,
        now_unix_ms: u64,
    ) -> Result<DesktopCommandResult, RuntimeError> {
        let already_reserved = self
            .active_command
            .lock()
            .map_err(|_| RuntimeError::StateUnavailable)?
            .as_ref()
            .is_some_and(|active| {
                active.command_id == command.command_id
                    && active.execution_id == command.execution_id
            });
        if !already_reserved {
            self.reserve(command)?;
        }
        let result = self.execute_inner(command, now_unix_ms);
        self.executor_handle
            .release_reservation(&command.command_id);
        if let Ok(mut active) = self.active_command.lock() {
            *active = None;
        }
        result
    }

    pub fn reserve(&self, command: &DesktopCommand) -> Result<(), RuntimeError> {
        let mut active = self
            .active_command
            .lock()
            .map_err(|_| RuntimeError::StateUnavailable)?;
        if active.is_some() || !self.executor_handle.reserve(&command.command_id) {
            return Err(RuntimeError::Busy);
        }
        *active = Some(ActiveCommand {
            command_id: command.command_id.clone(),
            execution_id: command.execution_id.clone(),
        });
        Ok(())
    }

    pub fn abandon(&self, command_id: &str) {
        self.executor_handle.release_reservation(command_id);
        if let Ok(mut active) = self.active_command.lock() {
            if active
                .as_ref()
                .is_some_and(|active| active.command_id == command_id)
            {
                *active = None;
            }
        }
    }

    fn execute_inner(
        &self,
        command: &DesktopCommand,
        now_unix_ms: u64,
    ) -> Result<DesktopCommandResult, RuntimeError> {
        let approval = command.approval.as_ref().map(|approval| ApprovalGrant {
            command_id: command.command_id.clone(),
            approved_by: approval.approved_by.clone(),
            expires_at_unix_ms: approval.expires_at_unix_ms,
        });
        self.processor
            .lock()
            .map_err(|_| RuntimeError::StateUnavailable)?
            .process(command, now_unix_ms, approval.as_ref())
            .map_err(|_| RuntimeError::Execution)
    }

    pub fn confirm_result_delivery(
        &self,
        result: &DesktopCommandResult,
    ) -> Result<(), RuntimeError> {
        if result.outcome == CommandOutcome::AwaitingApproval {
            return Ok(());
        }
        self.processor
            .lock()
            .map_err(|_| RuntimeError::StateUnavailable)?
            .confirm_result_delivery(&result.command_id)
            .map_err(|_| RuntimeError::RecoveryUnavailable)
    }

    pub fn cancel(&self, command_id: &str) -> bool {
        self.executor_handle.cancel(command_id)
    }

    pub fn cancel_active(&self) -> bool {
        let command_id = self
            .active_command
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|active| active.command_id.clone()));
        command_id
            .as_deref()
            .is_some_and(|command_id| self.cancel(command_id))
    }

    pub fn active_execution_id(&self) -> Option<String> {
        self.active_command
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|active| active.execution_id.clone()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeError {
    HostUnavailable,
    RecoveryUnavailable,
    StateUnavailable,
    Busy,
    Execution,
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use desktop_protocol::{CommandOutcome, DesktopAction, ExecutionLease};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn healthy_isolated_host_enables_capabilities_and_persisted_execution() {
        let now = current_millis();
        let root =
            std::env::temp_dir().join(format!("trigix-shell-runtime-{}-{now}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let host = root.join("fixture-host.sh");
        std::fs::write(
            &host,
            "#!/bin/sh\nIFS= read -r line\nid=$(printf '%s' \"$line\" | sed -n 's/.*\"request_id\":\"\\([^\"]*\\)\".*/\\1/p')\ncase \"$line\" in *'\"kind\":\"health\"'*) status=ready ;; *) status=succeeded ;; esac\nprintf '{\"request_id\":\"%s\",\"status\":\"%s\",\"output\":{}}\\n' \"$id\" \"$status\"\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&host).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&host, permissions).unwrap();

        let runtime = CommandRuntime::initialize(host, root.join("recovery.json"), now).unwrap();
        assert_eq!(
            runtime.capabilities(),
            vec![
                DeviceCapability::SystemInformation,
                DeviceCapability::WindowManagement,
                DeviceCapability::UiAutomation,
                DeviceCapability::KeyboardInput,
                DeviceCapability::PointerInput,
            ]
        );
        let command = DesktopCommand {
            command_id: "command-1".to_owned(),
            execution_id: "execution-1".to_owned(),
            tenant_id: "tenant-1".to_owned(),
            project_id: "project-1".to_owned(),
            requested_by: "user-1".to_owned(),
            issued_at_unix_ms: now,
            lease: ExecutionLease {
                lease_id: "lease-1".to_owned(),
                expires_at_unix_ms: now + 30_000,
            },
            approval: None,
            action: DesktopAction::ReadSystemInformation,
        };
        let result = runtime.execute(&command, now + 1).unwrap();
        assert_eq!(result.outcome, CommandOutcome::Succeeded);
        runtime.confirm_result_delivery(&result).unwrap();
        assert_eq!(runtime.active_execution_id(), None);

        let mut cancelled = command;
        cancelled.command_id = "command-cancelled-before-start".to_owned();
        cancelled.execution_id = "execution-cancelled-before-start".to_owned();
        runtime.reserve(&cancelled).unwrap();
        assert!(runtime.cancel(&cancelled.command_id));
        let result = runtime.execute(&cancelled, now + 2).unwrap();
        assert_eq!(result.outcome, CommandOutcome::Cancelled);
        runtime.confirm_result_delivery(&result).unwrap();

        let mut awaiting_approval = cancelled;
        awaiting_approval.command_id = "command-awaiting-approval".to_owned();
        awaiting_approval.execution_id = "execution-awaiting-approval".to_owned();
        awaiting_approval.action = DesktopAction::LaunchApplication {
            application_id: desktop_protocol::ApplicationIdentity::new("notepad"),
        };
        let result = runtime.execute(&awaiting_approval, now + 3).unwrap();
        assert_eq!(result.outcome, CommandOutcome::AwaitingApproval);
        runtime.confirm_result_delivery(&result).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    fn current_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn qualified_native_host_enables_runtime_capabilities() {
        let host = std::env::var_os("TRIGIX_AUTOMATION_HOST_EXE")
            .map(PathBuf::from)
            .expect("TRIGIX_AUTOMATION_HOST_EXE must identify the qualified Host");
        let root = std::env::temp_dir().join(format!(
            "trigix-native-shell-runtime-{}",
            std::process::id()
        ));
        let runtime =
            CommandRuntime::initialize(host, root.join("recovery.json"), current_millis())
                .expect("qualified native Host must initialize");
        assert_eq!(
            runtime.capabilities(),
            vec![
                DeviceCapability::SystemInformation,
                DeviceCapability::WindowManagement,
                DeviceCapability::UiAutomation,
                DeviceCapability::KeyboardInput,
                DeviceCapability::PointerInput,
            ]
        );
    }

    fn current_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}
