# Desktop Device Trust and Action Policy

- Status: Accepted
- Date: 2026-08-12

## Context

Desktop automation can operate applications and user data. Treating model output or a network message as direct authority would make prompt injection, replay, tenant confusion, and compromised Credentials capable of controlling a computer.

## Decision

Every Device is explicitly paired to one Tenant using a device-held key. Desktop Commands are typed, tenant- and project-scoped, actor-attributed, leased, validated, checked for replay, evaluated by local policy, and audited. Medium- and high-risk actions require a command-specific, expiring Approval by default. Critical actions are denied until an explicit product policy and implementation are reviewed.

Agents and voice input can request only approved Tools. They never call operating-system primitives directly.

## Consequences

- Automation has more visible gates than unrestricted remote-control software.
- Approval and audit behavior can be tested without Windows APIs.
- Credential rotation, Device revocation, cancellation, and persisted replay protection are mandatory before production use.
- Tenant policy may become stricter but cannot weaken product-wide prohibitions.
