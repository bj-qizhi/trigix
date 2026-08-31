# ADR 0010: Voice intent becomes a confirmed typed Tool proposal

## Status

Accepted

## Context

A final voice transcript is untrusted Tenant content. Treating inferred intent as an executable command would let recognition errors, prompt injection, or replay bypass Tool schemas, human confirmation, Workflow policy, Approval nodes, and auditability. The voice ingress also has no authority to choose a Device or construct a Desktop action.

## Decision

Voice intent enters a separate, versioned Tool-proposal boundary. The initial allow-list contains only `execute_workflow`. Its closed arguments are a published Workflow identifier and a bounded object input. Unknown Tool names, extra authority fields, non-object input, unbounded input, unknown Workflows, and Workflows without a published version fail before a proposal is created.

A proposal is bound to the authenticated Tenant and actor plus the accepted conversation identifier, session sequence, privacy-policy version, replay key, and a deadline of at most five minutes. An identical request is idempotent; conflicting reuse of the replay key is rejected. Proposal creation records no Execution identifier and cannot call the Execution or Desktop command services.

An authenticated Tenant administrator must explicitly confirm the proposal. Confirmation takes a single dispatch claim, then calls the same Workflow Execution service, quota checks, credential resolution, input-schema validation, Approval nodes, and audit path used by the existing typed Tool endpoint. A failed dispatch releases the claim for a safe retry. Rejection and expiry are terminal. The audit detail contains identifiers, the fixed Tool name, status, deadline, and resulting Execution identifier only; it contains no transcript, Tool input, credential, provider payload, or Desktop action.

The first proposal store is process-local and fail-closed across restart: pending proposals disappear and must be recreated from an unexpired conversation record. Durable proposal recovery can be added later without changing the contract. Confirmed Workflow Executions and audit events already use the configured production stores.

## Consequences

- Voice recognition alone never starts a Workflow or creates a Desktop command.
- Only registered typed Tools can cross the proposal boundary.
- Mutating work has a visible confirmation gap and continues through existing policy and Approval enforcement.
- A restart cannot silently resume or execute a pending voice proposal.
- Direct Desktop automation remains outside this contract and must continue through its command-specific Approval boundary.
