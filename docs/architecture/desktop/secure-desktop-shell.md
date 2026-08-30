# Secure Desktop Shell

## Status

Accepted implementation boundary for the Windows-first Tauri application shell.

## Authority boundary

The Tauri WebView is presentation code. It cannot launch applications, inspect targets, execute automation, read credentials, approve commands, or submit a raw `DesktopCommand`. Those operations remain behind the authenticated Device connection, `desktop.v1` validation, execution lease, local policy, command-specific Approval, replay journal, isolated automation Host, and Audit Log.

The shell exposes six local IPC operations:

- `shell_status` returns a revision, enumerated connection state, enumerated automation state, and whether stop is currently meaningful.
- `request_automation_stop` accepts a bounded opaque request identifier and the exact state revision observed by the UI.
- `pairing_status` returns only the pairing phase, Device identifier, short code, and expiry needed for presentation.
- `start_device_pairing` accepts a normalized HTTPS Platform origin and bounded display name, then creates a short-lived pairing session using the local Device public key.
- `complete_device_pairing` explicitly claims an approved session and reports paired only after the Credential is persisted.
- `forget_device_pairing` deletes the local Device Credential; it does not disguise this local action as server-side revocation.

A stop request is a cancellation signal only. It cannot start, retry, approve, retarget, or modify an action. The runtime consumes the signal and remains responsible for propagating cancellation to queued or active supervisor work.

## Tauri controls

The main local window is the only member of the `main-shell` capability. The capability allowlists only the six application commands above and grants no filesystem, shell, opener, HTTP, clipboard, process, or automation plugin permission. Remote origins are not configured. Pairing secrets, the Device private key, the Device Credential, connection session identifiers, response bodies, and transport errors never cross IPC.

Static UI assets are bundled locally. The Content Security Policy permits self-hosted scripts and styles plus Tauri's local IPC transport; it blocks external origins, objects, frames, base rewrites, and form submission. JavaScript prototype freezing is enabled. IPC structs deny unknown fields, mutation identifiers have a fixed character and size grammar, and stale state revisions or repeated identifiers fail closed.

The Rust state controller is platform-neutral and tested in Linux workspace CI. Native Tauri and WebView dependencies are Windows-target-only so server CI does not require a graphical desktop stack. Windows CI must compile the shell target as part of the supported desktop qualification lane.

## Pairing and connection lifecycle

The Windows runtime loads or creates the Ed25519 Device identity in Windows Credential Manager. Its stable Device identifier is derived from the public key; the private key is never serialized into pairing HTTP or IPC. Pairing accepts only a clean HTTPS origin with no user information, path, query, fragment, whitespace, or unbounded input. The claim secret remains in Rust memory only while approval is pending. The claimed Device Credential and normalized Platform origin are stored together under a per-Device Windows Credential Manager target before the state changes to paired.

After pairing, a background runtime opens the authenticated outbound server-sent event connection with `x-device-id` and the Device authorization scheme. It validates the first `connected` event, posts typed `desktop.v1` heartbeats with the assigned session, and drives the shell through offline, connecting, online, and degraded states. Request redirects are disabled, transport is HTTPS-only, streamed input is UTF-8 checked and bounded to 64 KiB, and server payloads are never shown in the WebView.

At startup the shell locates the packaged `desktop-automation-host.exe` beside its own executable, starts it through `AutomationHostSupervisor`, and requires a bounded successful health exchange before advertising any automation capability. A missing, incompatible, or unhealthy Host leaves the sanitized Automation Host state `unavailable` and the capability set empty; the shell never falls back to running automation inside the WebView or shell process.

For a validated live command, the Device first posts the acknowledgement and only then passes the typed action to the isolated Host. Heartbeats continue while the action runs and report the active execution identifier. A matching server cancellation or a revision-checked local stop is propagated to the Host. The terminal result is posted to the Platform, and the local recovery entry is confirmed only after that post succeeds. An `awaiting_approval` response is exposed as non-terminal shell state and is never mistaken for a persisted terminal recovery entry. A disconnect cancels active work; reconnect recovery may resend a pending terminal result but cannot re-execute an already terminal command. Receiving an invalid command or cancellation fails closed and reconnects as degraded instead of acknowledging or silently discarding work.

Reconnect delay grows exponentially to a 60-second base cap with 80–120% jitter. A successful connection resets the attempt count. Authentication rejection remains visibly degraded and retries no faster than once per minute; local unpair moves the shell offline.

## Packaging and upgrade boundary

`apps/desktop/scripts/prepare-sidecar.ps1` builds the Host for the same explicit target triple and configuration as the shell, then places the target-suffixed binary under `src-tauri/binaries`. Tauri validates that sidecar at bundle time and installs it under the unsuffixed executable name expected by the runtime. Windows CI compiles the native fixture and creates an unsigned NSIS qualification bundle. Authenticode signing, publisher identity, timestamping, and installer reputation remain release gates; the qualification artifact must not be represented as a production-signed installer.

The recovery journal lives in the application's per-user local data directory so normal Windows user ACLs protect it. It contains bounded protocol metadata needed for idempotency plus any pending, sanitized terminal result; it does not retain credentials or original command action arguments. Shell and Host must be packaged, upgraded, and rolled back as one compatible unit, while the recovery journal is preserved across an upgrade. Operators must not copy the journal between users or Devices.

## Failure behavior

- Poisoned or unavailable state returns a typed error and disables the UI stop control.
- An idle runtime rejects stop rather than manufacturing a successful side effect.
- A stale view must refresh before sending another mutation.
- A repeated request identifier is rejected within a bounded replay window.
- The UI displays fixed error copy and does not render backend error detail.
- Pairing transport failure leaves the session unpaired or pending and never manufactures a Credential.
- Invalid or mismatched Platform responses fail closed before secure persistence.
- Credential Manager read, write, or delete failure makes pairing unavailable or degraded rather than falling back to plaintext storage.
- Revocation, suspension, replacement by a newer session, malformed SSE, unsupported commands, and heartbeat rejection close the active connection and surface a sanitized degraded state.
- A missing or unhealthy packaged Host keeps the connection available for lifecycle reporting but advertises zero automation capabilities and rejects execution. Restore the matching signed shell/Host package rather than copying an arbitrary executable into place.
- A result-delivery interruption leaves bounded recovery state for reconnect. Preserve the local application-data journal during repair or upgrade, and delete it only as an explicit Device reset after confirming the Platform will not expect recovery.
- Operators should first confirm trusted HTTPS ingress forwarding, Device lifecycle state, system proxy reachability, and Credential Manager availability. Local “forget” is not revocation; an administrator must revoke the server record when access must be terminated.

The capability boundary complements command validation; it does not replace validation inside the Rust command implementation.
