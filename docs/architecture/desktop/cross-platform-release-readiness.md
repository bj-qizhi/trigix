# Cross-platform desktop release readiness

## Status

Accepted contract for Windows and macOS production-candidate evidence. This
contract describes evidence metadata only; private reports, credentials,
customer identities, signing material, and protected-system locations remain
outside the repository and ordinary CI.

## Problem

Desktop 1.0 has two independently distributed artifacts: a Windows x86-64
installer and a macOS Universal DMG. A release decision must not reuse evidence
from one artifact, operating-system version, or processor architecture for
another. Development self-signing proves the packaging mechanics but is not a
production trust decision.

The first manifest and readiness schemas did not bind the artifact to a desktop
target, and the readiness fields named Windows client smoke and Authenticode
directly. They could not safely distinguish a Windows installer from a macOS
DMG or represent Developer ID verification, notarization, and the macOS Apple
Silicon and Intel qualification matrix. Schema version 2 makes the target
explicit in both records and closes that gap.

## Candidate binding

Each `ReleaseManifest` and `ReleaseReadinessRecord` names the same target. The
update client rejects a manifest for a different target before considering
rollout or installation. The readiness record then binds all controls to one
exact candidate:

- release identifier and semantic version;
- reviewed source revision;
- installer or DMG SHA-256;
- SBOM and provenance SHA-256 values;
- attestation and expiry timestamps; and
- `windows_x86_64` or `mac_os_universal` as the release target.

Every control carries the same subject artifact digest. Reusing a report for a
different installer or DMG fails verification. A target disagreement, unknown
field, or previous schema version fails closed.

## Required client matrix

The operating-system identifiers are release-freeze inputs rather than
permanent product constants. Exactly two distinct supported versions are
required.

| Target | Required environments | Additional trust control |
| --- | --- | --- |
| Windows x86-64 | x86-64 on both supported Windows versions | production code-signature and timestamp verification |
| macOS Universal | Apple Silicon and Intel on both supported macOS versions | Developer ID verification plus notarization, stapling, and Gatekeeper evidence |

Each environment must pass clean installation, governed automation, upgrade,
rollback, and uninstall against the exact candidate digest. Duplicate or
incomplete matrix cells fail verification. A single aggregate smoke result
cannot stand in for a missing architecture or operating-system version.

## Shared release gates

Both targets additionally require current malware assessment, dependency
review, independent penetration-test disposition, and accountable release
approval. Critical or high findings block release. Medium findings require a
separate risk-acceptance evidence digest; the acceptance itself remains in its
protected authoritative system.

The record has a short bounded lifetime. Every referenced control must remain
valid through the record expiry, preventing an old qualification from being
attached to a later candidate.

## Development qualification boundary

Ordinary CI may create ephemeral self-issued identities and retain public-safe
verification metadata. Such evidence must state
`production_release_eligible=false`; it cannot populate a production readiness
record as passed. Production records are constructed only in the protected
release environment after the external controls have completed.

This boundary supports the release gates tracked by issues #82 and #103 while
allowing deterministic repository tests to verify the record shape before
production credentials are available.
