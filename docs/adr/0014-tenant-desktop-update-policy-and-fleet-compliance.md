# ADR 0014: Tenant Desktop Update Policy and Fleet Compliance

## Status

Accepted

## Context

Enterprise administrators need a durable Tenant policy for Desktop release channels and a bounded view of Device version compliance. The existing release crate deliberately has no network, persistence, installer, or signing authority, while the Device registry is authoritative for lifecycle, Agent version, and last-seen time.

Allowing clients to submit release origins, signatures, installation commands, or arbitrary remediation data through this API would collapse release, Platform, and Device trust boundaries. Unconditional updates would also make concurrent administrator changes unsafe.

## Decision

The Platform owns one revisioned update policy per Tenant. A missing record is represented by a safe, non-persisted default: updates disabled, stable channel, no pin, no offline import, and no emergency rollback. Every mutation is administrator-only, derives Tenant and actor identity from the verified token, supplies the observed revision, and succeeds only when that revision still matches.

The persisted policy contains only mode, release channel, required and optional pinned semantic version, optional UTC maintenance window, offline-import permission, emergency-rollback permission, revision, actor, and timestamp. Automatic mode requires a maintenance window. A pin must equal the required version. Unknown fields and invalid values fail closed.

The fleet compliance endpoint joins the Tenant policy to the existing authoritative Device registry at read time. It maps lifecycle, Agent version, and server-observed last-seen time through the pure `desktop-release` classifier. Results use fixed compliance and remediation categories, bounded filtering and pagination, and a hard inventory scan ceiling. They exclude Device credentials, capabilities, health detail, signatures, artifact locations, private origins, and command content.

PostgreSQL uses a Tenant row-level-security policy in addition to explicit Tenant predicates. Policy changes append an immutable, content-free Audit Log event containing actor, revision, mode, and channel only.

## Consequences

- Platform administrators can safely coordinate staged Desktop adoption without gaining signing or installation authority.
- Concurrent writes cannot silently overwrite a newer policy.
- Compliance is reproducible from authoritative inventory and the policy revision returned with the response.
- Fleets above the bounded scan ceiling fail explicitly until a database-side materialized compliance index is introduced.
- Release artifact publication, signature verification, download, installation, and rollback execution remain separate release and Device responsibilities governed by ADR 0013.
