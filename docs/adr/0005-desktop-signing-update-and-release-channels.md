# Desktop Signing, Update, and Release Channels

- Status: Accepted
- Date: 2026-08-12

## Context

A desktop automation runtime is a privileged software supply-chain target. Installation and update artifacts must be attributable, verifiable, reversible, and deployable gradually.

## Decision

Trigix Desktop uses separate internal, closed-beta, and stable release channels. Windows installers and update manifests are signed in a protected release environment. Signing keys are never stored in Git or exposed to ordinary build jobs. Releases include checksums, an SBOM, provenance, compatibility metadata, and rollback instructions.

Stable rollout is phased and can be halted. The updater verifies signature, channel, version monotonicity, and protocol compatibility before installation. Enterprise administrators can pin an approved version within the supported security window.

## Consequences

- Release automation requires protected environments and human Approval.
- Unsigned development builds cannot enter the stable update channel.
- Rollback and update-failure tests are required before general availability.
- Licensed avatar, voice, and customer assets are packaged from controlled private sources rather than this public repository.
