# ADR 0016: Deterministic Release Quality Evidence

## Status

Accepted

## Context

Passing functional tests does not prove that a release candidate was built from reviewed dependency inputs or that its product, chart, image, and release-note versions agree. Rust and Node already use lock files, while Python dependency resolution, frontend lint, coverage evidence, and cross-package version checks were not release gates. Helm application images also defaulted to the mutable `latest` tag.

Ordinary CI must improve release confidence without receiving a production signing identity, private security reports, customer data, or promotion authority. Evidence produced there must therefore remain bounded, deterministic, and safe to retain in public CI.

## Decision

Python application and SDK dependencies are resolved into reviewed, hash-locked requirement files. The AI Runtime has separate production and test locks so test tooling does not enter its container. CI and the container install their exact inputs with hash verification. Rust, Node, and Python vulnerability audits run in a separate security tier so dependency risk is visible independently from functional failures.

Audit exceptions are repository-visible and advisory-specific. `RUSTSEC-2023-0071` is temporarily accepted because it has no fixed release and the transitive RSA code is used for SSH signatures and MySQL password exchange rather than an application-exposed RSA private-key decryption oracle. Any fixed upstream version removes the basis for that exception; critical or high findings are never accepted by this decision.

The Web quality tier enforces strict TypeScript compilation, ESLint with zero warnings, unit coverage thresholds, and browser end-to-end tests. Coverage thresholds initially protect the pure modules already under unit test; expanding the covered module set requires tests and cannot silently reduce the established thresholds.

`scripts/release/verify_release_inputs.py` verifies that the Rust workspace, Web package, Web lock, and Helm application versions agree; Python lock files carry hashes; Helm application images do not default to `latest`; templates fall back to `Chart.appVersion`; and release notes do not embed a stale product version. It emits only the product version and SHA-256 digests of public lock files.

Helm keeps explicit image-tag overrides for private deployments, but an omitted application tag resolves to the immutable chart application version. A production release may use a separately reviewed digest. Database and infrastructure images remain independently versioned operational dependencies and are not represented as Trigix application artifacts.

## Consequences

- Pull requests fail on lint, covered-module regression, lock drift, version drift, mutable default application tags, or high-severity known dependency findings.
- CI evidence can be retained and digest-bound by the protected release-readiness process without exposing credentials or private report content.
- Dependency upgrades intentionally change lock digests and require review.
- The quality tier does not sign, publish, install, approve, or promote a release and cannot satisfy the external release gate by itself.
