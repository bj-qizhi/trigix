# Desktop solution validation and demo reset

## Purpose

This runbook makes the Windows and macOS Desktop solution journey repeatable without turning a demonstration into release evidence. It covers environment preparation, reset, the operator script, expected degradation, evidence, and approval. Use the exact same candidate digest throughout one run.

The environment is disposable and contains synthetic data only. Never use a production Tenant, customer content, production Device Credential, signing key, private penetration-test report, or real personal voice content. An Internal Alpha artifact remains non-production even when every step below passes.

## Frozen inputs

Before setup, record these values in the controlled validation record:

| Input | Required value |
| --- | --- |
| Source | Full 40-character default-branch commit |
| Platform candidate | Container digests or source commit and build run |
| Desktop candidate | Windows and macOS artifact names and SHA-256 digests |
| Product version | Exact version shown by Desktop and Platform |
| Supported devices | Exact OS build, architecture, and device identifier for each test computer |
| Configuration | Digest of the reviewed, non-secret staging configuration |
| Owners | Named environment, solution-validation, Desktop, Security, and Release roles |
| Window | UTC start, expiry, and planned reset time |

Use the commit-bound artifacts and evidence described in [Internal Alpha Desktop qualification](../release/internal-alpha-qualification.md). Verify every downloaded hash before installation.

## Reproducible environment

### Local deterministic baseline

The local baseline validates the Platform, workflow canvas, execution path, audit behavior, and synthetic seed independently of Desktop device trust:

```bash
git switch --detach FULL_COMMIT
docker compose --project-name trigix-demo up -d
make dev-verify
```

Start Platform, Executor, and Web using the commands in [Dev Bootstrap](../dev/bootstrap.md). Keep default development credentials on loopback only. The seeded Tenant, Workspace, Project, Workflow, and Workflow Version identifiers in that guide are stable after every fresh volume initialization.

Do not point Desktop at this plain-HTTP development endpoint. Device pairing requires an HTTPS Platform origin.

### Device-connected staging baseline

For the end-to-end Desktop journey, deploy the same reviewed commit to an isolated staging origin with:

- a valid HTTPS hostname trusted by both test devices;
- one primary synthetic Tenant and one isolation-control Tenant;
- a dedicated staging administrator and bounded evaluator accounts;
- an approved non-production realtime voice provider configuration when voice success is in scope;
- no production mirror, signing identity, customer integration, or customer data;
- Audit Log access for the validation owner;
- update promotion disabled until the halt and rollback steps explicitly enable it.

Use the normal deployment mechanism and keep secrets in its protected secret store. Record only the configuration digest and fixed provider category in the validation record.

## Reset procedure

### Local baseline reset

The following command deletes only the dedicated `trigix-demo` Compose volumes. Confirm the project name and current directory before running it:

```bash
docker compose --project-name trigix-demo down --volumes --remove-orphans
docker compose --project-name trigix-demo up -d
make dev-verify
```

Never apply the volume-removal command to another Compose project or a host containing retained data.

### Device-connected staging reset

Perform these actions in order:

1. Stop microphone capture and active automation on every test device.
2. Halt candidate promotion and automatic update authorization.
3. Revoke all synthetic Devices created by the prior run; confirm the Audit Log records each lifecycle change.
4. Forget local pairing and uninstall the prior Desktop candidate from each test device.
5. Remove only the prior development identity or managed test exception. Do not weaken system-wide publisher or Gatekeeper policy.
6. Delete synthetic workflows, approvals, and retained voice metadata according to staging policy, or replace the disposable Tenant.
7. Restore the reviewed staging snapshot or redeploy the frozen commit and configuration digest.
8. Create fresh primary and isolation-control Tenants and fresh evaluator accounts.
9. Confirm there are no connected Devices, pending Approvals, active voice sessions, or authorized updates before the next run.

The reset owner records completion and exceptions. A partial reset invalidates the next solution-validation result.

## Operator script

### 1. Establish scope and trust

1. Display the source commit, workflow-run URL, product version, artifact names, and hashes.
2. Verify the Windows checksum and self-issued qualification identity; verify the macOS checksum, qualification identity, and Universal slices.
3. State that both packages are development qualification artifacts and not Official releases.
4. Show the supported OS/session boundary and the private security-reporting channel.

Expected result: the audience can distinguish source, candidate, distributor, supported boundary, and production-release status.

