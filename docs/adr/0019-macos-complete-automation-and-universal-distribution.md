# ADR 0019: macOS Complete Automation and Universal Distribution

- Status: Accepted
- Date: 2026-09-01
- Decision owners: Desktop, Security, and Release Engineering

## Context

The first public desktop release now includes Windows and macOS. The macOS product must provide the same governed local automation action surface as Windows, run natively on Apple Silicon and Intel, and ship as a DMG from the project download channel. At release freeze, support covers the current generally available macOS major version and the preceding generally available major version.

macOS automation requires Accessibility consent and synthetic input privileges. Developer ID signing and Apple notarization require protected external credentials, and successful CI compilation is not evidence that Accessibility automation works in an interactive user session.

## Decision

The existing Tauri shell, isolated Automation Host protocol, action policy, Approval, lease, audit, selector, and redaction boundaries remain shared. macOS adds a target-specific adapter inside `desktop-automation`:

- AXUIElement provides bounded semantic window and control inspection, focus, invoke, and verified value entry.
- CoreGraphics provides guarded key and pointer events only after the selected application is confirmed frontmost.
- Password, secure-text, credential-labelled, stale, ambiguous, or non-frontmost targets fail closed.
- Application launch uses an exact application identity allowlist mapped to validated absolute paths. No shell command interpolation or AppleScript is permitted.
- Accessibility trust is checked without silently prompting from the Automation Host. The desktop onboarding experience owns the user-visible permission journey.

Tauri uses platform-specific bundle configuration. Windows remains NSIS; macOS uses a DMG with minimum deployment target macOS 15.0. macOS release artifacts are Universal binaries containing `arm64` and `x86_64` slices for both the shell and isolated Automation Host.

The protected macOS release workflow imports a Developer ID certificate into an ephemeral keychain, builds the Universal application, signs and notarizes it through Tauri, verifies strict code signatures, Gatekeeper assessment, notarization staples, both architecture slices, and SHA-256, then publishes the DMG and checksum to an existing immutable release tag.

CI compiles and tests the native target on both architectures across macOS 15 and macOS 26 runner families. These currently represent the preceding and current generally available major versions. The exact labels must be reviewed at every release freeze rather than treated as permanent product policy.

## Consequences

- macOS is a first-class production target without duplicating the domain protocol or weakening execution safeguards.
- Accessibility consent is explicit and revocable; denied consent produces a closed failure rather than a degraded unsafe path.
- Unsigned qualification DMGs can prove deterministic assembly but cannot satisfy the production release gate.
- General availability remains blocked until a signed/notarized DMG passes interactive qualification on Apple Silicon and Intel devices on both supported macOS major versions, external penetration testing is accepted, and release evidence is retained.
- Apple credentials remain only in the protected `desktop-production` GitHub environment and never enter repository files or ordinary CI jobs.
