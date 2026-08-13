# Trigix Desktop Product Foundation

## Status

Accepted foundation for the Trigix Desktop product line. This document defines the boundaries that must remain stable while Windows automation, voice conversation, avatar rendering, and enterprise deployment are implemented as vertical slices.

## Product Boundary

Trigix Desktop is a managed desktop runtime for Trigix Workflows. A Workflow can invoke an approved desktop Tool; the Platform converts that invocation into a typed Desktop Command and assigns it to a paired Device. The local runtime validates the command, evaluates policy, obtains Approval when required, invokes a platform-specific adapter, and returns a typed result and Audit Log events.

The desktop runtime is not an unrestricted remote shell. An Agent can select only registered Tools and typed actions. Model output never bypasses protocol validation, policy, Approval, local execution boundaries, or audit recording.

## Repository Layout

```text
crates/desktop-protocol/         versioned platform/device messages and validation
crates/desktop-agent-core/       policy, Approval, replay protection, execution, audit
crates/desktop-identity/         Ed25519 identity and operating-system Credential boundary
crates/desktop-host/             tested presentation/host IPC boundary
crates/desktop-windows/          planned Windows UI Automation adapter
apps/desktop/                    planned Tauri and React application shell
services/desktop-device-simulator/ deterministic protocol and execution simulator
services/platform-rs/            device pairing service; registry and command gateway follow
```

The protocol and policy crates are independent of Tauri and Windows APIs. This makes security rules testable on Linux CI and prevents presentation code from becoming an execution authority.

## Trust Boundaries

```text
Workflow / Agent
      |
      | approved Tool invocation
      v
Platform policy and tenant authorization
      |
      | signed, leased, typed Desktop Command
      v
Paired Device connection
      |
      v
Protocol validation -> replay check -> local policy -> Approval
      |
      v
Platform adapter -> operating system
      |
      v
typed result + immutable Audit Log event
```

Every boundary fails closed. Missing tenant context, an invalid protocol version, an expired lease, an unknown action, a duplicate command, or a missing Approval prevents execution.

## Device Lifecycle

1. The desktop runtime generates a device key pair in the operating-system credential store.
2. A user starts pairing and receives a short-lived, single-use pairing code.
3. The Platform binds the device public key to one Tenant and records the user who approved pairing.
4. The Platform issues a revocable device Credential reference. Credential plaintext is not returned to the web frontend.
5. The Device opens an authenticated outbound connection and sends heartbeats.
6. Administrators can suspend, rotate, or revoke the Device without uninstalling it.

The first Platform pairing vertical slice is implemented. A Device creates a short-lived session with its locally generated Ed25519 public key and receives a pairing code plus a separate claim secret. `desktop-identity` generates the private key locally, fails closed unless it can persist it, and uses Windows Credential Manager in production Windows builds. A Tenant administrator approves the code, atomically binding the Device, Tenant, public key, and approving actor. The Device then uses the claim secret exactly once to retrieve its Credential. The administrative response and Audit Log never contain the claim secret or Credential plaintext.

Pairing state is persisted in `af_desktop_pairing_sessions` and `af_desktop_devices`. Claim secrets and active device Credentials are stored only as hashes; the pending one-time Credential is protected with the configured Credential master key and erased after claim. Expiry, bounded attempts, per-endpoint rate limits, row locking, unique device/public-key constraints, and Tenant row-level-security policies make reuse and concurrent redemption fail closed.

The Device registry is the administrative source of truth for identity, operating system, Agent version, declared capabilities, lifecycle state, last-seen time, and stale status. Registry reads and mutations require a Tenant administrator and always scope queries by Tenant. List operations use bounded pagination and optional lifecycle-state filtering; cross-Tenant lookup deliberately returns not found.

Suspension and revocation fail Device authentication before heartbeat or command work can be accepted. Revocation also clears the active Credential hash and any pending rotation, and is irreversible through the registry API. Credential rotation is a two-party exchange: an administrator creates a pending rotation without receiving plaintext, then the authenticated Device claims the replacement exactly once using its current Credential. The replacement is encrypted while pending, claimed under a row lock, and atomically replaces the old hash so concurrent claims have one winner. Rename, suspend, revoke, and rotation events are recorded without Credential material.

