# Secure Desktop Shell

## Status

Accepted implementation boundary for the Windows-first Tauri application shell.

## Authority boundary

The Tauri WebView is presentation code. It cannot launch applications, inspect targets, execute automation, read credentials, approve commands, or submit a raw `DesktopCommand`. Those operations remain behind the authenticated Device connection, `desktop.v1` validation, execution lease, local policy, command-specific Approval, replay journal, isolated automation Host, and Audit Log.

The initial shell exposes exactly two local IPC operations:

- `shell_status` returns a revision, enumerated connection state, enumerated automation state, and whether stop is currently meaningful.
- `request_automation_stop` accepts a bounded opaque request identifier and the exact state revision observed by the UI.

A stop request is a cancellation signal only. It cannot start, retry, approve, retarget, or modify an action. The runtime consumes the signal and remains responsible for propagating cancellation to queued or active supervisor work.

## Tauri controls

The main local window is the only member of the `main-shell` capability. The capability allowlists only the two application commands above and grants no filesystem, shell, opener, HTTP, clipboard, process, or automation plugin permission. Remote origins are not configured.

Static UI assets are bundled locally. The Content Security Policy permits self-hosted scripts and styles plus Tauri's local IPC transport; it blocks external origins, objects, frames, base rewrites, and form submission. JavaScript prototype freezing is enabled. IPC structs deny unknown fields, mutation identifiers have a fixed character and size grammar, and stale state revisions or repeated identifiers fail closed.

The Rust state controller is platform-neutral and tested in Linux workspace CI. Native Tauri and WebView dependencies are Windows-target-only so server CI does not require a graphical desktop stack. Windows CI must compile the shell target as part of the supported desktop qualification lane.

## Failure behavior

- Poisoned or unavailable state returns a typed error and disables the UI stop control.
- An idle runtime rejects stop rather than manufacturing a successful side effect.
- A stale view must refresh before sending another mutation.
- A repeated request identifier is rejected within a bounded replay window.
- The UI displays fixed error copy and does not render backend error detail.

The capability boundary complements command validation; it does not replace validation inside the Rust command implementation.
