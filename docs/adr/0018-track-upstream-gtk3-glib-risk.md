# ADR 0018: Track Upstream GTK3 GLib Risk

- Status: Accepted
- Date: 2026-09-01
- Decision owners: Security and Desktop

## Context

The workspace lock file contains `glib` 0.18.5 through the Tauri 2.11.5 Linux desktop stack: Wry, WebKitGTK, GTK3, and the gtk-rs bindings. `RUSTSEC-2024-0429` and `GHSA-wrw7-89jp-8q8g` report undefined behavior in `glib::VariantStrIter`; the published fix begins at `glib` 0.20.0.

Trigix does not import `glib` or use `VariantStrIter` directly. The dependency does not appear in the Windows target graph and therefore does not enter the Windows-first desktop artifact. It remains relevant to any future Linux desktop distribution and must not be hidden merely because the affected API is not currently called by workspace code.

The latest published GTK3 crate is still 0.18.2 and Tauri 2.11.5 resolves the vulnerable `glib` line. The gtk-rs GTK3 repository has merged a migration to gtk-rs-core 0.22, but no compatible crates.io release and Tauri adoption are available yet. Cargo cannot safely substitute `glib` 0.20 or 0.22 under GTK3 crates compiled against the 0.18 API.

## Decision

The Dependabot alert remains open and is tracked by issue #94 as a P1 upstream-blocked security item. The repository will not dismiss it as unused, suppress it globally, force a semver-incompatible transitive version, or carry an unreviewed private GTK fork.

Windows release qualification may continue because the dependency is absent from the Windows target graph. A production Linux desktop release is blocked while the affected GTK3 stack is present. Server, Web, SDK, and Windows desktop artifacts are not blocked by this target-specific dependency.

Remediation requires one of the following reviewed upstream paths:

- a published GTK3 binding release using a fixed gtk-rs-core line, followed by compatible Tauri and Wry adoption;
- a supported Tauri GTK4 backend; or
- removal of the GTK3 Linux desktop backend.

After an upstream path is available, the change must pass Linux desktop compilation, workspace formatting, Clippy, tests, dependency audit, and the protected pull-request gate before issue #94 and the alert are closed.

## Consequences

- The unresolved upstream risk stays visible in repository security reporting and release governance.
- Windows-first delivery is not incorrectly blocked by a package that is absent from the Windows target graph.
- Linux desktop production distribution remains explicitly unavailable until the vulnerable stack is retired.
- Upstream status must be reviewed during dependency maintenance and before expanding supported desktop platforms.
