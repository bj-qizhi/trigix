use desktop_agent_core::{
    connection::{DeviceEndpoint, ReconnectBackoff},
    ApprovalGrant, CommandProcessor, ExecutionPolicy, InMemoryAuditSink, SystemInformationExecutor,
};
use desktop_protocol::{
    DesktopAction, DesktopCommand, DesktopCommandAcknowledgement, DeviceCapability,
    DeviceConnectionAccepted, DeviceDescriptor, DeviceState, Envelope, ExecutionLease, Heartbeat,
    PairingRequest,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let (Ok(base_url), Ok(device_id), Ok(credential)) = (
        std::env::var("DESKTOP_PLATFORM_URL"),
        std::env::var("DESKTOP_DEVICE_ID"),
        std::env::var("DESKTOP_DEVICE_CREDENTIAL"),
    ) {
        return run_connected(base_url, device_id, credential).await;
    }

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
        approval: None,
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

async fn run_connected(
    base_url: String,
    device_id: String,
    credential: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = DeviceEndpoint {
        base_url: base_url.trim_end_matches('/').to_string(),
        proxy_url: std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .ok(),
    };
    endpoint.validate().map_err(std::io::Error::other)?;
    let mut client_builder =
        reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(15));
    if let Some(proxy_url) = endpoint.proxy_url.as_deref() {
        client_builder = client_builder.proxy(reqwest::Proxy::https(proxy_url)?);
    }
    let client = client_builder.build()?;
    let mut backoff = ReconnectBackoff::default();
    let mut processor = CommandProcessor::new(
        SystemInformationExecutor,
        InMemoryAuditSink::default(),
        ExecutionPolicy::default(),
    );

    loop {
        match run_connection_once(
            &client,
            &endpoint.base_url,
            &device_id,
            &credential,
            &mut backoff,
            &mut processor,
        )
        .await
        {
            Ok(()) => {}
            Err(error) => eprintln!("Device connection ended: {error}"),
        }
        tokio::time::sleep(backoff.next_delay()).await;
    }
}

async fn run_connection_once(
    client: &reqwest::Client,
    base_url: &str,
    device_id: &str,
    credential: &str,
    backoff: &mut ReconnectBackoff,
    processor: &mut CommandProcessor<SystemInformationExecutor, InMemoryAuditSink>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut response = client
        .get(format!("{base_url}/v1/desktop/device-connection"))
        .header("x-device-id", device_id)
        .header("authorization", format!("Device {credential}"))
        .send()
        .await?
        .error_for_status()?;
    let mut pending = String::new();
    let connected = loop {
        let chunk = response
            .chunk()
            .await?
            .ok_or_else(|| std::io::Error::other("connection closed before acceptance"))?;
        pending.push_str(&String::from_utf8_lossy(&chunk));
        if let Some(event) = parse_sse_data(&mut pending, "connected")? {
            break serde_json::from_str::<DeviceConnectionAccepted>(&event)?;
        }
    };
    backoff.reset();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(u64::from(
        connected.heartbeat_interval_seconds,
    )));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;

    loop {
        tokio::select! {
            chunk = response.chunk() => {
                match chunk? {
                    Some(chunk) => {
                        pending.push_str(&String::from_utf8_lossy(&chunk));
                        while let Some((event, data)) = parse_next_sse(&mut pending) {
                            match event.as_str() {
                                "disconnect" => return Ok(()),
                                "command" => {
                                    let envelope: Envelope<DesktopCommand> = serde_json::from_str(&data)?;
                                    envelope.validate()?;
                                    let command = envelope.payload;
                                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
                                    let acknowledgement = Envelope::new(
                                        format!("ack-{}", command.command_id),
                                        now,
                                        DesktopCommandAcknowledgement {
                                            command_id: command.command_id.clone(),
                                            execution_id: command.execution_id.clone(),
                                            lease_id: command.lease.lease_id.clone(),
                                            acknowledged_at_unix_ms: now,
                                        },
                                    );
                                    post_device_message(client, base_url, device_id, credential, &connected.session_id, "device-command-acknowledgements", &acknowledgement).await?;
                                    let approval = command.approval.as_ref().map(|approval| ApprovalGrant {
                                        command_id: command.command_id.clone(),
                                        approved_by: approval.approved_by.clone(),
                                        expires_at_unix_ms: approval.expires_at_unix_ms,
                                    });
                                    let result = processor.process(&command, now, approval.as_ref())?;
                                    let result_envelope = Envelope::new(format!("result-{}", command.command_id), now, result);
                                    post_device_message(client, base_url, device_id, credential, &connected.session_id, "device-command-results", &result_envelope).await?;
                                }
                                "command_cancelled" => {}
                                _ => {}
                            }
                        }
                    }
                    None => return Err(std::io::Error::other("connection stream closed").into()),
                }
            }
            _ = heartbeat.tick() => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
                let envelope = Envelope::new(
                    format!("heartbeat-{now}"),
                    now,
                    Heartbeat {
                        device_id: device_id.to_string(),
                        state: DeviceState::Online,
                        active_execution_id: None,
                        agent_version: env!("CARGO_PKG_VERSION").to_string(),
                        capabilities: vec![DeviceCapability::SystemInformation],
                        health_detail: None,
                    },
                );
                client
                    .post(format!("{base_url}/v1/desktop/device-heartbeats"))
                    .header("x-device-id", device_id)
                    .header("x-device-session-id", &connected.session_id)
                    .header("authorization", format!("Device {credential}"))
                    .json(&envelope)
                    .send()
                    .await?
                    .error_for_status()?;
            }
        }
    }
}

async fn post_device_message<T: serde::Serialize>(
    client: &reqwest::Client,
    base_url: &str,
    device_id: &str,
    credential: &str,
    session_id: &str,
    path: &str,
    message: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    client
        .post(format!("{base_url}/v1/desktop/{path}"))
        .header("x-device-id", device_id)
        .header("x-device-session-id", session_id)
        .header("authorization", format!("Device {credential}"))
        .json(message)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

fn parse_next_sse(buffer: &mut String) -> Option<(String, String)> {
    let (boundary, delimiter_length) = buffer
        .find("\r\n\r\n")
        .map(|index| (index, 4))
        .or_else(|| buffer.find("\n\n").map(|index| (index, 2)))?;
    let block = buffer[..boundary].trim_end_matches('\r').to_string();
    buffer.drain(..boundary + delimiter_length);
    let mut event = String::new();
    let mut data = Vec::new();
    for line in block.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event: ") {
            event = value.to_string();
        } else if let Some(value) = line.strip_prefix("data: ") {
            data.push(value);
        }
    }
    Some((event, data.join("\n")))
}

fn parse_sse_data(
    buffer: &mut String,
    expected_event: &str,
) -> Result<Option<String>, std::io::Error> {
    while let Some((event, data)) = parse_next_sse(buffer) {
        if event == expected_event {
            return Ok(Some(data));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fragmented_sse_and_ignores_keepalive_blocks() {
        let mut buffer =
            ": connection-alive\n\nevent: connected\ndata: {\"session_id\":".to_string();
        assert!(parse_sse_data(&mut buffer, "connected").unwrap().is_none());
        buffer.push_str("\"session-1\"}\n\n");
        assert_eq!(
            parse_sse_data(&mut buffer, "connected").unwrap().as_deref(),
            Some("{\"session_id\":\"session-1\"}")
        );
    }
}
