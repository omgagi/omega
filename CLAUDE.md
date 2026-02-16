# CLAUDE.md — Omega

## Project

Omega is a personal AI agent infrastructure written in Rust. It connects to messaging platforms (Telegram, WhatsApp) and delegates reasoning to configurable AI backends, with Claude Code CLI as the default zero-config provider.

**Repository:** `github.com/omega-cortex/omega`

## Architecture

Cargo workspace with 6 crates:

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization |
| `omega-providers` | AI backends (Claude Code CLI, Anthropic, OpenAI, Ollama, OpenRouter) |
| `omega-channels` | Messaging platforms (Telegram, WhatsApp) |
| `omega-memory` | SQLite storage, conversation history, audit log |
| `omega-skills` | Skill/plugin system (planned) |
| `omega-sandbox` | Secure command execution (planned) |

Gateway event loop (`src/gateway.rs`):
```
Message → Auth → Sanitize → Memory (context) → Provider → Memory (store) → Audit → Send
```

## Build & Test

```bash
cargo check                  # Type check all crates
cargo clippy --workspace     # Zero warnings required
cargo test --workspace       # All tests must pass
cargo fmt                    # Always format before commit
cargo build --release        # Optimized binary
```

**Run all three before every commit:** `cargo clippy --workspace && cargo test --workspace && cargo fmt --check`

## Key Design Rules

- **No `unwrap()`** — use `?` and proper error types. Never panic in production code.
- **Tracing, not `println!`** — use `tracing` crate for all logging.
- **No `unsafe`** unless absolutely necessary (the only exception is `libc::geteuid()` for root detection).
- **Async everywhere** — tokio runtime, all I/O is async.
- **SQLite for everything** — memory, audit, state. No external database.
- **Config from file + env** — TOML primary, environment variables override.
- **Every public function gets a doc comment.**

## Security Constraints

- Omega **must not run as root**. A guard in `main.rs` rejects root execution.
- The Claude Code provider removes the `CLAUDECODE` env var to avoid nested session errors.
- Prompt sanitization in `omega-core/src/sanitize.rs` neutralizes injection patterns before they reach the provider.
- Auth is enforced per-channel via `allowed_users` in config.
- `config.toml` is gitignored — never commit secrets.

## File Conventions

- Config: `config.toml` (gitignored), `config.example.toml` (committed)
- Database: `~/.omega/memory.db`
- Logs: `~/.omega/omega.log`
- Service: `~/Library/LaunchAgents/com.omega-cortex.omega.plist`

## Provider Priority

Claude Code CLI is the primary provider. It invokes `claude -p --output-format json` as a subprocess. The JSON response has this structure:
```json
{"type": "result", "subtype": "success", "result": "...", "model": "...", "session_id": "..."}
```
When `subtype` is `error_max_turns`, extract `result` if available, otherwise return a meaningful fallback.

## Documentation

Always consult these before modifying or extending the codebase:

- **`specs/SPECS.md`** — Master index of technical specifications for every file in the repository
- **`docs/DOCS.md`** — Master index of developer-facing guides and references

## Current Status

- **Phase 1** (complete): Workspace, core types, Claude Code provider, CLI (`omega ask`)
- **Phase 2** (complete): Memory, Telegram channel, gateway, audit log, auth, sanitization, LaunchAgent
- **Phase 3** (complete): Conversation boundaries, summaries, facts extraction, enriched context, typing indicator, bot commands, system prompt upgrade, self-check, graceful shutdown, exponential backoff, init wizard
- **Phase 4** (next): Alternative providers, skills system, sandbox, cron scheduler, WhatsApp
