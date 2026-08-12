use desktop_agent_core::{
    CommandProcessor, ExecutionPolicy, InMemoryAuditSink, SystemInformationExecutor,
};
use desktop_protocol::{DesktopCommand, DesktopCommandResult};

pub type DesktopProcessor = CommandProcessor<SystemInformationExecutor, InMemoryAuditSink>;

pub fn create_processor() -> DesktopProcessor {
    CommandProcessor::new(
        SystemInformationExecutor,
        InMemoryAuditSink::default(),
        ExecutionPolicy::default(),
    )
}

pub fn process_platform_command(
    processor: &mut DesktopProcessor,
    command_json: &str,
    now_unix_ms: u64,
) -> Result<String, String> {
    let command: DesktopCommand = serde_json::from_str(command_json)
        .map_err(|error| format!("invalid command payload: {error}"))?;
    let result: DesktopCommandResult = processor
        .process(&command, now_unix_ms, None)
        .map_err(|error| error.to_string())?;
    serde_json::to_string(&result).map_err(|error| format!("result serialization failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_protocol::{DesktopAction, ExecutionLease};

    #[test]
    fn host_boundary_accepts_typed_json_and_returns_typed_result() {
        let command = DesktopCommand {
            command_id: "command-host-1".to_owned(),
            execution_id: "execution-host-1".to_owned(),
            tenant_id: "tenant-1".to_owned(),
            project_id: "project-1".to_owned(),
            requested_by: "user-1".to_owned(),
            issued_at_unix_ms: 1_000,
            lease: ExecutionLease {
                lease_id: "lease-host-1".to_owned(),
                expires_at_unix_ms: 5_000,
            },
            action: DesktopAction::ReadSystemInformation,
        };
        let mut processor = create_processor();

        let result = process_platform_command(
            &mut processor,
            &serde_json::to_string(&command).unwrap(),
            2_000,
        )
        .unwrap();

        assert!(result.contains("\"outcome\":\"succeeded\""));
    }
}