### 2. Build and execute a workflow

1. Sign in to the primary Tenant and create a workflow with Trigger, a deterministic transform, and a Desktop action proposal.
2. Save a draft, publish the version, run it with synthetic input, and show the execution timeline.
3. Edit the workflow to introduce a validation error, confirm publication fails closed, then restore the valid graph.
4. Open the Audit Log and correlate workflow publication and execution identifiers.

Expected result: the canvas, validation, versioning, execution, and audit journey is visible without external SaaS credentials.

### 3. Pair Windows and macOS

For each supported test computer:

1. Install the frozen candidate and launch it as a standard interactive user.
2. Pair it to the primary Tenant through an administrator-approved short-lived code.
3. Confirm connection, Device identity, protocol compatibility, and Automation Host health.
4. Restart Desktop and verify authenticated reconnect without exposing the Device Credential.

Expected result: both Devices are healthy in the primary Tenant and absent from the isolation-control Tenant.

### 4. Demonstrate governed automation

On Windows use a synthetic document in a supported native test application. On macOS use an equivalent synthetic document after granting Accessibility only to the verified candidate.

1. Inspect visible windows and controls; confirm protected or credential-like values are redacted.
2. Focus one unambiguous target, enter Unicode text, invoke a semantic control, and correlate the result with the Audit Log.
3. Attempt a protected-field write, stale selector, ambiguous selector, focus-loss action, expired lease, and cancelled command.
4. Confirm every negative case produces a fixed rejection category and no side effect.
5. Revoke macOS Accessibility permission and confirm automation stops; restore it only for the remaining test.

Expected result: semantic actions succeed only inside the approved lease, policy, selector, focus, permission, and Tenant boundaries.

### 5. Demonstrate voice and avatar safety

1. Deny microphone permission and confirm workflow and non-voice automation remain available.
2. Grant permission, start a staging voice session, and show the persistent microphone state, captions, and Stop control.
3. Propose a Desktop action by voice. Confirm it remains review-only until a separate human Approval is granted.
4. Stop the microphone, hide the window, and simulate provider loss in separate runs; confirm media tracks and transport close.
5. Exercise avatar listening, thinking, speaking, interruption, reduced motion, no motion, high contrast, and built-in fallback.

Expected result: voice and avatar degradation never grants Approval, Tool, Credential, or Desktop authority and never blocks the non-voice product path.

### 6. Demonstrate Tenant, halt, and recovery controls

1. Sign in to the isolation-control Tenant and attempt to read the primary Tenant's Workflow, Device, Execution, Approval, and Audit records.
2. Confirm every request is denied or returns only the isolation Tenant's data.
3. Enable a reviewed candidate policy for a bounded cohort, then activate rollout halt before installation authorization.
4. Confirm halted and non-selected Devices keep the installed version and report a fixed compliance state.
5. Crash or stop the Automation Host during a synthetic action, restart Desktop, and confirm unsafe non-idempotent replay is rejected.
6. Exercise the reviewed development rollback rehearsal with exact source and target versions; do not manually install an older package as a substitute.

Expected result: Tenant isolation, rollout halt, recovery journal, and rollback authority are visible and fail closed.

### 7. Close the run

1. Stop voice and automation, revoke both Devices, forget local pairing, and uninstall the candidate.
2. Confirm the Audit Log contains the bounded lifecycle and decision records without Credentials, command content, audio, or transcript text.
3. Run the reset procedure and record remaining defects, limitations, and evidence locations.

## Evidence and disposition

Record one pass, fail, blocked, or not-applicable result for every numbered section and for both operating systems. The record includes only fixed identifiers, versions, hashes, timestamps, result categories, and links to authorized evidence. Screenshots and logs containing customer data, Credentials, pairing codes, transcripts, private origins, or report contents are prohibited.

A solution-validation pass requires:

- all required Windows and macOS steps completed against the same candidate digests;
- Tenant isolation, protected-field, stale target, focus loss, lease, cancellation, permission revocation, halt, recovery, and rollback negative cases to fail closed;
- no open P0 or release-blocking P1 defect;
- the environment reset completed;
- named solution-validation, Desktop, Security, and Release owners to approve the disposition.

Approval authorizes use of the reviewed script for the next controlled stage only. It does not satisfy production signing, independent penetration testing, Closed Beta, or GA approval.
