# Cargo.toml (Root Workspace)

## Path
`backend/Cargo.toml`

## Purpose
Root workspace manifest file for the Omega project. Defines a Cargo workspace containing 6 crates, establishes shared dependencies with explicit versions, and configures workspace-level settings that apply to all member crates. The root workspace also provides the main binary target (`omega`).

## Workspace Configuration
- **Resolver Version:** `2` (modern dependency resolution)
- **Members:** All crates under `crates/*` (glob pattern)

## Workspace-Level Package Settings
These settings are inherited by all member crates unless explicitly overridden:

| Setting | Value |
|---------|-------|
| Version | `0.1.0` |
| Edition | `2021` (Rust 2021 edition) |
| License | `MIT OR Apache-2.0` (dual-licensed) |
| Repository | `https://github.com/omgagi/omega` |

## Workspace Members
The following 6 crates are part of this workspace:

1. **`omega-core`** (`crates/omega-core`)
   - Core types, traits, configuration structures, error handling, and prompt sanitization

2. **`omega-providers`** (`crates/omega-providers`)
   - AI backend integrations (Claude Code CLI, Anthropic, OpenAI, Ollama, OpenRouter, Gemini) with agentic tool loop and MCP client

3. **`omega-channels`** (`crates/omega-channels`)
   - Messaging platform implementations (Telegram with voice/photo, WhatsApp with voice/image/groups/markdown)

4. **`omega-memory`** (`crates/omega-memory`)
   - SQLite-based storage layer, conversation history, facts, scheduled tasks, reward-based learning, and audit logging

5. **`omega-skills`** (`crates/omega-skills`)
   - Skill loader (`~/.omega/skills/*/SKILL.md`) + project loader (`~/.omega/projects/*/ROLE.md`), trigger-based MCP server activation

6. **`omega-sandbox`** (`crates/omega-sandbox`)
   - OS-level filesystem protection -- Seatbelt (macOS) / Landlock (Linux), code-level write/read blocking, always active

## Workspace Dependencies
All dependencies are declared at workspace level for consistency and easier version management:

### Async Runtime & Concurrency
- **`tokio`** `1.x` - Async runtime with full feature set
  - Features: `full` (includes all tokio features)

- **`async-trait`** `0.1.x` - Async trait support for trait objects

### Serialization & Configuration
- **`serde`** `1.x` - Serialization/deserialization framework
  - Features: `derive` (macros for custom types)

- **`serde_json`** `1.x` - JSON support

- **`toml`** `0.8.x` - TOML parsing and serialization (for config files)

### HTTP Client
- **`reqwest`** `0.12.x` - HTTP client library
  - Features: `json` (JSON support), `rustls-tls` (TLS via rustls, not openssl), `multipart` (file uploads)

### Database
- **`sqlx`** `0.8.x` - Async SQL toolkit
  - Features: `runtime-tokio` (tokio integration), `sqlite` (SQLite driver)

### Logging & Tracing
- **`tracing`** `0.1.x` - Structured logging framework
- **`tracing-subscriber`** `0.3.x` - Tracing utilities and subscribers
  - Features: `env-filter` (environment-based filtering)
- **`tracing-appender`** `0.2.x` - Non-blocking file log appender (used for `~/.omega/logs/omega.log`)

### Error Handling
- **`thiserror`** `2.x` - Derive macros for `std::error::Error`
- **`anyhow`** `1.x` - Flexible error handling with context

### CLI
- **`clap`** `4.x` - Command-line argument parsing
  - Features: `derive` (procedural macros for CLI definitions), `env` (environment variable support)

### Utilities
- **`uuid`** `1.x` - UUID generation
  - Features: `v4` (v4 UUIDs), `serde` (serialization support)

- **`chrono`** `0.4.x` - Date and time handling
  - Features: `serde` (serialization support)

## Root Package Configuration

### Package Metadata
- **Name:** `omega`
- **Description:** "Personal AI agent infrastructure, forged in Rust"
- **Version, Edition, License, Repository:** Inherited from workspace settings

### Binary Target
- **Binary Name:** `omega`
- **Entry Point:** `src/main.rs`

## Root Package Dependencies
The root `omega` binary depends on all 6 internal crates and a curated selection of workspace dependencies:

**Internal Crates:**
- `omega-core`
- `omega-providers`
- `omega-channels`
- `omega-memory`
- `omega-skills`
- `omega-sandbox`

**External Dependencies (from workspace):**
- `tokio` (async runtime for main event loop)
- `clap` (CLI argument parsing)
- `tracing` and `tracing-subscriber` (structured logging to stdout)
- `tracing-appender` (non-blocking file log output)
- `anyhow` (error handling)
- `serde_json` (JSON processing)
- `serde` (serialization)
- `sqlx` (database access)
- `chrono` (timestamp handling)
- `reqwest` (HTTP requests)
- `toml` (configuration parsing)
- `uuid` (UUID generation for request IDs)

**Binary-specific (not workspace):**
- `libc` `0.2.x` - FFI bindings for `geteuid()` root detection
- `cliclack` `0.3.x` - Styled interactive CLI prompts for init wizard and status commands
- `console` `0.15.x` - Terminal text styling (bold, color) used by init_style and CLI output
- `axum` `0.8.x` - Lightweight HTTP framework for the API server (health, QR pairing, webhooks)
- `base64` `0.22.x` - Base64 encoding for QR code PNG images in API responses

**Dev Dependencies:**
- `tower` `0.5.x` - Service abstraction (used for API test utilities)
- `http-body-util` `0.1.x` - HTTP body utilities (API test assertions)
- `async-trait` (from workspace, for mock providers in tests)
- `tempfile` `3.x` - Temporary files/directories for tests

## Notable Design Decisions

1. **Workspace Dependency Management:** All external dependencies are declared at the workspace level (`[workspace.dependencies]`), allowing member crates to reference them with `workspace = true`. This ensures version consistency across the project.

2. **TLS Configuration:** `reqwest` uses `rustls-tls` instead of the default OpenSSL, reducing dependencies and improving security posture. Multipart feature enabled for file uploads (voice, photos).

3. **SQLite as Primary Storage:** `sqlx` is configured specifically for SQLite async runtime, reflecting the project's decision to use SQLite for all persistence (memory, audit logs, scheduled tasks, learning).

4. **Full Tokio Features:** The workspace enables all tokio features (`features = ["full"]`) to avoid feature resolution issues during development.

5. **Async-First Design:** The presence of `tokio`, `async-trait`, and `sqlx` with async runtime reflects the architecture's commitment to fully async I/O operations.

6. **Dual Licensing:** MIT OR Apache-2.0 dual license allows flexibility for diverse use cases.

7. **Axum for HTTP API:** Lightweight HTTP server for SaaS dashboard integration, same binary -- no separate service needed.

8. **Cliclack for CLI UX:** All interactive terminal flows use cliclack for styled prompts rather than raw println.

## Version Lock
The workspace uses specific major versions without pre-release specifiers (e.g., `1`, `2`, `0.1`, `0.2`), allowing patch and minor updates within those versions. This balances stability with access to bug fixes.
