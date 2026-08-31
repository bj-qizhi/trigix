# ADR 0011: Authenticated realtime voice transport

## Status

Accepted

## Context

Realtime audio needs a direct low-latency media path, but a Desktop WebView must never receive the provider's long-lived API key. Final transcripts still need the existing Tenant policy, replay, retention, Tool proposal, confirmation, Workflow Approval, and Audit boundaries. Provider events and failures are untrusted input and may contain content that is unsafe to log.

## Decision

The paired Device asks the Platform for a versioned short-lived bootstrap using its Device Credential over TLS. The Platform authenticates the active Device, resolves Tenant and approving actor from the registry, reads the effective voice privacy policy, and calls the provider client-secret endpoint with the long-lived API key. The provider request fixes the realtime, transcription, voice, noise-reduction, and server-VAD models; disables provider Tools; and uses a one-way safety identifier derived from Tenant, Device, and actor.

Only the provider-issued `ek_` client secret is returned. It may exist transiently in Desktop process and WebView memory solely for the SDP exchange. It is never placed in Credential storage, local storage, events, telemetry, Audit detail, or logs. The Platform API key remains server-only. Client-secret setup validity is two minutes; an established session is locally capped at 55 minutes.

The WebView sends the microphone track directly to the fixed provider WebRTC endpoint. Platform, Tauri IPC, Audit, crash reporting, and application telemetry never receive raw audio. The Content Security Policy permits network access only to the exact provider API origin in addition to Tauri IPC.

The provider data channel is parsed through a closed allow-list. Only a bounded final input-transcription event is accepted. Tauri posts it through Device-authenticated Platform ingress, where the server replaces all claimed identity with the registered Tenant and Device binding and authorizes the live session grant. The existing conversation store supplies monotonic sequence replay protection, retention, redaction, encryption, and Tenant isolation.

Provider Tools are disabled. A final transcript can lead to an application Tool proposal only through the separate typed proposal contract; proposal creation has no execution side effect, and explicit Tenant-administrator confirmation still enters the existing Workflow policy and Approval path.

Stop, window hiding, page teardown, capture loss, session expiry, exhausted reconnect, transcript rejection, suspension, and revocation close local tracks, Web Audio, data channel, peer connection, remote playback, expiry timers, and reconnect timers. Device suspension or revocation also removes server-side grants. Grants are process-local and fail closed after Platform restart.

The Device advertises `voice_conversation` only after a successful authenticated bootstrap and an open WebRTC connection plus provider data channel prove provider configuration, Device authentication, policy access, and the CI-qualified client path. Until then the capability is absent from pairing and heartbeat descriptors.

Latency, reconnect, interruption, failure, and stop telemetry uses a separate Device-authenticated closed schema. It accepts only a fixed event, an optional bounded duration, reconnect attempt, and fixed failure category. Unknown fields and arbitrary provider error strings are rejected, so telemetry cannot carry transcript or audio content.

## Consequences

- Long-lived provider credentials cannot cross the Platform trust boundary.
- The direct media path avoids raw-audio persistence while final text remains governed by Tenant policy.
- Provider payloads cannot manufacture a Tool call or Desktop action.
- Restart, expiry, revocation, and identity mismatch fail closed and require a fresh bootstrap.
- Live-provider smoke tests remain opt-in; deterministic contracts, state machines, noise, interruption, reconnect, and long-session fixtures remain the required CI evidence.
