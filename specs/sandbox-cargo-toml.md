# Technical Specification: omega-sandbox/Cargo.toml

## File

| Field       | Value                                                        |
|-------------|--------------------------------------------------------------|
| Path        | `crates/omega-sandbox/Cargo.toml`                            |
| Crate name  | `omega-sandbox`                                              |
| Description | Secure execution environment for Omega                       |
| Role        | Defines the build manifest for the `omega-sandbox` crate     |

## Purpose

Defines the `omega-sandbox` crate, which provides OS-level system protection for AI provider subprocesses. The crate uses a blocklist approach to block writes to dangerous system directories and OMEGA's core database, using Apple Seatbelt (macOS) and Landlock LSM (Linux).

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

The crate uses `workspace = true` for all core dependencies and all package metadata fields. No dependency versions or feature flags are declared locally for workspace deps; all resolution is deferred to `[workspace.dependencies]` in the root `Cargo.toml`.

## Dependencies

### External Dependencies

| Dependency    | Workspace Version | Features Enabled | Purpose                           |
|---------------|-------------------|------------------|-----------------------------------|
| `tokio`       | `1`               | `full`           | Async runtime, `tokio::process::Command` |
| `tracing`     | `0.1`             | (none)           | Structured logging for fallback warnings and sandbox events |
| `anyhow`      | `1`               | (none)           | Ergonomic error propagation in Landlock setup |

### Platform-Specific Dependencies

| Target | Dependency | Version | Purpose |
|--------|-----------|---------|---------|
| `cfg(target_os = "linux")` | `landlock` | `0.4` | Landlock LSM filesystem restrictions |

Note: The `landlock` dependency uses a direct version declaration (not workspace inheritance) because it is platform-specific and only needed on Linux. Target-specific dependencies are declared using Cargo's `[target.'cfg(...)'.dependencies]` syntax.

### Dependency Count

- **Direct dependencies:** 3 (all external)
- **Internal crate dependencies:** 0
- **Platform-specific dependencies:** 1 (landlock, Linux only)
- **Dev dependencies:** 0
- **Build dependencies:** 0

## Dependency Graph (Direct)

```
omega-sandbox
  +-- tokio 1 [full]
  +-- tracing 0.1
  +-- anyhow 1
  +-- [linux] landlock 0.4
```

## Integration with Workspace Root Cargo.toml

The workspace root `Cargo.toml` registers `omega-sandbox` in two places:

1. **Workspace member** — all crates under `crates/*` are automatically included via the `members = ["crates/*"]` glob pattern.

2. **Workspace dependency** — declared as an internal workspace dependency:

   ```toml
   [workspace.dependencies]
   omega-sandbox = { path = "crates/omega-sandbox" }
   ```

3. **Root binary dependency** — the root `omega` binary includes `omega-sandbox`:

   ```toml
   [dependencies]
   omega-sandbox = { workspace = true }
   ```

4. **Provider dependency** — `omega-providers` also depends on `omega-sandbox`:

   ```toml
   [dependencies]
   omega-sandbox = { workspace = true }
   ```

## Notes

- The crate has **no internal crate dependencies** — it does not depend on `omega-core` or any other workspace crate. This makes it fully standalone.
- The macOS Seatbelt implementation uses `sandbox-exec` which is a built-in macOS binary (`/usr/bin/sandbox-exec`). No additional macOS-specific crate dependency is needed.
- The `landlock` crate is Linux-only and declared under `[target.'cfg(target_os = "linux")'.dependencies]`.
- The crate does **not** declare any `[[bin]]`, `[[example]]`, or `[[bench]]` targets. It is a pure library crate.
- No `[dev-dependencies]` or `[build-dependencies]` sections are present.
- Previous versions depended on `omega-core` for `SandboxMode`, `serde`, and `thiserror`. These were removed when the crate switched from an allowlist (3 modes) to a blocklist (always-on) approach.
