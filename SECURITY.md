# Security policy

## Supported versions

Security fixes are prepared for the latest stable Platform release and, after Desktop GA, the latest stable `desktop-v*` release. A release may set an earlier security deadline or require an immediate upgrade. Unsupported versions may not receive a patch.

Community Builds and Self-managed Distributions retain their own vulnerability intake, signing, update, incident, and disclosure responsibility. Report an upstream source weakness here when it reproduces on official supported code. Report a distributor-only packaging, key, mirror, modification, or deployment weakness to that distributor unless it also compromises an Official Trigix boundary.

## Report a vulnerability privately

Use [GitHub private vulnerability reporting](https://github.com/bj-qizhi/trigix/security/advisories/new) for suspected vulnerabilities. Do not open a public issue, discussion, pull request, or test against systems or data you do not own or have explicit permission to assess.

Include, when safe:

- the affected version, commit, component, and deployment mode;
- a concise impact statement and prerequisite conditions;
- reproducible steps or a minimal proof of concept;
- relevant logs with credentials, tokens, customer data, transcripts, private origins, and screenshots removed;
- whether the issue is already being exploited or publicly discussed;
- a secure way to coordinate if GitHub advisory messages are insufficient.

Do not include live credentials, production customer data, biometric data, recorded voice, private signing material, or destructive payloads. Revoke exposed credentials before reporting them.

## What happens next

The security maintainer triages a report under the [support severity model](docs/support/support-policy.md). We aim to acknowledge credible reports within one business day and provide an initial severity assessment within three business days. These are response targets, not service-level guarantees.

We may ask for validation details, coordinate a fix and advisory, assign a CVE when appropriate, and agree on a disclosure date. Please allow a reasonable remediation period before publication. We will credit reporters who request credit and whose reports materially help remediation.

## Security scope

High-impact areas include Tenant isolation, authorization, pairing and Device credentials, workflow and Desktop Approval, secret handling, webhook verification, Agent tools, automation boundaries, update manifests, installer trust, signature verification, realtime voice authentication, transcript retention, and audit integrity.

Reports that describe only unsupported deployment, social engineering without a product weakness, missing optional headers with no demonstrated impact, denial of service requiring excessive traffic, automated scanner output without validation, or issues in an unsupported version may be closed as informational.

## Safe testing

Use your own local deployment and test data. Keep tests bounded, non-destructive, and reversible. Do not access another Tenant, degrade availability, persist after testing, move laterally, exfiltrate data, or test third-party services through Trigix without their permission.

## Release integrity concern

For a checksum, publisher, notarization, manifest, or release-asset mismatch, stop using the artifact and report privately with the immutable release URL and observed digest. Never bypass platform trust checks. See [Desktop download verification](docs/release/desktop-download-verification.md).
