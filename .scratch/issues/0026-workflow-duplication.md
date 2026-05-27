# Workflow Duplication

## Status

done

## Category

enhancement

## What to build

A "Duplicate" action per workflow row that creates a copy (`{name} (copy)`) with the same latest-version graph as a new draft, then opens the editor on the new workflow.

## Acceptance criteria

- [x] `WORKFLOW_DUPLICATED = "workflow.duplicated"` audit constant in `audit.rs`.
- [x] `POST /v1/workflows/:id/duplicate` route in `build_router`.
- [x] `DuplicateWorkflowBody { tenant_id }` struct.
- [x] `duplicate_workflow` handler: fetches original → creates new workflow with name `{name} (copy)` + same workspace_id/project_id → creates new draft version with original graph (if any) → emits WORKFLOW_DUPLICATED audit event with `duplicated_from` detail → returns 201 + new WorkflowRecord.
- [x] Integration test `duplicate_workflow_creates_copy`: verifies 201, distinct id, correct name, matching workspace/project.
- [x] `duplicateWorkflow(tenantId, workflowId): Promise<WorkflowRecord>` in `apps/web/src/api/client.ts`.
- [x] `duplicating: string | null` state in `WorkflowList.tsx`.
- [x] `handleDuplicate` handler — calls API, prepends new workflow to list, navigates editor to copy.
- [x] "⧉ Duplicate" button per row (disabled while in-flight, stopPropagation), next to "Open".
- [x] 106 Rust tests (45 executor + 57 platform + 4 workflow-core), 0 TypeScript errors.

## UX flow

Workflow list → click "⧉ Duplicate" on any row → new "{name} (copy)" draft created → editor opens on copy