## Protocol

The first wire contract is `desktop.v1`, represented by `desktop-protocol` Rust types and Serde JSON. Every envelope contains:

- `protocol_version`
- globally unique `message_id`
- sender timestamp
- typed payload

Every Desktop Command contains Tenant and Project scope, requesting actor, Workflow Execution identifier, a command identifier, an expiring lease, and one typed action. The local runtime rejects expired leases and duplicate completed commands.

The production transport will use TLS with an authenticated outbound connection from the Device. Transport selection does not change the domain messages. The Platform must support the current protocol and one previous compatible version during staged desktop upgrades. Breaking changes require a new protocol version and a migration window; fields cannot be silently reinterpreted.

## Action and Risk Model

| Action class | Default risk | Default handling |
| --- | --- | --- |
| Read non-sensitive system information | Low | Automatic |
| Focus a window or click a selected element | Medium | Approval unless explicitly trusted |
| Type text or launch an application | High | Approval required |
| Credentials, destructive filesystem operations, unrestricted commands | Critical | Prohibited until a dedicated policy exists |

Risk is attached to the typed action, not inferred from display text. Tenant policy can be stricter than the default but cannot bypass the product-wide critical-action prohibition.

Approval grants are command-specific, actor-attributed, short-lived, and unusable for a different command. An action waiting for Approval has not completed and can be retried with a matching grant. A completed, rejected, or failed command identifier cannot be replayed.

## Windows Automation Adapter

Windows automation will use this selector order:

1. UI Automation identifier and control type
2. accessible name within a stable window selector
3. application-specific semantic adapter
4. image recognition with confidence threshold and Approval policy
5. coordinates only for an explicitly calibrated and bounded target

Selectors must be resolved immediately before execution. The adapter records which selector strategy was used, the target application identity, timing, and a redacted result. Screenshots are opt-in evidence with Tenant retention policy; they are not unconditional logs.

The Windows adapter runs out of process from the presentation shell where practical. A crash or timeout terminates the action and returns a failure without granting the UI process broader authority.

## Voice and Avatar Boundaries

Voice input produces conversation events; it does not directly produce operating-system calls. Speech recognition output enters the same Agent, Tool, policy, and Approval path as typed input. Microphone use must have a visible local indicator and immediate stop control.

Avatar rendering consumes bounded presentation events such as speaking state, phoneme timing, expression, and interruption. It cannot authorize or execute desktop actions. Licensed models, voices, and customer media remain outside the public source repository.

## First Vertical Slice

The foundation slice is executable now:

```text
construct PairingRequest
    -> validate desktop.v1 envelope
    -> receive leased ReadSystemInformation command
    -> validate scope and lease
    -> evaluate local policy
    -> execute safe platform-neutral action
    -> return typed result
    -> record AuditEvent
```

Run it with:

```bash
cargo run -p desktop-device-simulator
```

The simulator is deterministic in shape and intentionally does not register a real Device or persist Credentials.

## Test Strategy

PR checks must keep protocol serialization, validation, risk classification, Approval behavior, replay prevention, audit generation, and the desktop host boundary covered by unit tests. Existing workspace formatting, Clippy, and tests automatically include the new Rust crates.

Before Windows actions are enabled, add:

- a signed Windows test-fixture application with stable UI Automation elements;
- Windows-hosted adapter integration tests;
- disconnect, reconnect, lease expiry, cancellation, and crash recovery tests;
- protocol compatibility fixtures for every supported desktop version;
- installer, upgrade, rollback, and code-signing verification;
- security tests proving a public fork cannot reach privileged self-hosted runners.

## Delivery Sequence

1. Device registry, pairing exchange, heartbeat, revocation, and authenticated connection.
2. Platform command gateway and end-to-end result/Audit Log persistence.
3. Windows fixture application and read-only UI Automation inspection.
4. Window, click, text, and application actions behind policy and Approval.
5. Desktop automation authoring and debugging experience.
6. Realtime voice conversation through the existing Agent boundary.
7. Avatar presentation events and rendering.
8. signed installer, staged updater, rollback, enterprise controls, and release qualification.

Each stage must deliver a working vertical path. Parallel feature work cannot weaken tenant isolation, Credential handling, Approval, or Audit Log guarantees.
