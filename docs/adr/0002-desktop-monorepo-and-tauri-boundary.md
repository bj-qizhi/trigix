# Desktop Monorepo and Tauri Boundary

- Status: Accepted
- Date: 2026-08-12

## Context

Trigix Desktop must share domain contracts, security policy, CI, and release compatibility with the existing Rust Platform while providing a Windows-first desktop experience. Splitting the product into an independent repository now would duplicate types and make cross-version changes harder to review.

## Decision

Trigix Desktop remains in the existing monorepo. Tauri is the planned desktop application shell, with React presentation code and a Rust host. Protocol, policy, Approval, audit, and platform-specific automation live outside the presentation layer in independent Rust crates.

The initial `apps/desktop/src-tauri` package implements only the tested host boundary. Tauri framework and Windows API dependencies are introduced with their vertical slices so the core remains platform-neutral and Linux CI remains useful.

## Consequences

- Platform and desktop changes can be reviewed and tested atomically.
- Security-sensitive execution logic cannot be hidden in UI callbacks.
- Windows-specific code requires dedicated adapters and Windows CI.
- Commercial assets, signing material, and customer deployment data remain outside the public monorepo.
