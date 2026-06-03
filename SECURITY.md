# Security Policy

## Supported versions

Trigix is under active development. Security fixes are applied to the latest
release on the `master` branch.

| Version | Supported |
|---------|-----------|
| 1.x     | ✅        |

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, email **managecode@gmail.com** with:

- A description of the vulnerability and its impact.
- Steps to reproduce (proof-of-concept if available).
- Affected component(s) and version/commit.

You will receive an acknowledgement within **72 hours**. We aim to provide a
remediation timeline within **7 days** of triage and will keep you informed as a
fix is developed and released. We are happy to credit reporters in the release
notes unless you prefer to remain anonymous.

## Scope

Areas of particular interest:

- Authentication / JWT handling and tenant isolation (`services/platform-rs`).
- Credential and secret storage.
- Webhook signature verification.
- SQL/template injection in node execution (`services/executor`).
