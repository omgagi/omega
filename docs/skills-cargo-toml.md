# omega-skills: Cargo.toml Guide

This document explains the Cargo manifest for the `omega-skills` crate -- what the crate does, what each dependency is for, how workspace inheritance works, and how to add new dependencies.

## What Is This Crate?

`omega-skills` is the skill and project loader for Omega. It scans `~/.omega/skills/*/SKILL.md` and `~/.omega/projects/*/ROLE.md` for definitions and exposes them to the system prompt so the AI knows what tools and contexts are available.

The crate provides:

- **Skill loading** -- parse TOML or YAML frontmatter from `SKILL.md` files, check CLI tool availability, extract MCP server declarations, match trigger keywords
- **Project loading** -- parse `ROLE.md` files with optional frontmatter for project-scoped skills
- **Bundled skill deployment** -- embed core skills at compile time, deploy on first run without overwriting user edits
- **Flat-to-directory migration** -- migrate legacy `skills/*.md` files to `skills/*/SKILL.md` layout

The file lives at:

```
backend/crates/omega-skills/Cargo.toml
```

## How Workspace Inheritance Works

Almost every field in this manifest says `workspace = true` rather than specifying a value directly:

```toml
[package]
name = "omega-skills"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Skill and plugin system for Omega"
```

This means the actual values come from the root `Cargo.toml` under `[workspace.package]`:

```toml
# Root Cargo.toml
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/omega-cortex/omega"
```

The same pattern applies to dependencies. When you write:

```toml
serde = { workspace = true }
```

Cargo looks up `serde` in the root `[workspace.dependencies]` table and pulls in the version and feature flags defined there. This keeps all version pinning in one place.

Only `name` and `description` are defined locally, since those are unique to each crate.

## Dependencies Explained

The crate has exactly 4 dependencies:

### omega-core

```toml
omega-core = { workspace = true }
```

Omega's core crate. Provides the `McpServer` type (used in skill trigger matching), configuration utilities, and shared types. Skills import `omega_core::context::McpServer` to populate MCP server lists that get passed to the provider layer.

### serde

```toml
serde = { workspace = true }   # version 1, features = ["derive"]
```

Serialization framework. Used with `#[derive(Deserialize)]` on frontmatter structs (`SkillFrontmatter`, `ProjectFrontmatter`, `McpFrontmatter`) to parse TOML frontmatter from skill and project files.

### toml

```toml
toml = { workspace = true }
```

TOML parser. Used as the primary frontmatter format in `SKILL.md` and `ROLE.md` files. The crate tries `toml::from_str()` first and falls back to a lightweight YAML-style parser for compatibility.

### tracing

```toml
tracing = { workspace = true }   # version 0.1
```

Structured logging. The project rule is: no `println!`, use `tracing` instead. Typical usage:

```rust
tracing::info!(skill = "gog", "Skill loaded");
tracing::warn!("skills: no valid frontmatter in {}", path.display());
```

Note that `tracing-subscriber` (which outputs logs to console or file) is **not** a dependency. That responsibility belongs to the root binary.

## What is NOT Here (and Why)

Several workspace dependencies are intentionally absent:

| Crate | Why absent |
|-------|-----------|
| `tokio` | All skill operations are synchronous (`std::fs`). No async I/O needed. |
| `async-trait` | No async traits. The crate exposes plain functions, not trait impls. |
| `thiserror` | No custom error types. Functions return `Vec` (empty on failure) or use `Option`. |
| `anyhow` | No `Result` returns from public functions. Errors are logged via `tracing::warn` and skipped. |
| `serde_json` | Not needed. Frontmatter is TOML (or YAML-style). The only JSON parsing is a minimal `extract_bins_from_metadata()` helper that uses string searching rather than a full parser. |
| `reqwest` | No HTTP calls. Skills are file-based definitions, not runtime services. |
| `sqlx` | Database access goes through `omega-memory`, not from skills. |

This keeps `omega-skills` lean with minimal compile-time overhead.

## Module Structure

The crate is organized into 3 internal modules:

| Module | Visibility | Purpose |
|--------|-----------|---------|
| `parse` | `mod` (private) | Shared utilities: `expand_tilde`, `unquote`, `parse_yaml_list`, `extract_bins_from_metadata`, `which_exists`, `data_path` |
| `skills` | `mod` (private) | Skill loading, parsing, deployment, migration, trigger matching |
| `projects` | `mod` (private) | Project loading, parsing, directory creation |

The public API is re-exported from `lib.rs`:

```rust
// Skills
pub use skills::{
    build_skill_prompt, install_bundled_skills, load_skills,
    match_skill_triggers, migrate_flat_skills, Skill,
};

// Projects
pub use projects::{
    ensure_projects_dir, get_project_instructions, load_projects, Project,
};
```

## How to Add a New Dependency

Because all dependencies use workspace inheritance, adding a new dependency is a two-step process.

**Step 1: Add it to the workspace root first.**

Open the root `Cargo.toml` and add the dependency under `[workspace.dependencies]`:

```toml
[workspace.dependencies]
# ... existing deps ...
my-new-crate = { version = "3.0", features = ["something"] }
```

If the dependency already exists in the workspace root (because another crate uses it), skip this step.

**Step 2: Reference it from this crate.**

In `backend/crates/omega-skills/Cargo.toml`, add:

```toml
[dependencies]
# ... existing deps ...
my-new-crate = { workspace = true }
```

**Step 3: Verify everything compiles.**

```bash
cargo check -p omega-skills
cargo clippy --workspace
cargo test --workspace
```

### Before Adding a Dependency, Ask Yourself

- **Does this belong in `omega-skills`, or should it live in a more specific crate?** Skills are file-based definitions -- keep runtime dependencies minimal.
- **Is this crate well-maintained and widely used?** Prefer established crates from the Rust ecosystem.
- **Does it add significant compile time?** The skill crate is compiled as part of the full workspace.
- **Could `omega-core` already provide what you need?** Check core types and traits before pulling in something new.

## Things to Keep in Mind

- Always run `cargo clippy --workspace && cargo test --workspace && cargo fmt --check` before committing changes to any `Cargo.toml` in the workspace.
- The crate is fully implemented with 3 modules (`parse`, `skills`, `projects`) and comprehensive tests.
- The dependency set is intentionally minimal (4 crates). The design avoids async, custom errors, and JSON parsing in favor of simplicity.
- The `Skill` struct is a plain data struct, not a trait. Skills are file-based definitions loaded from disk, not runtime plugins.
