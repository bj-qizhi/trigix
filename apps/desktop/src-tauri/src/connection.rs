use desktop_protocol::{DesktopCommand, DesktopCommandCancellation, Envelope};
use serde::Deserialize;
use std::time::Duration;

const MAX_SSE_BUFFER_BYTES: usize = 64 * 1024;
const MIN_RECONNECT_SECONDS: u64 = 1;
const MAX_RECONNECT_SECONDS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Default)]
pub(crate) struct SseDecoder {
    buffer: Vec<u8>,
}

impl SseDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, ConnectionError> {
        if self.buffer.len().saturating_add(bytes.len()) > MAX_SSE_BUFFER_BYTES {
            return Err(ConnectionError::InvalidStream);
        }
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some((end, delimiter)) = event_boundary(&self.buffer) {
            let frame = self.buffer.drain(..end).collect::<Vec<_>>();
            self.buffer.drain(..delimiter);
            if let Some(event) = parse_frame(&frame)? {
                events.push(event);
            }
        }
        Ok(events)
    }
}

fn event_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let crlf = buffer.windows(4).position(|window| window == b"\r\n\r\n");
    let lf = buffer.windows(2).position(|window| window == b"\n\n");
    match (crlf, lf) {
        (Some(left), Some(right)) if left <= right => Some((left, 4)),
        (Some(_), Some(right)) => Some((right, 2)),
        (Some(left), None) => Some((left, 4)),
        (None, Some(right)) => Some((right, 2)),
        (None, None) => None,
    }
}

fn parse_frame(frame: &[u8]) -> Result<Option<SseEvent>, ConnectionError> {
    let frame = std::str::from_utf8(frame).map_err(|_| ConnectionError::InvalidStream)?;
    let mut event = None;
    let mut data = Vec::new();
    for line in frame.lines() {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim_start().to_owned());
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.trim_start());
        }
    }
    match (event, data.is_empty()) {
        (None, true) => Ok(None),
        (Some(event), false) if event.len() <= 64 => Ok(Some(SseEvent {
            event,
            data: data.join("\n"),
        })),
        _ => Err(ConnectionError::InvalidStream),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConnectedEvent {
    pub device_id: String,
    pub session_id: String,
    pub server_time_unix_ms: u64,
    pub heartbeat_interval_seconds: u32,
}

pub(crate) fn reconnect_delay(attempt: u32, entropy: u64) -> Duration {
    let exponent = attempt.min(6);
    let base = MIN_RECONNECT_SECONDS
        .saturating_mul(1_u64 << exponent)
        .min(MAX_RECONNECT_SECONDS);
    let jitter_percent = 80 + entropy % 41;
    Duration::from_millis(base.saturating_mul(1_000) * jitter_percent / 100)
}

pub(crate) fn parse_command_event(
    data: &str,
    now_unix_ms: u64,
) -> Result<DesktopCommand, ConnectionError> {
    let envelope: Envelope<DesktopCommand> =
        serde_json::from_str(data).map_err(|_| ConnectionError::InvalidStream)?;
    envelope
        .validate()
        .and_then(|_| envelope.payload.validate(now_unix_ms))
        .map_err(|_| ConnectionError::InvalidStream)?;
    Ok(envelope.payload)
}

pub(crate) fn parse_cancellation_event(
    data: &str,
    expected_command_id: &str,
    expected_execution_id: &str,
) -> Result<DesktopCommandCancellation, ConnectionError> {
    let cancellation: DesktopCommandCancellation =
        serde_json::from_str(data).map_err(|_| ConnectionError::InvalidStream)?;
    if !is_wire_identifier(&cancellation.command_id)
        || !is_wire_identifier(&cancellation.execution_id)
        || cancellation.command_id != expected_command_id
        || cancellation.execution_id != expected_execution_id
        || cancellation.reason.is_empty()
        || cancellation.reason.len() > 256
        || cancellation.reason.chars().any(char::is_control)
    {
        return Err(ConnectionError::InvalidStream);
    }
    Ok(cancellation)
}

fn is_wire_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionError {
    InvalidStream,
    Transport,
    CredentialRejected,
    UnsupportedCommand,
    Unpaired,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_handles_fragmented_frames_keepalives_and_crlf() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push(b": keepalive\n\n").unwrap().is_empty());
        assert!(decoder.push(b"event: connec").unwrap().is_empty());
        let events = decoder
            .push(b"ted\r\ndata: {\"session_id\":\"one\"}\r\n\r\n")
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "connected");
        assert_eq!(events[0].data, "{\"session_id\":\"one\"}");
    }

    #[test]
    fn decoder_bounds_untrusted_stream_data_and_rejects_ambiguous_frames() {
        let mut decoder = SseDecoder::default();
        assert_eq!(
            decoder.push(&vec![b'x'; MAX_SSE_BUFFER_BYTES + 1]),
            Err(ConnectionError::InvalidStream)
        );
        assert_eq!(
            SseDecoder::default().push(b"data: missing-event\n\n"),
            Err(ConnectionError::InvalidStream)
        );
    }

    #[test]
    fn reconnect_backoff_is_bounded_and_jittered() {
        assert_eq!(reconnect_delay(0, 0), Duration::from_millis(800));
        assert_eq!(reconnect_delay(0, 40), Duration::from_millis(1_200));
        for attempt in 0..100 {
            let delay = reconnect_delay(attempt, u64::from(attempt));
            assert!(delay >= Duration::from_millis(800));
            assert!(delay <= Duration::from_secs(72));
        }
    }

    #[test]
    fn command_events_require_a_valid_current_envelope_and_live_lease() {
        let command = serde_json::json!({
            "protocol_version": "desktop.v1",
            "protocol_revision": 2,
            "message_id": "message-1",
            "sent_at_unix_ms": 1_000,
            "payload": {
                "command_id": "command-1",
                "execution_id": "execution-1",
                "tenant_id": "tenant-1",
                "project_id": "project-1",
                "requested_by": "user-1",
                "issued_at_unix_ms": 1_000,
                "lease": {"lease_id": "lease-1", "expires_at_unix_ms": 2_000},
                "action": {"kind": "read_system_information"}
            }
        });
        assert_eq!(
            parse_command_event(&command.to_string(), 1_100)
                .unwrap()
                .command_id,
            "command-1"
        );
        assert_eq!(
            parse_command_event(&command.to_string(), 2_000),
            Err(ConnectionError::InvalidStream)
        );
        let mut unsupported = command;
        unsupported["protocol_version"] = serde_json::json!("desktop.v2");
        assert_eq!(
            parse_command_event(&unsupported.to_string(), 1_100),
            Err(ConnectionError::InvalidStream)
        );
    }

    #[test]
    fn cancellation_is_bound_to_both_active_identifiers() {
        let cancellation = serde_json::json!({
            "command_id": "command-1",
            "execution_id": "execution-1",
            "reason": "operator_requested"
        })
        .to_string();
        assert!(parse_cancellation_event(&cancellation, "command-1", "execution-1").is_ok());
        assert_eq!(
            parse_cancellation_event(&cancellation, "command-1", "different-execution"),
            Err(ConnectionError::InvalidStream)
        );
    }
}
