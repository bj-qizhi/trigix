# Public launch measurement and feedback

Trigix uses the minimum public measurement needed to evaluate release discovery and onboarding. It does not add an advertising identifier, tracking pixel, analytics cookie, fingerprint, or cross-site profile to the GitHub Pages website.

## Public acquisition signals

Repository maintainers may use aggregate counts that GitHub provides for repository views, clones, release-asset downloads, stars, forks, issues, and discussions. GitHub controls the collection boundary and retention for those signals. Trigix does not join them to a Platform user, Tenant, Device, Workflow, microphone session, or avatar preference.

The website download panel makes one unauthenticated request to the public GitHub Releases API. It filters immutable stable `desktop-v*` releases and renders only real installer and checksum assets. The request contains no Trigix identifier and the result is not sent back to a Trigix service.

## Product activation signals

Self-hosted operators can assess activation from their own Tenant-scoped operational state:

- Platform deployment health;
- published Workflow and first successful Execution;
- paired Device lifecycle and last-seen state;
- supported client version and update compliance category;
- content-free fixed voice latency and failure categories;
- audit-confirmed completion or cancellation of a low-risk action.

These signals remain inside the operator's deployment. Trigix does not provide a default maintainer collection endpoint. Voice telemetry excludes audio and transcript content. Avatar preferences remain local.

## Validation checklist

Before public publication:

- [ ] Inspect `website/index.html` and `website/site.js` for third-party analytics, pixels, forms, and tracking cookies. None are permitted.
- [ ] Confirm theme preference is the only website local-storage value.
- [ ] Confirm the Releases request contains no credential or Trigix identifier.
- [ ] Test success, no Desktop release, API error, and rate-limit states.
- [ ] Confirm the website does not claim an installer exists when no stable `desktop-v*` asset is present.
- [ ] Confirm product telemetry schemas reject transcript, audio, credentials, and unbounded fields.
- [ ] Confirm Tenant access and retention tests pass for voice and Device records.
- [ ] Review GitHub and configured provider privacy terms before launch.

## Feedback loop

Use [GitHub Discussions](https://github.com/bj-qizhi/trigix/discussions) for questions and product feedback, [GitHub Issues](https://github.com/bj-qizhi/trigix/issues/new/choose) for reproducible defects, and the private channel in [SECURITY.md](../../SECURITY.md) for vulnerabilities or artifact-integrity concerns.

Launch review groups feedback by supported journey: installation, verification, pairing, permissions, automation, voice, avatar, update, rollback, uninstall, accessibility, and documentation. Public triage links each actionable result to an Issue without copying private customer data.
