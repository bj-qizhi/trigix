# ADR 0015: Desktop Release Readiness Evidence

## Status

Accepted

## Context

An unsigned qualification build or a valid update manifest does not prove that a production Desktop release is safe to sign or promote. Release approval also depends on supported Windows client smoke testing, Authenticode verification, malware scanning, dependency review, penetration-test disposition, and an accountable approval. Those records are produced in protected or external systems and can contain sensitive report content that does not belong in the public repository.

A checklist made only of mutable text is not sufficient: evidence can expire, refer to a different installer, omit a required control, or retain unresolved critical findings. Conversely, placing signing keys, private report URLs, scanner output, customer data, or certificate material in the application would broaden the trust boundary.

## Decision

`desktop-release` defines a closed versioned `ReleaseReadinessRecord` and a deterministic verifier. The record binds release identifier, semantic version, source revision, installer SHA-256, SBOM SHA-256, provenance SHA-256, attestation time, and a short expiry window. The verifier compares those release and evidence fields directly with the candidate release manifest, so replacing the complete readiness record cannot retarget it to a different manifest.

Six named controls are mandatory: supported-client smoke, Authenticode verification, malware scan, dependency review, penetration-test disposition, and release approval. Each control contains only the installer digest it assessed, an evidence digest, completion and validity times, and a fixed pass/fail outcome. The penetration-test disposition additionally records bounded unresolved finding counts. Critical and high findings always block readiness. Medium findings require a digest of explicit risk acceptance; the acceptance document itself remains in the controlled evidence system.

The verifier rejects unknown or missing fields, invalid identifiers and digests, future or over-age evidence, expired records or controls, failed outcomes, installer mismatches, and unresolved critical/high findings. It accepts an explicit current time, performs no I/O, and returns only fixed rejection categories. It has no network, filesystem, process, installer, certificate-store, signing, publishing, or secret access.

Protected release automation is responsible for obtaining the original evidence from its authoritative systems, verifying each evidence digest, constructing the record, calling the verifier, and retaining the immutable record with its approvals. A successful result is permission to proceed to the separately protected signing stage; it is not proof that signing, publication, installation, or rollout occurred.

## Consequences

- Ordinary CI can test the release gate without possessing production authority.
- Expired, failed, incomplete, mismatched, or critically unresolved evidence cannot be represented as release-ready.
- Public records remain content-free and cannot leak external report bodies, private locations, signing identities, or customer data.
- Actual production signing, Windows client evidence, malware assessment, independent penetration testing, and accountable approval remain external release obligations and cannot be inferred from repository tests.
