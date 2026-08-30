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
apps/desktop/                    Tauri application shell and local presentation assets
services/desktop-automation-host/ isolated automation child process
services/desktop-automation-fixture/ deterministic native Windows fixture
services/desktop-device-simulator/ deterministic protocol and execution simulator
services/platform-rs/            device pairing service; registry and command gateway follow
```

The protocol and policy crates are independent of Tauri and Windows APIs. This makes security rules testable on Linux CI and prevents presentation code from becoming an execution authority.

The initial Tauri shell and its least-privilege IPC boundary are defined in [Secure Desktop Shell](secure-desktop-shell.md). Its WebView can read sanitized runtime state and request cancellation only; it cannot submit actions or bypass the Device command path.

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

## Workflow authoring and selector inspection

The workflow editor registers one `desktop` node whose `action_kind` selects a protocol-owned action schema. The editor bounds every field to the same limits enforced by `desktop.v1`: application identities use the restricted identifier alphabet, text is capped at 16,384 characters, and selectors contain only typed window and element fields. The saved graph contains a Device identifier, action kind, typed selector, and action-specific input; it never contains a Device Credential, pairing claim, raw control tree, or authorization grant.

The Device picker reads Tenant-scoped registry metadata through the normal user bearer token. It displays only devices whose heartbeat is current, lifecycle state is eligible, Agent major version matches the Platform, and advertised capabilities cover the selected action. Editors may read this bounded metadata and dispatch low-risk inspection commands; medium- and high-risk test actions retain the command gateway's administrator requirement and command-specific Approval. Device management, pairing approval, Credential rotation, suspension, and revocation remain administrator-only.

Selector inspection is a test command, not a browser-to-device callback. The author must have an active Workflow Execution in `running` or `waiting_approval`; the browser submits a fixed `inspect_targets` action to the Platform with that Execution, Project, and selected Device. The Platform revalidates ownership, role, Device freshness, compatibility, capability, lease, and action bounds before delivery. It validates the returned `DesktopInspectionResult` again before storing or returning it, rejecting protected elements that combine a redaction reason with a value.

The editor renders only semantic selector fields, supported patterns, and redaction reasons. Selecting a unique window or element persists the inspection snapshot identifier inside the typed selector so execution can detect a changed target. Raw device errors are mapped to fixed author guidance for expired executions, offline or stale devices, incompatible versions or capabilities, missing, ambiguous, or stale targets, and insufficient authorization.

For operator testing, pair and connect the Device, confirm that it advertises the selected capability, then start a Workflow Execution that remains active while inspection completes. A Wait or Approval step is suitable for an authoring session. Re-run inspection after a window structure change, Device reconnect, or `target_stale` response. Standalone Executor processes do not possess Device authority; a `desktop` node submitted directly to an Executor fails closed and desktop commands must pass through the Platform command gateway.

## Windows Automation Adapter

Windows automation executes in the `desktop-automation-host` child process, not in the presentation shell or Device connection loop. Its stdin/stdout boundary accepts one newline-delimited, size-bounded typed request at a time. Requests carry a unique identifier and absolute deadline; malformed, oversized, expired, unknown, and unsupported operations fail closed. Health, execute, cancel, and shutdown are explicit operations, and every response has an explicit ready, succeeded, rejected, cancelled, failed, or shutting-down status.

The parent Device remains responsible for protocol validation, command leases, policy, Approval, persisted replay state, process startup deadlines, request timeouts, and terminating a hung host. The child receives only the already-authorized typed action plus opaque command and lease identifiers. A child crash therefore cannot grant authority, modify replay state, or terminate the connection owner. Restarting the child never implies permission to replay a side effect.

`trigix-desktop-automation` provides the shared IPC contract and a deterministic non-Windows fixture adapter so Linux workspace CI exercises the same message boundary. `desktop-automation-fixture` builds a native Windows fixture window with stable class/control identifiers, including a normal input, submit button, and protected password input. The fixture is test-only and must be signed by the release pipeline before it is used on Windows qualification runners.

Read-only target inspection is a typed low-risk Desktop action. Its request bounds element depth and count, wall-clock duration, and serialized payload size below the Host IPC limit. Results identify windows by executable, process identifier, title disclosure policy, and stable automation identifier. Elements carry a window-scoped semantic selector and only the automation patterns the adapter can support. Invisible controls are excluded. Password controls, credential-like labels, and oversized text never return their value; the result records only a redaction reason.

Every inspection returns a snapshot identifier derived from the observed target structure. A caller may provide the expected identifier when re-inspecting immediately before execution; a mismatch returns `target_stale`, while an empty match returns `target_not_found`. Future mutating actions must resolve selectors against a fresh snapshot and return `target_ambiguous` rather than choosing among multiple matches. Truncation is explicit, so authoring tools cannot mistake a bounded partial tree for a complete application model.

On Windows, the Host enumerates visible top-level windows and their visible native controls, resolves process executable names with limited query rights, and maps standard control classes to supported semantic patterns. The deterministic fixture covers duplicate executable matches, localized accessible names, missing targets, stale snapshots, and protected password values. Rich UI Automation provider traversal can extend the adapter behind the same result contract without changing Device or Platform authority.

Window focus resolves the selector again immediately before calling the operating system. A selector can carry the inspection snapshot that produced it; a changed snapshot, zero matches, multiple matches, or foreground access denial returns a distinct failure and never falls back to a best-effort window. The result exposes only the process identifier and selector strategy, not window text or executable paths.

Application launch accepts a typed application identity containing only ASCII identifier characters. The Windows Host maps that identity through `TRIGIX_DESKTOP_APPLICATION_ALLOWLIST`, a JSON array of unique identity and absolute executable-path registrations supplied by trusted local deployment configuration. It starts the exact registered executable with no arguments and never invokes a command shell. An absent identity, invalid allowlist, operating-system denial, and process creation failure fail explicitly. Executable paths are not returned in command results.

Focus remains a medium-risk action and launch remains high risk, so local policy and command-specific Approval are authoritative before the Host is invoked. The Host validates the request deadline and execution lease on receipt and rechecks them immediately before adapter dispatch. Cancellation remains owned by the parent Device, which terminates an in-flight Host action when required; the child process cannot extend a lease or convert cancellation into permission.

Element actions reuse the inspected window-scoped selector and snapshot. The Windows adapter resolves the window and control again, requires exactly one visible match, and verifies that the target window is still in the foreground. Click invokes a supported semantic control pattern (`Invoke` for native buttons); it does not fall back to screen coordinates. Text entry supports native editable value controls, rejects password controls before writing, and reads the value back to detect partial entry. A focus change, stale snapshot, ambiguous control, unsupported pattern, protected control, access denial, or incomplete verification is an explicit terminal error.

Text remains present only in the authorized command and the transient Host request needed to perform the action. Adapter results contain a character count and semantic pattern but never the text, and failure messages are fixed rather than derived from input. Audit and recovery records continue to store action class and outcome only. Because typing is non-idempotent, replay protection persists the in-flight command before dispatch and never repeats an uncertain write after interruption.

The Device owns an `AutomationHostSupervisor` and never lends process ownership to UI or adapter code. A supervisor accepts one command at a time for its Device, so queued and active work obey a limit of one and actions for the same application cannot overlap. Waiting for that permit remains bounded by the command deadline and observes cancellation. Each admitted request receives a fresh short-lived Host process; stdin is closed after the one typed request, and the parent waits for or forcibly reaps the child after every terminal path.

The parent polls an explicit cancellation token and the absolute request deadline while the Host is active. Cancellation or timeout terminates and reaps a hung Host without terminating the Device connection process, returning `cancelled` or `timed_out` as a typed command outcome. Empty output is classified as `host_crashed`; malformed or mismatched output is a protocol failure. A cancellation handle is independent of the synchronous command processor, allowing connection work to cancel active or queued commands without taking execution ownership.

Before adapter dispatch, the Host validates the lease again. Production Windows adapters also recheck an execution guard after target resolution and immediately before every focus, launch, invoke, or value side effect. A lease or request deadline that expires during resolution therefore prevents the operating-system call. The command processor persists in-flight state before the supervisor starts: parent crashes retry only read-only idempotent work, while an uncertain write becomes a terminal recovery failure. Cancelled, timed-out, crashed, and successful results are persisted through the same replay boundary, so reconnect cannot repeat their side effects.

Windows automation will use this selector order:

1. UI Automation identifier and control type
2. accessible name within a stable window selector
3. application-specific semantic adapter
4. image recognition with confidence threshold and Approval policy
5. coordinates only for an explicitly calibrated and bounded target

Selectors must be resolved immediately before execution. The adapter records which selector strategy was used, the target application identity, timing, and a redacted result. Screenshots are opt-in evidence with Tenant retention policy; they are not unconditional logs.

Automation evidence has a separate policy and storage boundary from the command result. A Device may upload adapter-audit metadata only after the Platform has persisted a matching terminal command result. The authenticated Device session, Tenant, Project, Execution, command, Device, and outcome must all match. The schema accepts an enumerated selector strategy, bounded application identity, start and completion times, terminal outcome, redaction policy version, and retention deadline; it has no fields for typed text, credentials, window titles, UI trees, or arbitrary adapter detail.

Screenshot capture is disabled by default. An operator must enable it, and every upload must carry an explicit capture opt-in plus a successful redaction attestation whose sensitive-region and redacted-region counts agree. Only signature-checked PNG or WebP content is accepted, request and decoded sizes are bounded to 1 MiB, and retention cannot exceed Tenant policy. Screenshot persistence requires an encryption key and AES-256-GCM ciphertext; missing encryption, failed redaction, invalid content, or failed durable insertion rejects the capture without a plaintext fallback. API responses, Audit Log records, lists, and exports contain metadata and digests only, never ciphertext or image bytes.

PostgreSQL enforces referential links and tenant row-level security for evidence. Per-record expiry is swept independently of the general data-retention setting. Tenant administrators may export safe metadata or delete an evidence record; deletion removes the encrypted payload while recording only the evidence identifier and actor in the immutable Audit Log. Tenant-scoped lookup and deletion deliberately return no cross-Tenant distinction.

The Windows adapter runs out of process from the presentation shell where practical. A crash or timeout terminates the action and returns a failure without granting the UI process broader authority.

The supported operating-system, session, policy, and resource budgets are defined in [Windows Automation Qualification](windows-automation-qualification.md). Its pinned Windows Server lanes are blocking qualification proxies; signed Windows 11 client smoke evidence remains a release-signing gate.

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
