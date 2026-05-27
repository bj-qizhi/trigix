# Issue Tracker

Issues are tracked locally as markdown files under `.scratch/` until this project is connected to GitHub, GitLab, Jira, Linear, or another tracker.

## Local issue convention

Use this structure:

```text
.scratch/
  issues/
    0001-short-title.md
    0002-short-title.md
```

Each issue should use this shape:

```markdown
# Short Title

## Status

needs-triage | needs-info | ready-for-agent | ready-for-human | wontfix

## Category

bug | enhancement

## What to build

Concise end-to-end behavior.

## Acceptance criteria

- [ ] Criterion 1
- [ ] Criterion 2

## Notes

Context, constraints, links, and decisions.
```

When this project gets a remote issue tracker, replace this file with the tracker-specific workflow.
