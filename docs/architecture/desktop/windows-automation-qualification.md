# Windows Automation Qualification

## Release boundary

This document is the qualification contract for the M2 Windows automation slice. “Supported” means the Device runs as a standard user in an active interactive desktop and automates native Win32 controls through semantic selectors. It does not imply privilege escalation, background automation of a locked desktop, remote control of another user session, coordinate replay, browser DOM automation, or support for every third-party accessibility provider.

The blocking continuous-integration lanes run on the pinned x64 `windows-2022` and `windows-2025` GitHub-hosted images. Each lane builds the native fixture and isolated Host, applies and verifies an ephemeral Authenticode qualification signature, discovers the real fixture window, validates password redaction, focuses it, enters text through the value path, invokes the button, and exercises the Host process tests. The runner matrix is pinned instead of using `windows-latest`, so image migrations cannot silently change the qualified operating system.

Fixture signing uses a short-lived RSA code-signing identity created in the current runner user certificate store with a fixed qualification-only subject and a non-exportable private key. It is never installed into the operating-system Root or Trusted Publisher stores: qualification builds an isolated in-memory custom trust chain against the exact ephemeral signer. Before native automation starts, the lane requires Authenticode integrity without a hash mismatch, the exact qualification signer, the successful custom chain, rejection of a deliberately tampered copy, and a fixture digest matching the generated evidence record. An untrusted-root `UnknownError` from WinVerifyTrust may proceed only through those identity, custom-chain, and negative-integrity checks. Each lane retains that bounded public record for 14 days, then an unconditional cleanup step removes the private-key container, thumbprint state, and local evidence file. The workflow contains no certificate password, production publisher identity, timestamp-service credential, or exported key.

This qualification signature proves that automation tests exercise a signed deterministic fixture and catches unsigned-binary regressions. It is not a production publisher signature, has no public timestamp or reputation, and cannot satisfy the signed Windows 11 release gate.

## Supported matrix

| Dimension | Supported boundary | Qualification |
| --- | --- | --- |
| Client edition | Windows 11 Pro or Enterprise 24H2 and 25H2, x64 | Same Win32 API contract as the blocking Server lanes; release-candidate smoke run on a physical or virtual client image is required before signing |
| CI edition | Windows Server 2022 and Windows Server 2025, x64 | Blocking native fixture lanes on every pull request and `master` push |
| Architecture | x64 process on x64 Windows | Built and executed in both Windows lanes |
| ARM64 | Not supported in M2 | Device installation fails compatibility validation; ARM64 qualification is a later release decision |
| Display scaling | 100%, 125%, 150%, 200% | Semantic window/control identifiers are supported; coordinate actions are outside M2 and must not be substituted |
| Locale | Any Windows display locale when the application exposes stable automation identifiers | Fixture and product selectors use identifiers; localized accessible-name fallback must be supplied and tested by an application-specific adapter |
| Console session | Active, unlocked standard-user desktop | Supported and exercised by native CI |
| RDP | Active, connected RDP session owned by the Device user | Supported; disconnect, lock, or session replacement terminates eligibility for side effects |
| Multi-monitor | Native semantic controls on any attached monitor | Supported because no screen coordinates enter the selector contract; topology changes require fresh resolution |

Windows 10 consumer editions, Windows 11 ARM64, Windows Sandbox, Windows containers, Session 0 services, and cross-user desktops are outside the supported M2 boundary. This is a declared compatibility limit rather than a degraded hidden path.

## Session, privilege, and policy classification

| Environment | Classification | Required behavior and operator action |
| --- | --- | --- |
| Standard user targeting same-integrity application | Supported | Run the Device and target in the same signed-in user session |
| Standard user targeting elevated application | Protected boundary | Return `access_denied`; never request elevation or inject input. Run an approved elevated Device deployment only after a separate security review |
| Locked desktop or disconnected RDP | No side effects allowed | Resolution or focus returns `target_not_found`, `focus_changed`, or `access_denied`; unlock or reconnect and issue a new leased command |
| Fast user switching or another user session | Unsupported | The Device owns only its current interactive session; reconnect the correct user session |
| Antivirus or EDR | Supported when signed binaries are allowlisted | The runtime uses documented window APIs and no code injection. Quarantine or launch denial surfaces as `launch_failed` or `host_crashed`; verify signature and enterprise allowlist |
| AppLocker or WDAC | Supported when publisher/path policy permits the signed Device and Host | Policy denial is terminal; administrators must deploy the publisher rule before pairing |
| HTTP proxy | Device connection supports explicit TLS HTTP CONNECT proxy | Automation remains local; proxy configuration cannot grant desktop authority |
| Accessibility provider | Native Win32 edit/button/window patterns are supported | `unsupported_pattern` identifies controls that require a dedicated UI Automation provider adapter |

The adapter never falls back from a semantic selector to coordinates. A stale inspection returns `target_stale`; multiple matches return `target_ambiguous`; password controls return `protected_control`; focus changes return `focus_changed`; partial text verification returns `partial_entry`. These codes are safe diagnostics and must not include typed text, unrestricted titles, control trees, or credentials.

## Enforced budgets

The native qualification test performs 50 value-entry and invoke cycles after a real fixture inspection and focus. It fails the release lane when any of these budgets is exceeded:

- fixture discovery: 10 seconds;
- long-session test wall time: 60 seconds;
- p95 native action latency: 2 seconds;
- process handle growth: 16 handles;
- test-process working-set growth: 64 MiB;
- protected password writes: zero successful attempts;
- unreaped fixture processes: zero after test teardown.

The process-boundary suite separately enforces queued and active cancellation, deadline termination, crash classification, replay safety, serialized execution, and child reaping. The Windows lanes run both the native qualification and this suite.

## Milestone exit report

M2 Windows automation is accepted when both pinned Windows lanes and the general CI suite are green on the merge commit. The declared support boundary has no unresolved release-blocking defect: native inspection, focus, value entry, invoke, protected-control rejection, isolation, cancellation, timeout, recovery, evidence, and resource budgets have blocking coverage. Environments marked protected or unsupported above are explicit product limits and cannot be represented as successful automation.

Before a signed public Desktop release, release engineering must attach the green workflow run, the signed Windows 11 client smoke result, installer signature verification, antivirus scan result, and any application-specific adapter results to the release record. A failure in that release-only evidence blocks signing even if M2 CI remains green.
