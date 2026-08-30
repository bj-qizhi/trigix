use super::{
    AutomationCancellation, AutomationHostOperation, AutomationHostRequest, AutomationHostStatus,
    AutomationHostSupervisor,
};
use desktop_agent_core::{ActionExecutor, CoreError};
use desktop_protocol::DesktopCommand;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct SupervisedActionExecutorHandle {
    cancellations: Arc<Mutex<HashMap<String, AutomationCancellation>>>,
}

impl SupervisedActionExecutorHandle {
    pub fn cancel(&self, command_id: &str) -> bool {
        let cancellations = self
            .cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(cancellation) = cancellations.get(command_id) else {
            return false;
        };
        cancellation.cancel();
        true
    }

    pub fn active_commands(&self) -> usize {
        self.cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

#[derive(Debug)]
pub struct SupervisedActionExecutor {
    supervisor: AutomationHostSupervisor,
    handle: SupervisedActionExecutorHandle,
}

impl SupervisedActionExecutor {
    pub fn new(supervisor: AutomationHostSupervisor) -> (Self, SupervisedActionExecutorHandle) {
        let handle = SupervisedActionExecutorHandle::default();
        (
            Self {
                supervisor,
                handle: handle.clone(),
            },
            handle,
        )
    }
}

impl ActionExecutor for SupervisedActionExecutor {
    fn execute(&mut self, command: &DesktopCommand) -> Result<Value, CoreError> {
        let cancellation = AutomationCancellation::default();
        {
            let mut cancellations =
                self.handle.cancellations.lock().map_err(|_| {
                    CoreError::Execution("cancellation state is poisoned".to_owned())
                })?;
            if cancellations
                .insert(command.command_id.clone(), cancellation.clone())
                .is_some()
            {
                return Err(CoreError::Execution(
                    "command is already active in the automation host".to_owned(),
                ));
            }
        }

        let now = now_unix_ms();
        let response = self.supervisor.execute(
            AutomationHostRequest {
                request_id: command.command_id.clone(),
                sent_at_unix_ms: now,
                deadline_unix_ms: command.lease.expires_at_unix_ms,
                operation: AutomationHostOperation::Execute {
                    command_id: command.command_id.clone(),
                    lease_id: command.lease.lease_id.clone(),
                    lease_expires_at_unix_ms: command.lease.expires_at_unix_ms,
                    action: Box::new(command.action.clone()),
                },
            },
            &cancellation,
        );
        self.handle
            .cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&command.command_id);

        let response = response.map_err(|error| CoreError::Execution(error.to_string()))?;
        match response.status {
            AutomationHostStatus::Succeeded => Ok(response.output.unwrap_or(Value::Null)),
            AutomationHostStatus::Cancelled => Err(CoreError::ExecutionCancelled),
            AutomationHostStatus::TimedOut => Err(CoreError::ExecutionTimedOut),
            AutomationHostStatus::Rejected => Err(CoreError::ExecutionRejected {
                code: response
                    .error_code
                    .unwrap_or_else(|| "automation_rejected".to_owned()),
                message: response
                    .error_message
                    .unwrap_or_else(|| "automation action was rejected".to_owned()),
            }),
            AutomationHostStatus::Failed => Err(CoreError::Execution(
                response
                    .error_message
                    .unwrap_or_else(|| "automation host failed".to_owned()),
            )),
            AutomationHostStatus::Ready | AutomationHostStatus::ShuttingDown => {
                Err(CoreError::Execution(
                    "automation host returned an invalid execute status".to_owned(),
                ))
            }
        }
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(u64::MAX)
}
