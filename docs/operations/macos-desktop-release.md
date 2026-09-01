# macOS Desktop Release Qualification

## Supported release envelope

Trigix Desktop supports Apple Silicon and Intel on the current generally available macOS major version and the preceding generally available major version at release freeze. For the September 2026 implementation baseline, CI uses macOS 26 and macOS 15. Release Engineering must confirm Apple's GA status and GitHub runner labels before every production freeze.

The public artifact is one Universal DMG. Both the application binary and `desktop-automation-host` must contain `arm64` and `x86_64` slices. The configured minimum deployment target is macOS 15.0.

## Required protected configuration

Create a GitHub environment named `desktop-production`, require production reviewer approval, restrict it to `desktop-v*` tags, and configure these secrets:

- `APPLE_CERTIFICATE`: base64-encoded Developer ID Application PKCS#12 certificate;
- `APPLE_CERTIFICATE_PASSWORD`: PKCS#12 password;
- `APPLE_SIGNING_IDENTITY`: exact Developer ID Application identity;
- `APPLE_ID`: notarization account;
- `APPLE_APP_SPECIFIC_PASSWORD`: app-specific password;
- `APPLE_TEAM_ID`: Apple Developer team identifier;
- `MACOS_KEYCHAIN_PASSWORD`: unique high-entropy password used only for the ephemeral runner keychain.

Do not use repository variables for these values. Rotate any value exposed in logs, artifacts, support bundles, or local shell history.

## Build and publish

1. Ensure the release commit passed protected CI and the external penetration-test disposition is accepted.
2. Create a signed annotated tag matching `desktop-vMAJOR.MINOR.PATCH` on that exact commit and push it.
3. Approve the `Release Desktop macOS` deployment in the protected environment.
4. The workflow builds both Rust targets, combines the sidecar, builds the Universal Tauri target, signs, notarizes, and validates the DMG.
5. Confirm the release contains exactly the DMG and its `.sha256` file and that the recorded digest matches the downloaded file.

Manual dispatch is only for rebuilding an existing immutable release tag. It must not point at an unreviewed branch or synthesize a tag.

## Interactive qualification matrix

The release owner records one result for every architecture and supported OS combination:

| OS | Apple Silicon | Intel |
| --- | --- | --- |
| Current GA major | Required | Required |
| Previous GA major | Required | Required |

Each device run must cover installation from the downloaded DMG, first launch, Accessibility denied state, guided permission grant, permission revocation, relaunch, update policy, and uninstall. Automation qualification covers inspection, window focus, semantic invoke, protected-field rejection, verified Unicode text entry, named keys and modifiers, left/right/middle pointer actions, exact allowlisted launch, stale/ambiguous selectors, focus loss, lease expiry, cancellation, Host crash recovery, voice proposal and Approval, avatar state transitions, and audit correlation.

Record device model, architecture, exact OS build, artifact SHA-256, signing identity, notarization result, test operator, start/end time, failures, retest references, and final disposition. CI runner output is supplemental evidence and does not replace interactive qualification.

## Release gates

Production publication is blocked when any of these conditions is true:

- Developer ID, notarization, Gatekeeper, staple, or Universal-slice verification fails;
- any supported architecture/OS matrix cell lacks interactive evidence;
- Accessibility permission handling can bypass Approval, lease, tenant policy, or audit controls;
- a protected control can be read or written;
- the external penetration test has an unresolved release-blocking finding;
- the closed-beta owner has not accepted the signed build on the agreed devices;
- checksum, SBOM, dependency, or incident-response evidence is missing.

If signing or notarization credentials are not yet available, unsigned CI qualification may continue, but the result must remain explicitly non-production and the macOS GA gate stays blocked.
