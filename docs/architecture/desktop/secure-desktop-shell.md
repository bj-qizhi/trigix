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

The shell advertises an empty capability set until the supervised command execution runtime is attached. Receiving a command or cancellation before that boundary exists fails closed and reconnects as degraded instead of acknowledging or silently discarding work. Reconnect delay grows exponentially to a 60-second base cap with 80–120% jitter. A successful connection resets the attempt count. Authentication rejection remains visibly degraded and retries no faster than once per minute; local unpair moves the shell offline.

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
- Operators should first confirm trusted HTTPS ingress forwarding, Device lifecycle state, system proxy reachability, and Credential Manager availability. Local “forget” is not revocation; an administrator must revoke the server record when access must be terminated.

The capability boundary complements command validation; it does not replace validation inside the Rust command implementation.
