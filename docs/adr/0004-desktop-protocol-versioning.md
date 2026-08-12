# Desktop Protocol Versioning

- Status: Accepted
- Date: 2026-08-12

## Context

Desktop installations upgrade more slowly than cloud services. A Platform deployment must not silently break connected Devices or reinterpret an older action with new semantics.

## Decision

Platform/device messages use explicit versioned envelopes. The first contract is `desktop.v1` with typed JSON payloads. The Platform supports the current version and one previous compatible version during staged upgrades. Additive fields must have safe defaults. Breaking structure, validation, or action semantics requires a new protocol version and compatibility fixtures.

Commands include unique identifiers and expiring leases. Completed commands are replay-protected. Unknown versions and actions fail closed.

## Consequences

- Protocol types are a shared crate, not duplicated frontend/backend interfaces.
- Compatibility tests become a release gate.
- Removal of an old version requires telemetry proving the supported Device population has upgraded or been explicitly retired.
