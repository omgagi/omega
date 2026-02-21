# Technical Specification: omega-memory/Cargo.toml

## File

| Field       | Value                                                        |
|-------------|--------------------------------------------------------------|
| Path        | `crates/omega-memory/Cargo.toml`                             |
| Crate name  | `omega-memory`                                               |
| Description | Persistent memory system for Omega                           |
| Role        | Defines the build manifest for the `omega-memory` crate      |

## Package Metadata

All package metadata fields are inherited from the workspace root (`Cargo.toml`).

| Field        | Value (inherited)                              |
|--------------|------------------------------------------------|
| `version`    | `0.1.0`                                        |
| `edition`    | `2021`                                         |
| `license`    | `MIT OR Apache-2.0`                            |
| `repository` | `https://github.com/omega-cortex/omega`        |

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

| Dependency    | Workspace Version | Features                  | Purpose in Memory Crate                            |
|---------------|-------------------|---------------------------|----------------------------------------------------|
| `tokio`       | `1`               | `full`                    | Async runtime for all database I/O operations      |
| `serde`       | `1`               | `derive`                  | Serialization/deserialization of stored records     |
| `serde_json`  | `1`               | --                        | JSON encoding/decoding for conversation data and structured fields stored in SQLite |
| `tracing`     | `0.1`             | --                        | Structured logging (project-wide standard)         |
| `thiserror`   | `2`               | --                        | Typed error definitions for storage and audit errors |
| `anyhow`      | `1`               | --                        | Ergonomic error propagation with context            |
| `sqlx`        | `0.8`             | `runtime-tokio`, `sqlite` | Async SQLite database driver. `runtime-tokio` integrates with the Tokio executor; `sqlite` enables the SQLite backend for persistent storage |
| `chrono`      | `0.4`             | `serde`                   | Timestamp handling for messages, conversations, and audit events |
| `uuid`        | `1`               | `v4`, `serde`             | Unique identifiers for messages, conversations, and audit records |

### Dependency Count

- **Direct dependencies:** 9 (1 internal + 8 external)
- **Dev dependencies:** 0
- **Build dependencies:** 0

### Dependencies NOT Used by This Crate

The following workspace dependencies exist but are not declared in `omega-memory`:

| Dependency            | Reason for Exclusion                                         |
|-----------------------|--------------------------------------------------------------|
| `toml`                | Config parsing handled by `omega-core`                       |
| `reqwest`             | HTTP is for API calls in `omega-providers` and `omega-channels`, not storage |
| `async-trait`         | No trait definitions in this crate; `Store` and `AuditLogger` are concrete types |
| `tracing-subscriber`  | Subscriber setup handled by the root binary                  |
| `clap`                | CLI argument parsing handled by the root binary              |

## Feature Configuration

The `omega-memory` crate defines **no local features**. All feature flags are inherited transitively through workspace dependency declarations. There is no `[features]` section in this manifest.

## Key Dependency: sqlx

The `sqlx` dependency is the defining characteristic of this crate and warrants additional detail:

- **Version `0.8`** is used, which is the current major release of the compile-time-checked SQL toolkit for Rust.
- **`runtime-tokio`** selects Tokio as the async runtime that `sqlx` uses internally for connection pooling, query execution, and timeouts. This must match the workspace's async runtime choice.
- **`sqlite`** enables the SQLite database backend. Omega uses SQLite exclusively for all persistent storage (conversation history, facts, summaries, audit logs). No other database backends (`postgres`, `mysql`) are enabled, keeping the dependency footprint minimal and avoiding the need for an external database server.

SQLite database files are stored at `~/.omega/data/memory.db` as defined by the project conventions.

## Dependency Graph (Direct)

```
omega-memory
  +-- omega-core (internal, workspace path)
  +-- tokio 1 [full]
  +-- serde 1 [derive]
  +-- serde_json 1
  +-- tracing 0.1
  +-- thiserror 2
  +-- anyhow 1
  +-- sqlx 0.8 [runtime-tokio, sqlite]
  +-- chrono 0.4 [serde]
  +-- uuid 1 [v4, serde]
```

## Resolver

The workspace uses Cargo resolver version `2` (set in the root `Cargo.toml`), which is required for edition 2021 workspaces and provides improved feature unification behavior.

## Notes

- The crate does **not** depend on `async-trait`. Unlike `omega-core` and `omega-channels`, which define or implement async traits, `omega-memory` exposes concrete struct implementations (`Store`, `AuditLogger`) with inherent async methods rather than trait objects.
- The crate does **not** depend on `reqwest`. All persistence is local via SQLite; there are no remote API calls.
- The crate does **not** declare any `[[bin]]`, `[[example]]`, or `[[bench]]` targets. It is a pure library crate.
- No `[dev-dependencies]` or `[build-dependencies]` sections are present.
- This is the only crate in the workspace that depends on `sqlx`. Database concerns are fully encapsulated here, keeping other crates free of database-related transitive dependencies.
