# Internal Alpha Desktop qualification

## Candidate identity

Every successful default-branch CI run produces two development-signed Desktop qualification artifacts:

- `windows-development-signed-qualification-COMMIT`, containing the x64 NSIS package and Windows signing evidence;
- `macos-universal-development-signed-qualification-COMMIT`, containing the Universal DMG and macOS signing evidence.

The artifact name, evidence, product version, and SHA-256 digest bind the candidate to one source commit. Artifacts are retained for 90 days. Select one green default-branch run and record its full 40-character commit, workflow-run URL, artifact names, hashes, test start/end time, and accountable candidate owner before distributing it to internal evaluators.

These packages use ephemeral self-signed identities. Their evidence explicitly sets `production_release_eligible=false`. They are suitable only for controlled development and Internal Alpha qualification; they are not Official releases, must not be uploaded to a public GitHub Release, and must not be used to close the production signing gates.

## Installation boundary

Limit distribution to named internal evaluators and managed test devices. Transfer packages and recorded hashes through the approved internal channel. Verify the artifact hash before installation. Remove any previously trusted development certificate and previous Alpha installation before changing candidates.

On Windows, the self-signed publisher is intentionally not publicly trusted. Install only after independently matching the candidate hash and recorded `Trigix Development Qualification` identity. On macOS, Gatekeeper is expected to reject the self-signed package; use only the documented managed-test exception, never weaken Gatekeeper on a normal workstation.

Do not include Device Credentials, pairing claims, Tenant data, customer content, voice recordings, private URLs, production tokens, or signing material in the candidate record or feedback.

## End-to-end matrix

Run the same exact candidate through the matrix below. A failed required cell blocks promotion.

| Area | Windows | macOS | Required evidence |
| --- | --- | --- | --- |
| Install and launch | Clean install and relaunch | DMG install and relaunch on Apple Silicon and Intel | OS build, architecture, artifact hash, result |
| Pair and reconnect | Pair, restart, reconnect, revoke | Pair, restart, reconnect, revoke | Device and execution identifiers only |
| Automation | Inspect, focus, invoke, text, keys, pointer, launch | Same governed action surface | Approval and Audit Log correlation |
| Safety | Protected control, stale selector, focus loss, timeout, cancel | Permission denial/revocation plus shared controls | Fixed error category and no side effect |
| Voice | Consent, connect, proposal, Approval, stop, failure | Same | No retained audio or transcript in evidence |
| Avatar | State, viseme, emotion, interruption, fallback | Same | Bounded event/result record |
| Update controls | Policy, pin, halt, offline/mirror rejection | Same | Policy revision and fixed compliance state |
| Recovery | Host crash, app restart, journal replay rejection | Same | Recovery result without command content |
| Rollback | Reviewed development rollback rehearsal | Same | Exact source/target version and outcome |
| Uninstall | Remove app and development trust | Remove app and managed exception | Residual-state check |

Use the supported-version and architecture matrices in the Windows and macOS release runbooks. CI runner results supplement but do not replace interactive device evidence.

## Promotion decision

Promotion requires all required cells to pass, no open P0 or release-blocking P1 defect, a current dependency review, and recorded Product, Release, Security, Desktop, and QA dispositions. Record only the public-safe outcome in the repository Issue; retain device logs and private evidence in the authorized evidence system.

An Internal Alpha pass authorizes progression to production-signed candidate work. It does not authorize public distribution, Closed Beta, or GA. Those stages still require production signatures, independent security evidence, supported-client verification, and accountable approval.
