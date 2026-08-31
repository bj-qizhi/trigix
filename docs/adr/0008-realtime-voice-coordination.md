# ADR 0008: Realtime voice coordination and local device switching

## Status

Accepted

## Context

Realtime conversation must coordinate audio capture, speech recognition, speech synthesis, voice activity, interruption, provider outages, and input-device changes. Provider SDKs have different callbacks and failure payloads, while microphone identifiers and raw audio are sensitive local data. Coupling these concerns directly to the shell would make behavior difficult to test and could accidentally expand presentation authority.

## Decision

Recognition and synthesis implement separate provider interfaces behind a deterministic `desktop-voice` coordinator. Provider failures map to fixed categories and a retryable flag; arbitrary provider error bodies never enter conversation events or telemetry. The coordinator emits only validated `desktop-protocol` voice events with monotonic sequence and timestamp checks.

Voice activity detection uses bounded PCM frame sizes, an RMS threshold, consecutive onset frames, and a silence hangover. Short noise or echo-like bursts do not open an utterance. Speech detected while synthesis is active cancels synthesis and emits an interruption before listening resumes.

Reconnect attempts and exponential delays have fixed upper bounds. A local Stop closes recognition and synthesis, resets VAD state, and permanently prevents that coordinator instance from reconnecting.

The shell enumerates microphone devices only after microphone permission succeeds. Device identifiers remain in the live DOM and capture constraints only; they are not persisted, sent as telemetry, or written to logs. Switching first acquires and validates a replacement audio track, then activates its analysis path, and only then releases the previous stream. Failed switching leaves the current live stream intact.

The shell derives a transient activity level through Web Audio time-domain samples. It does not create a `MediaRecorder`, audio blob, base64 payload, or durable sample buffer. Page hiding, teardown, explicit Stop, and ended tracks still release capture and analysis resources.

## Consequences

- Provider adapters can change without changing conversation semantics.
- Noise, interruption, degradation, and long-session behavior are deterministic in Linux CI.
- Input switching preserves continuity without persisting hardware identifiers.
- The local activity meter is presentation state, not transcript or audit evidence.
- Production capability advertisement remains disabled until authenticated streaming, Tenant policy, and end-to-end provider qualification are implemented.
