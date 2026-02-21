# omega-memory Cargo.toml -- Developer Guide

This document explains the build manifest for the `omega-memory` crate, which lives at `crates/omega-memory/Cargo.toml`. If you are working with persistent storage, modifying the database schema, or adding new data-related dependencies, this is the right place to start.

## What Does omega-memory Do?

The `omega-memory` crate is Omega's persistence layer. It is responsible for everything that needs to survive a process restart:

- **Conversation history** -- storing and retrieving messages exchanged between users and the AI.
- **Audit logging** -- recording every interaction for accountability and debugging.
- **Conversation metadata** -- summaries, extracted facts, and context windows used to enrich future prompts.

All of this is backed by SQLite via the `sqlx` crate. The database file lives at `~/.omega/data/memory.db`. There is no external database server to manage -- SQLite is embedded directly in the binary.

The crate exports two primary types:

- **`Store`** -- handles conversation storage, retrieval, summaries, and facts.
- **`AuditLogger`** -- writes immutable audit records for every message processed.

## How Workspace Inheritance Works

You will notice that every dependency in this crate looks like this:

```toml
tokio = { workspace = true }
```

This means the version, features, and other settings are **not** declared here. Instead, they are declared once in the root `Cargo.toml` under `[workspace.dependencies]`:

```toml
[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
```

When you write `{ workspace = true }` in a crate, Cargo pulls in whatever version and features the workspace root defines. This gives us two important guarantees:

1. **Every crate in the workspace uses the same version** of a given dependency. No version conflicts.
2. **Upgrades happen in one place.** Bump the version in the root `Cargo.toml` and every crate picks it up.

Package metadata fields (`version`, `edition`, `license`, `repository`) are also inherited the same way:

```toml
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/omega-cortex/omega"
```

## Dependencies and What They Do

### Internal

| Dependency   | What It Provides                                                         |
|--------------|--------------------------------------------------------------------------|
| `omega-core` | Shared types (`Message`, `Config`, error types, conversation boundaries) |

This is the only internal crate that `omega-memory` depends on. It does not depend on `omega-providers` or `omega-channels` -- the gateway in `src/gateway.rs` wires those together at a higher level.

### External

| Dependency    | Version | What It Is Used For                                                                                 |
|---------------|---------|-----------------------------------------------------------------------------------------------------|
| `tokio`       | 1       | The async runtime. All database operations run asynchronously on Tokio.                             |
| `serde`       | 1       | Deriving `Serialize` and `Deserialize` on stored record types and query results.                    |
| `serde_json`  | 1       | Encoding and decoding JSON for structured fields stored in SQLite (e.g., conversation metadata).    |
| `tracing`     | 0.1     | Structured logging. The project uses `tracing` everywhere instead of `println!` or `log`.           |
| `thiserror`   | 2       | Defining typed error enums for storage operations with clear `Display` implementations.             |
| `anyhow`      | 1       | Quick error propagation with `?` in functions that do not need typed errors.                        |
| `sqlx`        | 0.8     | The async SQLite driver. This is the core dependency of the crate -- all database access goes through `sqlx`. |
| `chrono`      | 0.4     | Working with timestamps on messages, conversations, and audit entries.                              |
| `uuid`        | 1       | Generating unique v4 UUIDs for messages, conversations, and audit records.                          |

### The Star Dependency: sqlx

The `sqlx` crate deserves special attention because it is the reason this crate exists. It provides:

- **Async database access** -- queries execute without blocking the Tokio runtime.
- **Compile-time query checking** (optional) -- SQL queries can be verified against the schema at compile time.
- **SQLite backend** -- enabled via the `sqlite` feature. No need for PostgreSQL, MySQL, or any external server.
- **Tokio integration** -- the `runtime-tokio` feature ensures `sqlx` uses the same executor as the rest of Omega.

The workspace declares `sqlx` with these features:

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
```

This is the minimal feature set needed. Adding `postgres` or `mysql` features would pull in unnecessary dependencies and increase compile times.

## What is NOT Here (and Why)

| Crate                | Where it lives instead      | Why                                                |
|----------------------|-----------------------------|----------------------------------------------------|
| `reqwest`            | `omega-providers`, `omega-channels` | HTTP is for API calls, not database storage. |
| `async-trait`        | `omega-core`, `omega-channels`      | This crate uses concrete structs, not trait objects. |
| `toml`               | `omega-core`                | Config parsing is not a storage concern.           |
| `clap`               | Root binary                 | CLI argument parsing happens only in `main.rs`.    |
| `tracing-subscriber` | Root binary                 | Log output setup is an application-level concern.  |

This separation keeps `omega-memory` focused on one job: persistent storage. It does not parse config, handle HTTP, or define abstract traits. It receives data, stores it in SQLite, and retrieves it when asked.

## How to Add a New Dependency

Because all dependencies use workspace inheritance, adding a new dependency to `omega-memory` is a two-step process:

### Step 1: Add it to the workspace root

Open the root `Cargo.toml` and add the dependency under `[workspace.dependencies]`:

```toml
[workspace.dependencies]
# ... existing entries ...
my-new-crate = { version = "2.0", features = ["some-feature"] }
```

### Step 2: Reference it in the crate

Open `crates/omega-memory/Cargo.toml` and add:

```toml
[dependencies]
my-new-crate = { workspace = true }
```

That is it. The version and features come from the workspace definition.

### Step 3: If only this crate needs a feature

If `omega-memory` needs a feature that no other crate needs, you can add it locally without overriding the workspace version:

```toml
[dependencies]
my-new-crate = { workspace = true, features = ["extra-feature"] }
```

This merges `extra-feature` with whatever features the workspace already declares.

## When You Might Modify This File

Common scenarios where you would touch `crates/omega-memory/Cargo.toml`:

- **Adding a new storage backend** -- if you needed Redis or DuckDB alongside SQLite, you would add the corresponding driver crate here.
- **Adding full-text search** -- a crate like `tantivy` would be added here if you needed full-text search beyond what SQLite's FTS5 provides.
- **Adding encryption at rest** -- if you needed to encrypt the SQLite database, you might add `sqlcipher` support or an encryption crate.
- **Adding migration tooling** -- if `sqlx`'s built-in migration support is not sufficient, you might add a dedicated migration crate.

Before adding a dependency, ask yourself:

- Does this belong in `omega-memory`, or should it live in a more specific crate?
- Is this crate well-maintained and widely used?
- Does it add significant compile time?

Keeping `omega-memory` focused on storage benefits the entire workspace.

## Common Tasks

**Check that everything compiles:**

```bash
cargo check -p omega-memory
```

**Run clippy on just this crate:**

```bash
cargo clippy -p omega-memory
```

**See the full resolved dependency tree:**

```bash
cargo tree -p omega-memory
```

This shows every transitive dependency, which is useful for debugging version conflicts or understanding binary size.

**Run tests for just this crate:**

```bash
cargo test -p omega-memory
```
