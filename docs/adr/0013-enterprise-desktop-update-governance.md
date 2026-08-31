# ADR 0013: Enterprise Desktop update governance

## Status

Accepted

## Context

Desktop automation packages have local operating-system authority and upgrade more slowly than the Platform. An updater that mixes policy, network download, signature decisions, and installer execution would create a privileged path that is difficult to test or centrally govern. Private mirrors and air-gapped environments also need the same authenticity, replay, compatibility, and evidence rules as the public release channel.

## Decision

`desktop-release` defines a closed, versioned release manifest and a deterministic policy evaluator. The manifest binds a monotonic release sequence, semantic version, channel, supported Desktop protocol revision range, publication and expiry, rollout percentage, optional security deadline, optional explicit rollback source version, bounded artifact descriptor, SBOM descriptor, provenance descriptor, and deployment signature. Unknown fields fail closed.

Online artifacts must use a clean HTTPS URL whose origin is in an administrator-supplied allow-list. Offline bundles use a bounded opaque identifier rather than a filesystem path and are accepted only when central policy enables offline import. Artifacts are capped at 1 GiB; SBOM and provenance evidence are capped at 32 MiB. Every descriptor has a SHA-256 digest. A deployment-owned verifier checks the canonical manifest payload; no signing key or default trust root is compiled into the public repository.

Central policy selects disabled, manual, or automatic mode; one release channel; an optional exact version pin; an optional UTC maintenance window; offline import permission; and emergency rollback permission. The evaluator rejects malformed, untrusted, unsigned, expired, replayed, channel-mismatched, pin-mismatched, or protocol-incompatible manifests. Downgrade is rejected unless the signed manifest names the exact installed version and central policy independently enables emergency rollback.

A valid decision is only `current`, `available_for_manual_install`, `install_authorized`, or a fixed rejection/deferral reason. It contains no command line, Credential, private URL, arbitrary error, or installer handle. The crate has no filesystem, process, HTTP, Tauri, Device identity, Workflow, Approval, or automation-host dependency. A separate protected release component must revalidate the manifest, artifact digest, Authenticode chain, and policy immediately before installation.

Rollout assignment hashes Device and release identifiers into a deterministic bucket; no Credential or customer data enters the manifest. Security deadlines may override an automatic-mode maintenance deferral, but cannot override disabled/manual mode, channel, pin, signature, expiry, replay, origin, protocol, or rollback policy.

Fleet compliance uses only bounded Device inventory metadata: identifier, Agent semantic version, lifecycle, and last-seen time. It distinguishes compliant, update-required, ahead-of-policy, stale, suspended, revoked, and invalid inventory. It cannot change lifecycle or dispatch a Desktop Command.

## Consequences

- Release eligibility is testable without granting installer authority to ordinary CI or the application UI.
- Public, private-mirror, and air-gapped delivery use the same signed metadata and digest boundary.
- Administrators can halt, pin, stage, schedule, and explicitly authorize emergency rollback.
- Production signing, timestamping, malware scanning, installation, and rollback evidence remain separate release gates.
- A policy decision alone is never evidence that installation occurred or succeeded.
