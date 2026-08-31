# ADR 0009: Tenant voice privacy and conversation ingress boundary

## Status

Accepted

## Context

A final speech transcript is sensitive Tenant content arriving from an external provider path. It must not become a hidden audio archive, bypass authentication, cross a Tenant boundary, or carry Desktop execution authority. Retaining every transcript by default would also conflict with data-minimization and deletion requirements.

## Decision

The Platform accepts final transcripts through a dedicated authenticated endpoint. The request schema is closed and bounded: it contains Tenant, session, sequence, occurrence time, and final text only. Unknown fields are rejected, so raw audio, partial-provider payloads, credentials, Tool approvals, and Desktop actions cannot enter through this contract. A valid JWT always determines the effective Tenant; production mode additionally requires Editor write access.

The default Tenant policy retains bounded operational metadata for seven days and retains no transcript content. A Tenant administrator may opt into redacted transcript retention for at most 30 days. Email-like values, long numeric values, credential prefixes, and bearer credentials are replaced before a record is created. API responses and debug output never contain the original transcript.

Conversation identifiers are Tenant-scoped for reads and deletion. A cross-Tenant lookup and an unknown identifier return the same not-found result. Expiry removes both the record and its replay index, and duplicate session sequence values are rejected while a record is live.

This ingress creates conversation input only. It cannot construct a Tool call, grant Approval, dispatch a Desktop command, or invoke an operating-system primitive. Any later intent-to-Tool adapter must use the existing typed Tool, policy, Approval, lease, and audit path.

## Consequences

- Content retention is opt-in and bounded; metadata-only is the safe default.
- Tenant isolation and deletion semantics are deterministic and testable without a provider SDK.
- Raw audio remains outside Platform storage and telemetry.
- Durable PostgreSQL persistence and the intent-to-Tool adapter remain prerequisites before advertising production voice conversation capability.
