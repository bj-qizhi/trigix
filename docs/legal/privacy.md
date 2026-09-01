# Privacy and data-processing boundaries

Effective date: 1 September 2026

This notice explains data boundaries in the public Trigix website, open-source Platform, and Trigix Desktop. Trigix is primarily self-hosted. The organization operating a deployment controls its users, Tenants, integrations, infrastructure, retention, and configured service providers.

## Public website

The GitHub Pages website contains no advertising SDK, analytics script, tracking pixel, cookie banner, marketing form, or cross-site identifier. A local theme preference may be stored in browser local storage. The download panel requests the public GitHub Releases API to show real release assets and does not transmit a Trigix user or Device identifier.

GitHub hosts the website, repository, Releases, Issues, Discussions, and private vulnerability reporting. GitHub may process network and account data under its own terms and privacy notice. Repository maintainers can see the aggregate and account information that GitHub makes available to them.

## Self-hosted Platform

A Platform operator determines the deployment's purposes and means of processing. Depending on enabled features, the deployment can process:

- account, Tenant, Project, role, and authentication data;
- Workflow definitions, inputs, outputs, schedules, and execution state;
- integration credentials and secrets encrypted or protected by the deployment configuration;
- audit events, bounded operational metrics, traces, and error categories;
- Device identity, pairing lifecycle, capability, version, last-seen, and compliance state;
- voice conversation metadata and, only under an enabled Tenant policy, redacted final transcript text;
- automation evidence only when Tenant policy enables it.

Operators must configure lawful purposes, user notice, access controls, processors, locations, retention, export, and deletion appropriate to their deployment. They should not place credentials or unnecessary personal data in Workflow names, Device names, prompts, logs, or support reports.

## Desktop and device data

Desktop stores the Platform origin, Device identifier, and Device credential in the operating-system credential vault. It stores locale and avatar presentation preferences locally. A bounded recovery journal prevents ambiguous side effects from being replayed after restart. Desktop does not expose the Device credential to the WebView.

Local automation receives an already-authorized typed action. Semantic selector results and fixed outcome categories can enter audit records. Protected control values, unrestricted control trees, typed credential content, and raw screenshots are not general telemetry.

Unpairing removes the local pairing credential. A Tenant administrator must separately revoke the Device record when a computer is retired, lost, or compromised.

## Voice and avatar

Microphone capture begins only after the local user selects Start and grants operating-system permission. Audio uses a direct WebRTC path to the deployment-approved realtime provider. The provider credential remains server-side and Desktop receives a short-lived setup secret. Stop, window hiding, input loss, session expiry, and teardown release local media resources.

The Platform accepts bounded final transcript text through a Tenant-scoped privacy boundary. The default policy retains metadata for seven days and does not retain transcript text. When a Tenant administrator enables transcript retention, Trigix redacts supported sensitive patterns before storage and permits a bounded retention period of no more than 30 days. Operators remain responsible for verifying that redaction and retention meet their requirements.

Voice latency telemetry contains fixed event and failure categories plus bounded durations. It excludes audio and transcript content. The built-in avatar processes transient local audio levels for presentation and does not retain sample buffers.

## Screenshots and evidence

Screenshot evidence is disabled unless the Tenant policy permits it and encryption is available. When enabled, evidence is command-bound, size and format limited, encrypted, access controlled, and retention bounded. Operators should choose the minimum evidence necessary and apply stricter policy where screens can contain sensitive data.

## External integrations and AI providers

A Workflow or administrator can configure external model, speech, storage, SaaS, or protocol providers. Data sent to those providers follows the Workflow configuration and each provider's terms, location, security, and retention. Trigix does not make an external provider private merely by routing it through a self-hosted deployment.

## Security and retention

Tenant context, RBAC, credential boundaries, Approval, encrypted secrets, audit logging, and fixed retention controls reduce risk but do not replace secure deployment. Operators should use TLS, supported releases, least privilege, protected backups, secret rotation, database isolation, network allowlists, and tested deletion procedures.

## Requests and questions

Users of an organization-managed Trigix deployment should first contact that deployment's administrator for access, correction, export, deletion, restriction, or retention questions. For this public repository, use [GitHub Discussions](https://github.com/bj-qizhi/trigix/discussions) for general privacy questions and the private channel in [SECURITY.md](../../SECURITY.md) for a privacy issue that creates a security risk. Do not submit personal data or credentials in a public issue.

Material changes to this notice are published in repository history with a new effective date.
