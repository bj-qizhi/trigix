use desktop_agent_core::{
    CommandProcessor, ExecutionPolicy, InMemoryAuditSink, SystemInformationExecutor,
};
use desktop_protocol::{
    DesktopAction, DesktopCommand, DeviceCapability, DeviceDescriptor, Envelope, ExecutionLease,
    PairingRequest,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let pairing = Envelope::new(
        "simulator-pairing-message",
        now,
        PairingRequest {
            pairing_code: "TRGX2026".to_owned(),
            device: DeviceDescriptor {
                device_id: "simulator-device".to_owned(),
                display_name: "Trigix Desktop Simulator".to_owned(),
                operating_system: std::env::consts::OS.to_owned(),
                agent_version: env!("CARGO_PKG_VERSION").to_owned(),
                capabilities: vec![DeviceCapability::SystemInformation],
            },
            device_public_key: "simulator-public-key".to_owned(),
        },
    );
    pairing.validate()?;
    pairing.payload.validate()?;

    let command = DesktopCommand {
        command_id: "simulator-command".to_owned(),
        execution_id: "simulator-execution".to_owned(),
        tenant_id: "simulator-tenant".to_owned(),
        project_id: "simulator-project".to_owned(),
        requested_by: "simulator-user".to_owned(),
        issued_at_unix_ms: now,
        lease: ExecutionLease {
            lease_id: "simulator-lease".to_owned(),
            expires_at_unix_ms: now + 60_000,
        },
        action: DesktopAction::ReadSystemInformation,
    };
    let mut processor = CommandProcessor::new(
        SystemInformationExecutor,
        InMemoryAuditSink::default(),
        ExecutionPolicy::default(),
    );
    let result = processor.process(&command, now, None)?;

    println!("{}", serde_json::to_string(&pairing)?);
    println!("{}", serde_json::to_string(&result)?);
    for event in processor.audit_sink().events() {
        println!("{}", serde_json::to_string(event)?);
    }
    Ok(())
}
