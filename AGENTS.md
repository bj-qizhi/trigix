# AI Workflow Project Agent Guide

This repository is the planning and implementation workspace for an enterprise AI Agent workflow low-code platform.

## Agent skills

### Issue tracker

Issues are tracked locally as markdown files under `.scratch/` until a remote repository issue tracker is configured. See `docs/agents/issue-tracker.md`.

### Triage labels

The project uses the default Matt Pocock engineering skill triage vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

This repository uses a single-context domain documentation layout with `CONTEXT.md` at the root and ADRs in `docs/adr/`. See `docs/agents/domain.md`.

## Engineering posture

- Keep architecture decisions explicit and recorded when they are hard to reverse.
- Prefer vertical slices over horizontal implementation plans.
- Treat workflow execution, Agent safety, tenant isolation, credential handling, and auditability as first-class concerns.
- Use the domain language in `CONTEXT.md` when writing plans, issues, PRDs, and code comments.
