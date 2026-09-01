# Enterprise Desktop Release Operations

## Purpose and authority

This runbook governs Desktop release distribution, fleet compliance, private mirrors, air-gapped import, rollout, halt, rollback, and incident response. It does not grant an application process or CI job permission to sign or install software.

The public repository qualifies unsigned development artifacts only. Production release jobs run in a protected environment with human approval, hardware- or service-protected signing identity, restricted network egress, immutable logs, and short-lived workload credentials. Signing keys, private mirror URLs, customer identifiers, Device Credentials, installer tokens, licensed media, and production configuration must never enter source, ordinary CI, issue text, or public build artifacts.

## Tenant policy and fleet API

Tenant administrators read and mutate policy at `GET/PATCH /v1/desktop/update-policy`. A mutation must send the last observed `revision`; HTTP 409 requires the operator to reload and review the newer policy before retrying. Automatic mode requires a UTC maintenance window, and an exact pin must equal the required version. The safe initial response is revision zero with updates disabled.

Tenant administrators read compliance at `GET /v1/desktop/fleet-compliance`. Optional `state` and fixed `compliance` filters may be combined with `limit` and `offset`; each page is capped at 100. The response includes the policy revision and required version used for classification. `stale` requires Device health recovery before update action, `invalid_inventory` requires registry repair, and lifecycle suspension or revocation must be resolved through Device governance. An explicitly rejected fleet-size bound is an operational signal to partition the query or deploy a reviewed database-side compliance index, never a reason to bypass Tenant isolation.

These endpoints never accept or return signing keys, signatures, private mirror origins, artifact locations, Device Credentials, command payloads, or installer authority. Policy Audit Log events contain only actor, revision, mode, and channel.

## Roles

| Role | Allowed | Prohibited |
| --- | --- | --- |
| Release builder | Produce reproducible unsigned shell/Host bundle, checksums, SBOM, provenance | Access production signing identity or publish stable metadata |
| Release approver | Review evidence and authorize protected signing/promotion | Modify the built payload after review |
| Release signer | Sign the reviewed installer and manifest in the protected environment | Select rollout policy or bypass evidence |
| Tenant administrator | Select channel, mode, pin, maintenance window, mirror, rollout halt, offline permission | Sign artifacts, change protocol compatibility, expose Credentials |
| Device runtime | Verify policy, manifest, artifact, Authenticode, then invoke the protected installer boundary | Trust UI input, arbitrary origins, unsigned metadata, or a stale decision |
| Incident commander | Halt promotion, revoke trust, coordinate containment and recovery | Reuse normal release approval as incident evidence |

No single ordinary role can build, approve, sign, promote, and install a stable release.

## Delivery modes and network policy

| Mode | Required outbound path | Artifact source | Failure behavior |
| --- | --- | --- | --- |
| Managed online | TLS through the configured system or explicit CONNECT proxy | Allow-listed release origin | Keep installed version; report a fixed compliance reason |
| Private mirror | TLS through enterprise proxy or private routing | Exact administrator allow-listed mirror origin | Reject redirects and any origin change |
| Air-gapped | None from the Device | Administratively imported offline bundle | Reject unless offline import policy is enabled and all evidence verifies |
| Fully disabled | None | None | Report updates disabled; never download or install |

Firewall rules should allow only the Platform origin, the selected release/mirror origin, certificate status and timestamp services required by enterprise policy, and the approved realtime provider when voice is enabled. The Platform service port remains private behind trusted TLS ingress. Proxies must not replace origin policy, add Credentials to logs, or downgrade TLS. Redirect following is disabled at the release boundary.

For a private mirror, copy the signed installer, signed manifest, SBOM, provenance, checksum, and signature evidence byte-for-byte. Do not regenerate the manifest at the mirror. Configure only the clean HTTPS origin; paths, user information, query strings, wildcard origins, and embedded Credentials are invalid policy.

For air-gapped import, transfer the complete release evidence set on controlled media, scan it on entry, record media custody, and assign a bounded opaque bundle identifier. The importer verifies the same manifest signature, expiry, sequence, channel, pin, protocol range, artifact digest, SBOM, provenance, and Authenticode chain used online. A filesystem path never becomes trusted manifest data. Expired metadata requires a newly approved manifest; changing the local clock or editing expiry is prohibited.

## Required release evidence

Before protected signing:

1. All blocking CI jobs pass for the exact source revision.
2. The Windows client qualification matrix passes on supported client editions.
3. Shell and isolated Host are built as one versioned unit.
4. Unit, integration, upgrade, interrupted-upgrade, rollback, recovery-journal, proxy, offline, and policy tests pass.
5. Dependency review contains no unaccepted critical or high finding.
6. The threat model and penetration-test disposition are current for changed privileged boundaries.
7. CycloneDX or SPDX SBOM and SLSA provenance are generated and digest-bound to the manifest.
8. Installer digest, Authenticode publisher, timestamp, malware scan, and reputation evidence are recorded.
9. Protocol minimum/maximum, release channel, rollout, expiry, security deadline, and rollback instructions are reviewed.

