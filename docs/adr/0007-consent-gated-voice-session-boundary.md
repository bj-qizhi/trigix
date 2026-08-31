# ADR 0007: Consent-gated voice session boundary

## Status

Accepted

## Context

Realtime voice requires microphone access, partial and final transcripts, provider reconnects, and speech output. These signals are sensitive and arrive outside the typed Workflow editor. Treating them as Desktop actions or allowing presentation code to authorize automation would bypass the existing Tool, policy, Approval, and audit boundary.

## Decision

Trigix Desktop starts microphone access only from an explicit local user gesture. The shell keeps microphone use visibly announced, exposes an immediate Stop control, and releases all media tracks when the user stops, the window becomes hidden, the input ends, or the page tears down. Permission denial and missing devices fail closed. The foundation does not record, persist, or transmit audio.

Voice conversation events use their own versioned, provider-neutral contract. It bounds session identifiers, monotonic sequence values, timestamps, transcript payloads, reconnect attempts, delays, fixed interruption reasons, and fixed failure categories. Voice latency telemetry contains only state and bounded duration values; it cannot contain raw audio, transcripts, provider payloads, credentials, or arbitrary error detail.

A final transcript may later enter the same authenticated Agent conversation ingress as typed input. It cannot become a `DesktopAction`, an Approval grant, or an operating-system call. Any requested action must still be selected as a typed Tool and pass Platform authorization, Device capability checks, local policy, command-specific Approval, lease validation, and audit recording.

The Device does not advertise `voice_conversation` merely because local consent UI exists. That capability remains disabled until an authenticated streaming path, provider abstraction, privacy policy, and end-to-end qualification are present.

## Consequences

- Local consent and revocation behavior can be qualified before a speech provider is selected.
- Provider integrations can evolve behind a stable conversation event contract.
- Voice telemetry is operationally useful without becoming a content retention channel.
- Hidden-window stopping favors privacy over background continuity; a user must explicitly restart listening.
- Streaming recognition, synthesis, Device switching, conversation ingress, and long-session qualification remain later vertical slices.
