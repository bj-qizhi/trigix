# Desktop troubleshooting and known limitations

Start with the safest recovery step. Do not disable signature verification, Gatekeeper, endpoint protection, Tenant policy, Approval, or audit controls to make an operation proceed.

## Collect safe diagnostics

Record the Desktop version, operating system and build, architecture, approximate UTC time, Device identifier, fixed error category, and workflow execution identifier. Remove access tokens, pairing codes, credentials, private URLs, prompt or transcript content, screenshots, and customer data before opening a public issue.

## Installation and trust

### Checksum does not match

Delete the installer and checksum, then download both again from the same immutable `desktop-v*` GitHub Release. Stop and report privately if the mismatch repeats. Never use an installer from a cache, mirror, chat attachment, or issue comment unless the organization has approved and verified that distribution path.

### Windows reports an unknown publisher or blocks launch

Confirm the Authenticode status and expected publisher from the release note. A public production installer must have a valid signature and timestamp. Organization policy may require a publisher allow rule. Do not use a self-issued development certificate as a production trust root.

### macOS blocks the application

Confirm the Developer ID signature, notarization ticket, Gatekeeper assessment, and checksum. Move the application into `/Applications` before granting Accessibility. Do not use the Privacy settings, quarantine removal, or a Gatekeeper override to make an unverifiable public build run.

## Pairing and connection

### Platform origin rejected

Enter a complete HTTPS origin such as `https://trigix.example.com`. Remove paths, query strings, fragments, user information, and trailing application routes. Confirm that the certificate is valid for the hostname.

### Pairing code expired or cannot be claimed

Create a new code and ask the Tenant administrator to approve that exact code. Confirm both users are working in the same Tenant. If a Device is already paired, forget or centrally revoke the old pairing before starting a replacement.

### Device remains disconnected

Check system time, DNS, TLS inspection policy, the Platform origin, proxy policy, and the Device lifecycle state. A revoked Device cannot reconnect and must be paired again. Do not copy the credential vault entry from another machine.

## Automation

| Result | Meaning | Safe next action |
| --- | --- | --- |
| `access_denied` | Target is protected, elevated, or outside the current user boundary | Run both applications at an approved equal integrity level or stop |
| `target_not_found` | Window or semantic control is absent | Open the expected application and issue a new command |
| `target_stale` | The interface changed after inspection | Inspect again and review the new target |
| `target_ambiguous` | More than one element matches | Use a more specific semantic selector |
| `protected_control` | The target is a password or protected field | Do not automate the field |
| `focus_changed` | Focus moved before the side effect | Restore the correct window and issue a new command |
| `unsupported_pattern` | The application does not expose the required accessibility pattern | Use a supported application adapter or manual operation |
| `partial_entry` | Text verification did not match | Inspect the target and enter data manually if appropriate |
| `host_crashed` | The isolated Automation Host stopped | Stop active work, restart Desktop, and reconcile the previous result |
| `cancelled` | A user or policy cancelled the command | Review audit history before issuing new work |

Coordinate replay is not a supported fallback. Desktop automation works only in an active, unlocked session owned by the Desktop user. A disconnected RDP session, fast user switch, screen lock, Session 0 service, container, or cross-user target cannot receive side effects.

### macOS Accessibility remains required

Open System Settings from Desktop, confirm the verified application in `/Applications` is enabled, then quit and reopen Desktop. After an application replacement or operating-system update, macOS may require permission to be granted again. Remove stale duplicate entries instead of enabling all copies.

## Voice and avatar

### Microphone unavailable or denied

Stop the voice session. Confirm system microphone access for Trigix Desktop, select a working input, and start again. If a browser or meeting application has exclusive control, release it first. Voice failure does not grant permission to retry automatically after the window is hidden.

### Realtime voice does not connect

Confirm the Device is paired, the deployment configured an approved realtime provider, WebRTC traffic is allowed, and the short-lived session has not expired. Desktop makes only bounded reconnect attempts. Select Stop before a fresh attempt.

### Avatar stops or shows a fallback

The fallback is safe and content-free. Select reduced or no motion, disable and re-enable the avatar, or restart Desktop. Avatar failure does not affect Approval or automation authority.

## Update and rollback

When an update is rejected, record the fixed compliance reason and keep the current installed version. Confirm channel, pin, maintenance window, release target, protocol range, manifest sequence and expiry, approved origin, artifact digest, and operating-system signature. Do not edit the manifest or substitute an installer.

Emergency downgrade requires a newly signed monotonic manifest, an exact `rollback_from_version`, compatible persisted-state evidence, and explicit Tenant policy. Reinstalling an old package outside that process is unsupported.

## Recovery and removal

After a crash or interruption, restart Desktop under the same user and allow the prior result to reconcile. If the Host remains unavailable, stop issuing commands and report the execution identifier. Do not delete or edit the recovery journal while an operation is unresolved.

If the computer is lost or compromised, the Tenant administrator must revoke the Device. Local uninstall alone is insufficient. If local removal fails, preserve the error, stop the process, and use the operating system's normal application removal mechanism.

## Known limitations

- Windows support is x64 Windows 11 Pro or Enterprise 24H2 and 25H2 only.
- Windows ARM64, Windows 10, Windows Sandbox, Windows containers, Session 0, and cross-user automation are unsupported.
- macOS support is limited to the two major versions named in the release note, on Apple Silicon and Intel.
- Desktop does not bypass higher-integrity, password, secure desktop, or protected-field boundaries.
- Desktop does not perform browser DOM automation or visual coordinate matching as a runtime fallback.
- Application-specific accessibility implementations can require a dedicated adapter.
- Voice depends on a deployment-approved realtime provider and allowed WebRTC connectivity.
- Voice can propose a published Workflow execution but cannot approve tools or Desktop actions.
- The built-in avatar is a presentation surface, not an autonomous operator.
- Official Trigix Desktop GA installers are unavailable until the official production signing and release qualification gates complete. Independent distributors may qualify and support their own clearly identified builds.

## Get help

For an Official Trigix artifact or a source-level defect reproducible on official supported code, use [GitHub Discussions](https://github.com/bj-qizhi/trigix/discussions) or [GitHub Issues](https://github.com/bj-qizhi/trigix/issues/new/choose). For Community Builds and Self-managed Distributions, contact the identified distributor first for installation, signing, update, privacy, incident, and support questions. Follow the [support policy](../support/support-policy.md). Report suspected vulnerabilities through the appropriate private security channel, never in a public issue.
