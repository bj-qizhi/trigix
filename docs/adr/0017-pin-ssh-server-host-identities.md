# ADR 0017: Pin SSH server host identities

- Status: Accepted
- Date: 2026-08-31
- Decision owners: Security and Automation

## Context

SSH and SFTP workflow nodes authenticate with passwords or private keys and can execute commands or transfer files on operator-selected systems. The executor previously accepted every server host key. An attacker able to intercept or redirect a connection could therefore impersonate the configured server and receive authentication attempts or workflow data.

The executor cannot safely infer the intended server identity from the network. Trust on first use would persist an identity learned through the same unauthenticated connection and is unsuitable for unattended enterprise workflows.

## Decision

Every SSH and SFTP node must provide `host_key_fingerprint` in canonical OpenSSH `SHA256:<base64>` form. Configuration validation rejects a missing, malformed, non-SHA-256, or out-of-range connection input before opening a socket. During SSH key exchange, the executor calculates the SHA-256 fingerprint of the presented public host key, or the certified public key carried by a host certificate, and continues only on an exact match. Server identity verification occurs before password or client private-key authentication.

Operators obtain the expected fingerprint through a separately authenticated channel, such as a managed server inventory or an administrator-confirmed console. Key rotation is an explicit workflow configuration change subject to the normal review and audit path.

## Consequences

- Redirected and impersonated hosts fail closed before credentials or workflow actions are sent.
- Existing SSH and SFTP nodes require a fingerprint before their next successful execution.
- Host-key rotation requires coordinated configuration updates.
- Automatic enrollment and trust on first use remain outside the executor trust boundary.
- A future managed known-host registry may provide centrally governed fingerprints, but it must preserve tenant isolation, auditability, and explicit rotation approval.
