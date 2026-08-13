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
crates/desktop-automation/       isolated IPC plus Windows target inspection adapter
apps/desktop/                    planned Tauri and React application shell
services/desktop-automation-host/ isolated automation child process
services/desktop-automation-fixture/ deterministic native Windows fixture
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

The Device maintains an outbound HTTPS server-sent event stream and posts typed `desktop.v1` Heartbeats over the same TLS and proxy-compatible HTTP path. The Platform authenticates the Device Credential, resolves its Tenant from the registry, assigns an opaque connection session, and uses server receipt time rather than trusting the Device clock. Heartbeats report online, busy, awaiting Approval, or degraded state together with Agent version and capabilities. A background guard changes missed-heartbeat sessions to offline after 90 seconds.

TLS terminates at the trusted ingress or reverse proxy, which must overwrite and forward the original scheme. The Platform service port is not exposed directly to untrusted networks; Device endpoints reject requests that do not carry trusted HTTPS transport metadata.

Connection ownership is stored with the Device record so it remains authoritative across Platform replicas. A newer connection atomically replaces the previous session; heartbeats from the former session fail, and each stream periodically checks persisted ownership so replacement, suspension, or revocation disconnects it even when the administrative request reached another replica. The Device reconnect loop uses bounded exponential backoff with 80–120% jitter, resets after a successful long-lived connection, requires an HTTPS Platform URL, and supports an explicit HTTP CONNECT proxy.

Desktop Command dispatch is a durable Platform-owned state machine. Before creating a command, the gateway authorizes the caller role and Tenant, verifies that the Project owns the active Workflow Execution, and checks Device lifecycle state, freshness, advertised capability, and Agent major-version compatibility. Every `desktop.v1` command has a unique command and lease identifier plus a bounded deadline. Commands move through queued, delivered, acknowledged, and an explicit terminal state: succeeded, failed, rejected, cancelled, or timed out.

The active SSE connection carries typed command envelopes and cancellation events. A reconnect replays only unexpired queued or delivered work; the Device acknowledges the matching command, execution, and lease before executing. The Platform accepts a result only after acknowledgement and treats a byte-equivalent repeated result as idempotent, while rejecting conflicting completion attempts so duplicate delivery cannot repeat a completed side effect. PostgreSQL stores the command and redacted lifecycle Audit Log records transactionally for queued, acknowledged, completion, cancellation, and timeout transitions; result payloads remain in the command record rather than being copied into Audit Log detail.

The Device persists a bounded, versioned command recovery journal before starting any side effect. The journal retains completed command identifiers for a configurable safety window and keeps an undelivered typed result until the Platform confirms receipt. A restart can retry only explicitly idempotent work while its original lease remains valid. Expired work becomes timed out, and an interrupted non-idempotent action becomes a terminal recovery failure because its side effect cannot be proven safe to repeat. Corrupt, oversized, duplicate, or unsupported state fails closed; reaching the configured capacity rejects new execution instead of dropping live replay protection.

Recovery storage uses a synchronized temporary file and replace sequence, with a recoverable backup when a process stops during replacement. Unix files are created owner-readable and owner-writable; the Windows application must place the journal under its per-user application-data ACL. The journal excludes action inputs from in-flight records, while pending results remain only until delivery acknowledgement. Recovery emits redacted local Audit Log records and one-shot degraded health signals without placing command identifiers or action payloads in heartbeat detail.

## Protocol

The first wire family is `desktop.v1`, represented by `desktop-protocol` Rust types and Serde JSON. Every envelope contains:

- `protocol_version`
- `protocol_revision`
- globally unique `message_id`
- sender timestamp
- typed payload

Every Desktop Command contains Tenant and Project scope, requesting actor, Workflow Execution identifier, a command identifier, an expiring lease, and one typed action. The local runtime rejects expired leases and duplicate completed commands.

The production transport will use TLS with an authenticated outbound connection from the Device. Transport selection does not change the domain messages. Within `desktop.v1`, revision 2 is current and revision 1 is the previous supported desktop release shape. A missing revision is interpreted as revision 1 so already deployed messages remain compatible; any other revision fails closed. The Platform must support the current and previous revisions during staged desktop upgrades. Breaking semantic changes require a new protocol family and a migration window; fields cannot be silently reinterpreted.

Canonical current and previous fixtures live under `crates/desktop-protocol/fixtures/`. Current fixtures promise byte-stable pretty JSON for pairing, heartbeat, command, result, error, and Approval messages. Previous fixtures promise semantic round-trip compatibility, including safe defaults for fields added later. Workspace Rust tests exercise both fixture generations in CI and reject unknown versions, revisions, actions, extra fields, control characters, and invalid bounds.

Removing the previous revision is a release decision, not a routine code cleanup. It requires persisted Device fleet telemetry proving the revision is no longer active, a staged migration window, and an ADR that records the evidence, rollback plan, and accountable owner before the supported revision constants change.

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

Windows automation executes in the `desktop-automation-host` child process, not in the presentation shell or Device connection loop. Its stdin/stdout boundary accepts one newline-delimited, size-bounded typed request at a time. Requests carry a unique identifier and absolute deadline; malformed, oversized, expired, unknown, and unsupported operations fail closed. Health, execute, cancel, and shutdown are explicit operations, and every response has an explicit ready, succeeded, rejected, cancelled, failed, or shutting-down status.

The parent Device remains responsible for protocol validation, command leases, policy, Approval, persisted replay state, process startup deadlines, request timeouts, and terminating a hung host. The child receives only the already-authorized typed action plus opaque command and lease identifiers. A child crash therefore cannot grant authority, modify replay state, or terminate the connection owner. Restarting the child never implies permission to replay a side effect.

`trigix-desktop-automation` provides the shared IPC contract and a deterministic non-Windows fixture adapter so Linux workspace CI exercises the same message boundary. `desktop-automation-fixture` builds a native Windows fixture window with stable class/control identifiers, including a normal input, submit button, and protected password input. The fixture is test-only and must be signed by the release pipeline before it is used on Windows qualification runners.

Read-only target inspection is a typed low-risk Desktop action. Its request bounds element depth and count, wall-clock duration, and serialized payload size below the Host IPC limit. Results identify windows by executable, process identifier, title disclosure policy, and stable automation identifier. Elements carry a window-scoped semantic selector and only the automation patterns the adapter can support. Invisible controls are excluded. Password controls, credential-like labels, and oversized text never return their value; the result records only a redaction reason.

Every inspection returns a snapshot identifier derived from the observed target structure. A caller may provide the expected identifier when re-inspecting immediately before execution; a mismatch returns `target_stale`, while an empty match returns `target_not_found`. Future mutating actions must resolve selectors against a fresh snapshot and return `target_ambiguous` rather than choosing among multiple matches. Truncation is explicit, so authoring tools cannot mistake a bounded partial tree for a complete application model.

On Windows, the Host enumerates visible top-level windows and their visible native controls, resolves process executable names with limited query rights, and maps standard control classes to supported semantic patterns. The deterministic fixture covers duplicate executable matches, localized accessible names, missing targets, stale snapshots, and protected password values. Rich UI Automation provider traversal can extend the adapter behind the same result contract without changing Device or Platform authority.

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

The simulator is deterministic in shape and intentionally does not register a real Device or persist Credentials. Connected mode persists command recovery state at `DESKTOP_RECOVERY_STATE_PATH`, or at a stable per-Device temporary path when the variable is absent.

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
