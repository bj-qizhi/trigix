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
}
