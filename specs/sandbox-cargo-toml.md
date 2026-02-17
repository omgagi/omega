# Technical Specification: omega-sandbox/Cargo.toml

## File

| Field       | Value                                                        |
|-------------|--------------------------------------------------------------|
| Path        | `crates/omega-sandbox/Cargo.toml`                            |
| Crate name  | `omega-sandbox`                                              |
| Description | Secure execution environment for Omega                       |
| Role        | Defines the build manifest for the `omega-sandbox` crate     |

## Purpose

Defines the `omega-sandbox` crate, which will provide a secure, isolated execution environment for running commands and code on behalf of Omega. This crate is currently a scaffold (Phase 4, planned) with its public API stubbed out in `src/lib.rs`. The manifest declares only the minimal set of dependencies needed to build the crate and begin implementation.

## Package Metadata

All package metadata fields are inherited from the workspace root (`Cargo.toml`).

| Field        | Value (inherited)                              |
|--------------|------------------------------------------------|
| `version`    | `0.1.0`                                        |
| `edition`    | `2021`                                         |
| `license`    | `MIT OR Apache-2.0`                            |
| `repository` | `https://github.com/omega-cortex/omega`        |

The `description` field is set locally to `"Secure execution environment for Omega"`.

## Workspace Inheritance

The crate uses `workspace = true` for every dependency and all package metadata fields. No dependency versions or feature flags are declared locally; all resolution is deferred to `[workspace.dependencies]` in the root `Cargo.toml`. This guarantees version consistency across the entire workspace.

The following fields are inherited from `[workspace.package]`:

- `version`
- `edition`
- `license`
- `repository`

All dependencies are inherited from `[workspace.dependencies]` with no local overrides. This means version bumps and feature changes are performed exclusively in the root `Cargo.toml`.

## Dependencies

### Internal Crate Dependencies

| Dependency   | Workspace Ref          | Resolved Path           |
|--------------|------------------------|-------------------------|
| `omega-core` | `{ workspace = true }` | `crates/omega-core`     |

### External Dependencies

| Dependency    | Workspace Version | Features Enabled | Purpose in Sandbox Crate                           |
|---------------|-------------------|------------------|----------------------------------------------------|
| `tokio`       | `1`               | `full`           | Async runtime for spawning and managing sandboxed processes, timers, and I/O |
| `serde`       | `1`               | `derive`         | Serialization/deserialization of sandbox configuration, execution requests, and results |
| `tracing`     | `0.1`             | (none)           | Structured logging for sandbox lifecycle events (creation, execution, teardown, violations) |
| `thiserror`   | `2`               | (none)           | Typed error definitions for sandbox-specific failure modes (permission denied, timeout, resource limits exceeded) |
| `anyhow`      | `1`               | (none)           | Ergonomic error propagation with context for sandbox operations |

### Dependency Count

- **Direct dependencies:** 6 (1 internal + 5 external)
- **Dev dependencies:** 0
- **Build dependencies:** 0

### Dependencies NOT Used by This Crate

The following workspace dependencies exist but are not declared in `omega-sandbox`:

| Dependency            | Reason for Exclusion                                         |
|-----------------------|--------------------------------------------------------------|
| `serde_json`          | No JSON parsing needed yet; sandbox communicates via typed Rust structs through `omega-core` |
| `toml`                | Config parsing is handled by `omega-core`                    |
| `reqwest`             | HTTP is for API calls in `omega-providers` and `omega-channels`, not command execution |
| `sqlx`                | Database access is encapsulated in `omega-memory`            |
| `tracing-subscriber`  | Subscriber setup is handled by the root binary               |
| `clap`                | CLI argument parsing is handled by the root binary           |
| `uuid`                | Unique identifiers for sandbox sessions may be added later if needed |
| `chrono`              | Timestamps may be added later for execution timing and audit integration |
| `async-trait`         | No trait definitions in this crate yet; concrete types are planned |

## Feature Configuration

The `omega-sandbox` crate defines **no local features**. All feature flags are inherited transitively through workspace dependency declarations. There is no `[features]` section in this manifest.

## Dependency Graph (Direct)

```
omega-sandbox
  +-- omega-core (internal, workspace path)
  +-- tokio 1 [full]
  +-- serde 1 [derive]
  +-- tracing 0.1
  +-- thiserror 2
  +-- anyhow 1
```

## Integration with Workspace Root Cargo.toml

The workspace root `Cargo.toml` registers `omega-sandbox` in two places:

1. **Workspace member** -- all crates under `crates/*` are automatically included via the `members = ["crates/*"]` glob pattern.

2. **Workspace dependency** -- the root declares `omega-sandbox` as an internal workspace dependency:

   ```toml
   [workspace.dependencies]
   omega-sandbox = { path = "crates/omega-sandbox" }
   ```

3. **Root binary dependency** -- the root `omega` binary includes `omega-sandbox` in its `[dependencies]`:

   ```toml
   [dependencies]
   omega-sandbox = { workspace = true }
   ```

This means the sandbox crate is compiled and linked into the final `omega` binary, even though its implementation is currently a stub.

## Resolver

The workspace uses Cargo resolver version `2` (set in the root `Cargo.toml`), which is required for edition 2021 workspaces and provides improved feature unification behavior.

## Notes

- The crate is currently a **scaffold**. The `src/lib.rs` file contains only a module-level doc comment. The core sandbox logic (`SandboxMode` enum, `SandboxConfig` struct, `prompt_constraint()` method) lives in `omega-core::config`. This crate's dependency list represents the anticipated baseline for future process-level isolation implementation.
- The crate does **not** depend on `serde_json`, `uuid`, or `chrono`. These may be added during Phase 4 implementation when execution results need serialization, unique session tracking, and timing.
- The crate does **not** depend on `async-trait`. If sandbox behavior is exposed via a trait (e.g., `Executor`), this dependency will need to be added.
- The crate does **not** declare any `[[bin]]`, `[[example]]`, or `[[bench]]` targets. It is a pure library crate.
- No `[dev-dependencies]` or `[build-dependencies]` sections are present.
- The dependency set is intentionally minimal -- only the "foundation six" dependencies (`omega-core`, `tokio`, `serde`, `tracing`, `thiserror`, `anyhow`) that nearly every Omega crate requires. Additional dependencies will be added as implementation progresses.
- Future implementation may require platform-specific dependencies for process isolation (e.g., `nix` for Unix namespace/seccomp support, `caps` for Linux capabilities, or `sandbox` for macOS sandbox profiles).
