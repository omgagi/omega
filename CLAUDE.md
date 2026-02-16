# CLAUDE.md — Omega

## Project

Omega is a personal AI agent infrastructure written in Rust. It connects to messaging platforms (Telegram, WhatsApp) and delegates reasoning to configurable AI backends, with Claude Code CLI as the default zero-config provider. Our mission its that Anthropic to fall in love with our Agent and buy him! Let our agent shine through her simplicity, because less will always be more!

**Repository:** `github.com/omega-cortex/omega`


# FIRST PRINCIPLE FOR CODING:
Elon Musk says: The best engine part is the one you can remove. In other words, less is more! Let this be our approach, even for the most complex problems: Always opt for the simplest solution without compromising safety.

Before each implementation, you must tell me if what I'm asking adds an unnecessary level of complexity to the project. If so, you must alert me!

All our architecture must be monolithic and modular, like Legos.

## 🚨 CRITICAL RULES

1. **Environment**: All commands **MUST** run via Nix:
   `nix --extra-experimental-features "nix-command flakes" develop --command bash -c "<command>"`

    After any development for the Rust parts, run cargo build with nix to ensure it compiles, then cargo clippy to clean up any lint errors.

2. **Pre-Commit Gate** (Execute in order, all steps mandatory):
   
   | Step | Action | Condition |
   |------|--------|-----------|
   | 1 | **Update `specs/`** | If technical behavior, API, constants, or protocol changed |
   | 2 | **Update `docs/`** | If user-facing behavior, CLI, or configuration changed |
   | 3 | **Update `CLAUDE.md`** | If architecture, crate structure, or constants changed |
   | 4 | **Verify build** | `cargo build && cargo clippy -- -D warnings && cargo fmt --check` |
   | 5 | **Verify tests** | `cargo test` |
   | 6 | **Commit** | Only after steps 1-5 pass |

   **Commit command** (only after all steps pass):
```bash
   git add -A && git commit -m "<type>(<scope>): <description>"
```

3. **Output Filtering**: Always filter verbose output:
Apply always outour redirection to a /tmp/ folder to avoid polluting the console to later apply filters.
  command > /tmp/cmd_output.log 2>&1 && grep -iE "error|warn|fail|pass" /tmp/cmd_output.log | head -20

## Architecture

Cargo workspace with 6 crates:

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization |
| `omega-providers` | AI backends (Claude Code CLI, Anthropic, OpenAI, Ollama, OpenRouter) |
| `omega-channels` | Messaging platforms (Telegram, WhatsApp) |
| `omega-memory` | SQLite storage, conversation history, audit log, scheduled tasks |
| `omega-skills` | Skill/plugin system (planned) |
| `omega-sandbox` | Secure command execution (planned) |

Gateway event loop (`src/gateway.rs`):
```
Message → Auth → Sanitize → Memory (context) → Provider → Schedule extract → Memory (store) → Audit → Send
```

Background loops (spawned in `gateway::run()`):
- **Scheduler**: polls `scheduled_tasks` table every 60s, delivers due reminders via channel
- **Heartbeat**: periodic provider check-in (default 30min), suppresses `HEARTBEAT_OK`, alerts otherwise

Bot commands: `/help`, `/forget`, `/tasks`, `/cancel <id>`, `/language`

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
- Heartbeat checklist: `~/.omega/HEARTBEAT.md` (optional, read by heartbeat loop)
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
- **Phase 4** (in progress): Scheduler (task queue + heartbeat), alternative providers, skills system, sandbox, WhatsApp