After signing, verify on a clean supported Windows client. Store the immutable release record, approvals, signatures, timestamps, digests, scanner versions, test run URLs, known risks, rollback version, and accountable owners according to the compliance retention policy. Do not store Device Credentials, private keys, access tokens, customer data, command payloads, voice content, or evidence ciphertext in the release record.

## Staged rollout and halt

1. Promote to `internal`; validate fresh install, upgrade, service restart, Device reconnect, recovery journal, voice cleanup, avatar fallback, and automation Host isolation.
2. Promote the identical digest to `closed_beta`; begin with a deterministic small percentage and representative proxy/private-mirror cohorts.
3. Observe installation success, crash-free startup, reconnect, stale fleet, version drift, rollback, and security telemetry. Telemetry uses fixed categories and bounded numbers only.
4. Increase rollout percentage without changing release identifier, sequence, artifact, signature, or evidence.
5. Promote the identical reviewed artifact to `stable` through a separately signed stable manifest.

Halt immediately on signature, digest, publisher, provenance, protocol, installer, recovery, data-loss, isolation, Credential, Approval, or unexpected network behavior. Halting freezes promotion and automatic authorization; it does not delete local recovery data or manufacture a downgrade.

## Rollback

Prefer a forward fix. Emergency downgrade requires all of the following:

- incident commander approval and Tenant administrator policy enabling emergency rollback;
- a new monotonic manifest sequence signed by the trusted release identity;
- `rollback_from_version` exactly matching the installed version;
- compatible protocol and persisted-state evidence;
- an allow-listed artifact whose digest, SBOM, provenance, Authenticode chain, and malware scan verify;
- a tested path that preserves the Device Credential and bounded recovery journal;
- a post-rollback fleet and security review.

Never rename an old installer as a new release, lower the accepted sequence, bypass expiry, disable signature checking, copy recovery state between Devices, or use local UI input as rollback authority.

## Threat model and verification focus

| Threat | Required control and test |
| --- | --- |
| Manifest or installer tampering | Canonical manifest signature plus artifact/evidence digests; byte-tamper tests |
| Compromised mirror or redirect | Exact HTTPS origin allow-list; redirects disabled; origin-change tests |
| Replay or freeze | Monotonic sequence, bounded expiry, version compliance; replay and clock-boundary tests |
| Unauthorized downgrade | Exact signed rollback source plus independent central permission |
| Protocol incompatibility | Signed minimum/maximum revision checked before download and again before install |
| Rollout targeting abuse | Deterministic opaque Device bucket; no identity or Credential in manifest |
| Signing-key theft | Protected non-exportable identity, dual control, audit, rotation and revocation procedure |
| Installer privilege escalation | Minimal installer surface, Authenticode verification, clean-client penetration test |
| Recovery corruption | Atomic bounded journal, restart/upgrade/rollback tests, no action-input retention |
| Offline-media substitution | Custody log, entry scan, identical signature/digest/evidence verification |

Penetration testing must cover manifest parsing, canonicalization, signature confusion, URL/user-info/redirect handling, proxy interception, offline import, archive/path traversal, installer custom actions, DLL search order, service permissions, update race and rollback, journal migration, local privilege boundaries, WebView IPC, and uninstallation. Findings have an owner, severity, remediation release, evidence, and explicit risk acceptance when not fixed.

Dependency review covers Rust, Node, Python, container bases, Tauri, Windows installer tooling, GitHub Actions, and transitive build tools. Pin mutable release inputs, retain provenance, and treat action/runtime deprecation warnings as planned maintenance even when they do not fail CI.

## Incident runbook

1. Declare severity and incident commander; preserve immutable evidence and clocks.
2. Halt channel promotion and automatic authorization. Disable affected mirror or offline bundle identifiers.
3. If trust is compromised, revoke the manifest/signing identity and publish the revocation through the protected channel.
4. Identify affected release IDs, sequences, digests, channels, protocol ranges, tenants, and fleet compliance states without collecting Credentials or command content.
5. Suspend affected Devices only when continued connection is unsafe; revocation is irreversible and requires re-pairing.
6. Choose forward fix or explicit signed rollback and test it on a clean representative client.
7. Communicate fixed facts, scope, mitigations, and operator actions. Never include secrets, private URLs, or customer payloads.
8. Verify fleet convergence, stale/offline exceptions, recovery, and Audit Log coverage.
9. Rotate affected credentials and signing trust, close temporary network access, and complete root-cause and control follow-up.

If verification is ambiguous, keep the current installed version, disable update authorization, and escalate. Availability does not override authenticity, Tenant isolation, Credential protection, or Desktop action safety.
