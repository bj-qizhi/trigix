# ADR 0012: Bounded avatar presentation

## Status

Accepted

## Context

A virtual avatar needs to reflect realtime conversation without becoming a second Agent, an automation controller, or an ungoverned media loader. Conversation content, provider audio, Device Credentials, Tool proposals, Approval grants, and operating-system actions cross different trust boundaries and must not become renderer input. Commercial avatar packages also carry licensing, integrity, supply-chain, memory, and device-loss risks that the public repository cannot absorb implicitly.

## Decision

Avatar input is a versioned `AvatarPresentationEvent` contract in `desktop-protocol`. It accepts only a session identifier, monotonic sequence and timestamp, and a closed event enum: state, bounded viseme timing and intensity, bounded emotion, interruption, fixed failure category, or stop. Unknown fields and unsupported versions fail closed. There is no field for transcript, raw audio, Tool, Approval, Credential, URL, script, media payload, or Desktop action.

`desktop-avatar` owns the deterministic presentation state machine. Visemes are legal only while speaking; interruption, stop, background suspension, and device recovery reset the mouth to rest. A no-motion preference suppresses visemes. Disabling the avatar immediately enters a terminal stopped state for that renderer instance. The crate has no dependency on the Desktop automation runtime, identity store, network client, or Platform command gateway.

The Desktop shell ships a code-native fallback face. It consumes conversation state and a transient bounded level derived from the remote WebRTC media track. It never records, serializes, persists, logs, or sends the track or time-domain buffer to IPC. Local preferences may persist only enablement, voice playback, motion, captions, and high contrast. A separate Stop control, hidden-window transition, page teardown, and realtime transport cleanup stop presentation and release its Web Audio graph.

Optional licensed packages are disabled unless deployment code constructs an explicit non-empty allow-list of clean HTTPS origins. Before activation, the package policy validates bounded identifier and version syntax, declared and actual size, a 64 MiB maximum, SHA-256 digest, and a deployment-owned signature verifier. The public repository contains no licensed package, customer likeness, recorded voice, generated media, private source URL, signing key, or test Credential. Any validation or renderer failure selects the built-in fallback without affecting voice, Workflow, or automation state.

`avatar_rendering` is a qualified Device capability, not a static claim. The native shell enables it only when the renderer stays within a two-second startup budget, 33.334 ms p95 frame budget, 256 MiB memory budget, and five-percent dropped-frame budget and supplies deterministic evidence for resize, device loss, background suspension, interruption recovery, and a 60-minute session. Pairing restart and credential removal clear qualification. Failure removes the capability; it cannot manufacture another capability or an action.

## Consequences

- Avatar rendering remains presentation-only and cannot expand Agent or Device authority.
- Accessibility, motion, playback, and immediate stop remain under the local user's control.
- The built-in fallback works without third-party assets or network media loading.
- Licensed packages require an explicit private distribution and signature-verification integration.
- Performance and recovery regressions remove capability advertisement instead of degrading into an unbounded renderer.
