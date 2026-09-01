# Trigix Desktop onboarding

This guide covers the complete first-use journey for Trigix Desktop on its supported Windows and macOS systems. Trigix Desktop must be paired with a Trigix Platform deployment before it can receive work.

## Supported systems

| Platform | Supported release boundary |
| --- | --- |
| Windows | Windows 11 Pro or Enterprise 24H2 and 25H2 on x64 hardware |
| macOS | The current generally available macOS major version and the preceding generally available major version, on Apple Silicon or Intel |

The exact macOS versions are recorded in each Desktop release note. Windows 10, Windows ARM64, locked sessions, Session 0, and automation across user sessions are not supported.

## Before installation

You need:

1. A standard user account on a supported computer.
2. The HTTPS origin of your organization's Trigix Platform.
3. A Tenant administrator who can approve a Device pairing code.
4. Permission to install software and, on macOS, grant Accessibility and microphone access when those features are needed.

Download only from the [Trigix Releases page](https://github.com/bj-qizhi/trigix/releases). Follow the [download verification guide](../release/desktop-download-verification.md) before opening an installer. Builds signed with a self-issued development identity are qualification artifacts, not public releases.

## Install on Windows

1. Download the x64 Windows installer and its `.sha256` file from the same `desktop-v*` release.
2. Verify the checksum and Authenticode publisher as described in the verification guide.
3. Open the installer as your normal signed-in user.
4. Keep the Desktop application and its Automation Host together. Do not copy the Host executable to another directory.
5. Start Trigix Desktop from the Start menu.

Enterprise application control may require an administrator to allow the verified publisher. Trigix does not request elevation to automate a higher-integrity application.

## Install on macOS

1. Download the Universal DMG and its `.sha256` file from the same `desktop-v*` release.
2. Verify the checksum, Developer ID signature, notarization ticket, Gatekeeper assessment, and Universal architecture slices.
3. Open the DMG and drag Trigix Desktop to Applications.
4. Eject the DMG, then open Trigix Desktop from Applications.
5. If macOS warns that the application cannot be verified, stop. Confirm that you downloaded a production release and repeat the verification steps. Do not bypass Gatekeeper for a public release.

The Universal application includes Apple Silicon and Intel code. Rosetta is not required on Apple Silicon.

## Pair the computer

1. Open Trigix Desktop.
2. In **Platform HTTPS origin**, enter only the secure origin, for example `https://trigix.example.com`. Paths, query strings, fragments, embedded credentials, and plain HTTP are rejected.
3. Enter a recognizable Device name that does not contain confidential information.
4. Select **Create pairing code**.
5. Give the short-lived code to your Tenant administrator through an approved channel.
6. The administrator confirms the code in Tenant administration.
7. Return to Desktop and select **I approved the code**.
8. Confirm that the connection, Device identifier, and Automation Host show a healthy state.

The Device credential is placed in the operating-system credential vault. It is never displayed in the Desktop interface. If the pairing code expires, start again. Do not post pairing codes in issues or support discussions.

## Permissions

### Windows

Trigix Desktop uses the current interactive user session. It does not require a general accessibility permission. The target application must run at the same integrity level as Desktop. Windows, endpoint protection, AppLocker, or WDAC may still block installation or execution according to organization policy.

### macOS Accessibility

Automation requires Accessibility permission:

1. In Desktop, select **Open macOS permission settings**.
2. In **System Settings > Privacy & Security > Accessibility**, enable Trigix Desktop.
3. Return to Desktop and wait for the permission status to update.
4. If the state does not update, quit and reopen Desktop once.

You can revoke this permission at any time. Revocation stops local automation and does not unpair the Device. Grant Accessibility only to the verified application in `/Applications`.

### Microphone

Voice access is requested only when you select **Start microphone**. The active state and Stop control remain visible. Selecting Stop, hiding the window, session expiry, input loss, or closing Desktop releases the local media tracks. Denial leaves voice unavailable but does not affect workflow or non-voice automation.

## Automation and approvals

Desktop actions use semantic window and control identifiers. Trigix does not silently fall back to coordinate replay. A command can run only when all required controls agree:

- the Device is paired and authenticated to the correct Tenant;
- the command version, lease, and policy are valid;
- the action is in the Device capability set;
- required Approval is present and unexpired;
- the current window and control still match the selector;
- the protected Automation Host is available;
- the action has not been cancelled.

Launch and other high-risk actions require explicit command-specific Approval. Voice and the avatar cannot supply Approval. Use **Stop automation** to cancel active work. Locking the computer, switching users, disconnecting the supported session, or losing focus prevents side effects until a new command is issued.

## Voice conversation

1. Pair the Device and confirm the connection is healthy.
2. Select **Start microphone** and grant operating-system access if prompted.
3. Choose an input device after access has been granted.
4. Speak only after Desktop reports that realtime voice is connected.
5. Review any proposed workflow action in the Platform. Voice input can create a review-only proposal, not execute it.
6. Select **Stop microphone** when finished.

Audio uses a direct WebRTC path to the deployment-approved realtime provider. The Platform receives bounded final text and content-free latency categories. The default Tenant policy retains conversation metadata for seven days and does not retain transcript text. A Tenant administrator can enable redacted transcript retention for a bounded period. See [Privacy and data boundaries](../legal/privacy.md).

## Avatar controls

The built-in avatar presents idle, listening, thinking, speaking, interruption, error, and stopped states. You can independently:

- show or hide the avatar;
- enable or mute voice playback;
- enable captions;
- select full, reduced, or no motion;
- enable high contrast;
- stop the avatar immediately.

These preferences are stored only in local browser storage. The avatar has no automation, credential, Approval, or tool authority. If rendering fails, Desktop uses a content-free built-in fallback.

## Accessibility

Desktop provides English and Simplified Chinese text, keyboard-operable controls, a skip link, visible focus, status announcements, alert regions, captions, high contrast, and reduced or disabled avatar motion. Use the operating system's display scaling and screen reader controls as needed. At 200 percent Windows scaling, semantic automation remains supported, but the Desktop window may need to be maximized.

If a focus transition, status announcement, label, or contrast prevents task completion, file an accessibility bug and include the operating system, assistive technology, version, and affected control. Do not include credentials, voice content, or confidential screen data.

## Updates and recovery

Tenant update policy may disable updates, pin a version, select a channel, require a maintenance window, or use an approved mirror. Desktop verifies the signed manifest, release target, protocol range, sequence, expiry, artifact digest, and platform signature before installation. It keeps the installed version when verification is ambiguous.

After an interrupted action or restart, Desktop uses a bounded recovery journal to prevent unconfirmed side effects from being replayed. Reconnect the same user session, confirm the Platform and Automation Host status, and issue new work only after the previous result is reconciled. Never edit recovery files or lower the local clock to bypass expiry.

For update failures, pairing recovery, permission resets, or an unavailable Automation Host, use [Desktop troubleshooting and known limitations](desktop-troubleshooting.md).

## Unpair and uninstall

Before transferring or retiring a computer:

1. Stop voice and active automation.
2. Select **Forget local pairing** and confirm. Ask the Tenant administrator to revoke the Device if the computer is unavailable or compromised.
3. On Windows, use **Settings > Apps > Installed apps > Trigix Desktop > Uninstall**.
4. On macOS, quit Desktop, move Trigix Desktop from Applications to Trash, and remove it from Accessibility and Microphone permissions in System Settings.
5. Follow organization policy for any retained logs or recovery records. Do not manually move credentials to another computer.

Uninstalling locally does not prove that a Device record was revoked centrally. Tenant administrators should verify the Device lifecycle state and audit record.

## First-use completion checklist

- [ ] Installer checksum and operating-system signature verified
- [ ] Desktop starts under a standard interactive user
- [ ] Device pairing approved by the correct Tenant administrator
- [ ] Connection and Automation Host healthy
- [ ] macOS Accessibility permission granted only if automation is needed
- [ ] A low-risk approved automation action completes and appears in the audit log
- [ ] Stop automation cancels a test action safely
- [ ] Voice starts and stops without retaining an active microphone track
- [ ] Avatar motion, captions, high contrast, and Stop controls behave as selected
- [ ] Update policy is visible to the Tenant administrator
- [ ] Support and private security-reporting channels are known
