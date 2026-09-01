# Public support policy

This policy defines public intake, severity, response targets, triage, escalation, and security handoff for Trigix. It is not a paid support agreement or uptime service-level agreement.

## Channels

| Need | Channel | Public content rule |
| --- | --- | --- |
| Reproducible defect | [GitHub Issues](https://github.com/bj-qizhi/trigix/issues/new/choose) | Remove credentials, customer data, transcripts, private URLs, and screenshots |
| Usage question or feedback | [GitHub Discussions](https://github.com/bj-qizhi/trigix/discussions) | Use sanitized examples |
| Vulnerability or release-integrity concern | [Private vulnerability report](https://github.com/bj-qizhi/trigix/security/advisories/new) | Never disclose publicly before coordination |
| Platform status | [GitHub Actions](https://github.com/bj-qizhi/trigix/actions) and [GitHub Status](https://www.githubstatus.com/) | Self-hosted deployment status remains the operator's responsibility |

Enterprise customers use the intake channel and contractual targets in their separate support agreement.

## Required report information

Include the affected version or commit, component, deployment mode, operating system, architecture, expected and observed result, minimal reproduction, approximate UTC time, and fixed error category. For Desktop, include the Device identifier and execution identifier only when policy permits. Sanitize all logs.

## Severity model and response targets

Targets are measured during normal project business days and are goals, not guarantees.

| Severity | Impact | Initial response target | Update target |
| --- | --- | --- | --- |
| SEV-1 Critical | Active compromise, broad Tenant isolation failure, release-signing compromise, unrecoverable data loss, or production outage with no safe workaround | 1 business hour for an established support channel, otherwise 1 business day | Every 4 hours while actively managed |
| SEV-2 High | Major supported function unavailable, high-risk security weakness without known active exploitation, or widespread Desktop install, update, pairing, automation, voice, or rollback failure | 1 business day | Each business day |
| SEV-3 Medium | Supported function degraded with a safe workaround, isolated compatibility issue, or non-critical accessibility barrier | 3 business days | Weekly or at material change |
| SEV-4 Low | Documentation, cosmetic, feature request, or minor unsupported-boundary question | 5 business days | At triage or milestone change |

Severity can change as scope and evidence become clearer. Security reports follow private coordination even when service impact is low.

## Triage and ownership

The project maintainer role owns public intake and assigns one of these accountable roles in the private operational record:

- Product owner for scope, priority, user impact, and public wording;
- Release owner for installer, update, checksum, promotion, halt, and rollback;
- Security owner for vulnerabilities, credentials, Tenant boundaries, signatures, and disclosure;
- Service owner for Platform, Executor, AI Runtime, Desktop shell, or Automation Host diagnosis;
- Incident commander for SEV-1 and cross-component SEV-2 events.

Role names are public and stable. The current named person and backup belong in the access-controlled on-call and release system so personnel changes do not make a public document inaccurate. A production release is blocked unless those named assignments are present and acknowledged.

## Intake and escalation flow

1. Intake confirms the report is safe to handle in its current channel.
2. Triage reproduces the behavior on a supported version and classifies component, severity, scope, and workaround.
3. A service owner is assigned. Security-sensitive details move to the private advisory immediately.
4. SEV-1 or cross-component SEV-2 activates an incident commander, freezes risky release promotion, and establishes a private coordination record.
5. Release owner assesses whether to halt rollout, revoke trust, issue a forward fix, or start the signed rollback process.
6. Product owner approves public impact and recovery wording. Security owner approves vulnerability wording.
7. Resolution records the fixed version, verification evidence, remaining limitation, and follow-up owner.

## Component handoffs

- Installer or publisher failure goes to the Release owner. A signature or checksum mismatch also goes to Security.
- Update or rollback failure goes to Release and the Desktop service owner. Keep the installed version when authenticity is ambiguous.
- Pairing, credential vault, Tenant isolation, or Device authentication failure goes to Security and the Platform service owner.
- Automation Host crash, selector, permission, focus, or recovery issue goes to the Desktop service owner.
- Voice authentication, unexpected media capture, transcript retention, or provider credential issue goes to Security and the voice service owner.
- Avatar rendering without authority impact goes to Desktop. Any unexpected credential, Approval, or action access goes to Security.

## Closure criteria

A supported defect closes only after the fix or documented limitation is linked, relevant tests pass, release applicability is stated, and the reporter can identify the resolution. An incident additionally requires containment, recovery verification, public communication decision, and follow-up actions.

Operational exercises, named assignments, pager details, customer contacts, private infrastructure, and access tokens are intentionally not stored in this public repository.
