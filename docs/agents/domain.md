# Domain Documentation

This repository uses a single-context domain documentation layout.

## Files

```text
CONTEXT.md
docs/adr/
```

## Consumer rules

Agents should read `CONTEXT.md` before writing PRDs, issues, architecture reviews, tests, or implementation plans.

Agents should read relevant ADRs under `docs/adr/` before proposing architecture changes or technology choices.

## Writing rules

`CONTEXT.md` is a domain glossary, not an implementation specification.

ADRs should be short and should only capture decisions that are:

- Hard to reverse
- Surprising without context
- The result of a real trade-off
