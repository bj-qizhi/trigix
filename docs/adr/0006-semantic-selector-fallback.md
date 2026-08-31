# ADR 0006: Keep selector fallback semantic and visual matching authoring-only

- Status: Accepted
- Date: 2026-08-31

## Context

Stable accessibility identifiers can change between application releases. Treating every missing identifier as terminal makes authored Workflows unnecessarily brittle, but falling back to a coordinate or an arbitrary match can operate on the wrong control. A visual matcher can help an operator rediscover a target, yet its pixels, confidence score, and screen position are not a durable execution identity.

## Decision

The Desktop Host resolves an ordered semantic strategy chain immediately before each action. A window tries its automation identifier before executable and exact title, executable, or exact title alternatives that are actually present in the typed selector. An element tries automation identifier plus control type before accessible name plus control type. A strategy advances only after zero matches. Multiple matches fail as `target_ambiguous`, and an inspection snapshot mismatch fails as `target_stale`; neither condition may advance to another strategy.

Every successful selector-targeted action returns only the selected strategy, a fallback depth from zero through four, and whether fallback occurred. Durable adapter evidence stores the same bounded fields. Telemetry never contains a title, accessible name, typed value, control tree, screenshot, or screen position.

Visual matching is a read-only authoring suggestion carried by the bounded inspection request. It must have at least 90% confidence, exactly one candidate, an age no greater than 30 seconds, and the same inspection snapshot as its semantic selector. The Device re-inspects the current desktop and must resolve that selector to exactly one semantic target before the Web editor can save it. Unknown fields are rejected, and the Web editor separately rejects coordinate and bounds keys. Runtime action schemas contain no visual or coordinate target.

## Consequences

Application updates can recover through deterministic semantic information already captured during inspection, and operators can use visual discovery without turning pixels into an execution primitive. Ambiguous, stale, low-confidence, expired, or coordinate-bearing suggestions require another inspection. Applications without a unique semantic target need an application-specific adapter rather than a weaker generic fallback.
