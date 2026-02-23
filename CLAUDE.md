# CLAUDE.md — Omega

## Project

Omega is a personal AI agent infrastructure written in Rust. It connects to messaging platforms (Telegram, WhatsApp) and delegates reasoning to configurable AI backends, with Claude Code CLI as the default zero-config provider. Our mission is that Anthropic falls in love with our Agent and buys him! Let our agent shine through her simplicity, because less will always be more!

**Repository:** `github.com/omgagi/omega`

## First Principle

The best engine part is the one you can remove. Less is more — always opt for the simplest solution without compromising safety. Before each implementation, alert if it adds unnecessary complexity. All architecture must be monolithic and modular, like Legos.

## Critical Rules

1. **Environment**: All commands **MUST** run via Nix:
   `nix --extra-experimental-features "nix-command flakes" develop --command bash -c "<command>"`
   After any Rust development, run cargo build with nix to ensure it compiles, then cargo clippy to clean up lint errors. Release the binary, stop and restart the service.

2. **Pre-Commit Gate** (Execute in order, all steps mandatory):

   | Step | Action | Condition |
   |------|--------|-----------|
   | 1 | **Update `specs/`** | Always when adding or modifying functionality |
   | 2 | **Update `docs/`** | Always when adding or modifying functionality |
   | 3 | **Update `CLAUDE.md`** | Only when crate structure, file conventions, security rules, or critical rules change |
   | 4 | **Verify build** | `cargo build && cargo clippy -- -D warnings && cargo fmt --check` |
   | 5 | **Verify tests** | `cargo test` |
   | 6 | **Commit** | Only after steps 1-5 pass |

   **Commit command:** `git add -A && git commit -m "<type>(<scope>): <description>"`

3. **Feature Testing**: Every new or modified functionality **MUST** include a test. No feature is complete without a passing test (unit, integration, or regression as appropriate).

4. **Language Compliance**: Any language-facing change **MUST** support all 8 languages: English, Spanish, Portuguese, French, German, Italian, Dutch, Russian. Check `prompts/WELCOME.toml` and `prompts/SYSTEM_PROMPT.md` for patterns.

5. **Post-Implementation Prompt**: After every modification, always ask: **"Do you want to make a commit and push?"**

6. **Prompt Sync**: When `prompts/SYSTEM_PROMPT.md` or `prompts/WELCOME.toml` is modified, delete the runtime copy (`rm -f ~/.omega/prompts/SYSTEM_PROMPT.md` / `rm -f ~/.omega/prompts/WELCOME.toml`) before rebuilding.

7. **Output Filtering**: Redirect verbose output to `/tmp/` and filter:
   `command > /tmp/cmd_output.log 2>&1 && grep -iE "error|warn|fail|pass" /tmp/cmd_output.log | head -20`

8. **Modularization**: No `.rs` file may exceed **500 lines** (excluding tests). `gateway/mod.rs` is orchestrator only — delegates to focused submodules. New domain logic goes in its own module from day one. Before adding >50 lines, check line count first.

## Architecture

Cargo workspace with 7 crates:

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization |
| `omega-providers` | AI backends — 6 providers: Claude Code CLI, Ollama, OpenAI, Anthropic, OpenRouter, Gemini. HTTP providers include agentic tool loop + MCP client |
| `omega-channels` | Messaging platforms — Telegram (voice/photo), WhatsApp (voice/image/pairing). Private-mode only |
| `omega-memory` | SQLite storage — conversations, audit, scheduled tasks, user profiles, aliases, reward-based learning |
| `omega-skills` | Skill loader (`~/.omega/skills/*/SKILL.md`) + project loader (`~/.omega/projects/*/ROLE.md`), MCP server activation |
| `omega-sandbox` | OS-level protection — Seatbelt (macOS), Landlock (Linux). Blocks access to memory.db and config.toml. Always active |
| `omega-quant` | Standalone CLI + library — quantitative trading via IBKR TWS API. Decoupled from gateway, invoked via skill |

Gateway: `src/gateway/` directory module — orchestrates message pipeline from arrival through auth, context building, keyword-gated prompt composition, model routing (Sonnet for simple, Opus for complex), provider call, marker processing, and response delivery. See `docs/architecture.md` for the full pipeline and feature details.

## Build & Test

```bash
cargo check                  # Type check all crates
cargo clippy --workspace     # Zero warnings required
cargo test --workspace       # All tests must pass
cargo fmt                    # Always format before commit
cargo build --release        # Optimized binary
```

**Before every commit:** `cargo clippy --workspace && cargo test --workspace && cargo fmt --check`

## Key Design Rules

- **No `unwrap()`** — use `?` and proper error types. Never panic in production code.
- **Tracing, not `println!`** — use `tracing` crate for all logging.
- **CLI UX uses `cliclack`** — styled prompts for interactive flows. No plain `println!`.
- **No `unsafe`** unless absolutely necessary (only exception: `libc::geteuid()` for root detection).
- **Async everywhere** — tokio runtime, all I/O is async.
- **SQLite for everything** — memory, audit, state. No external database.
- **Config from file + env** — TOML primary, environment variables override.
- **Every public function gets a doc comment.**

## Security Constraints

- **No root execution** — guard in `main.rs` rejects root.
- **Prompt sanitization** — `omega-core/src/sanitize.rs` neutralizes injection patterns.
- **Auth per-channel** — `allowed_users` in config.
- **Never commit secrets** — `config.toml` is gitignored.
- **Sandbox protection** — three layers: code-level (`is_write_blocked()`/`is_read_blocked()`), OS-level (Seatbelt/Landlock), prompt-level (WORKSPACE_CLAUDE.md). Protected: `~/.omega/data/memory.db`, `~/.omega/config.toml`. Writable store: `~/.omega/stores/`.

## File Conventions

- Config: `config.toml` (gitignored), `config.example.toml` (committed)
- Database: `~/.omega/data/memory.db`
- Prompts (bundled): `prompts/SYSTEM_PROMPT.md`, `prompts/WELCOME.toml`, `prompts/WORKSPACE_CLAUDE.md`
- Prompts (runtime): `~/.omega/prompts/` (auto-deployed on first run)
- Skills: `~/.omega/skills/*/SKILL.md`
- Projects: `~/.omega/projects/*/ROLE.md`
- Workspace: `~/.omega/workspace/` (AI subprocess working directory)
- Builds: `~/.omega/workspace/builds/<project-name>/`
- Stores: `~/.omega/stores/` (domain-specific databases)
- Heartbeat: `~/.omega/prompts/HEARTBEAT.md` (global), `~/.omega/projects/<name>/HEARTBEAT.md` (per-project)
- Logs: `~/.omega/logs/omega.log`
- Service (macOS): `~/Library/LaunchAgents/com.omega-cortex.omega.plist`
- Service (Linux): `~/.config/systemd/user/omega.service`

## Providers

| Provider | Auth | Notes |
|----------|------|-------|
| `claude-code` (default) | CLI subprocess | `claude -p --output-format json --model <model>`, auto-resume on max_turns |
| `ollama` | None | Local server |
| `openai` | Bearer token | Also works with OpenAI-compatible endpoints |
| `anthropic` | `x-api-key` header | System prompt as top-level field |
| `openrouter` | Bearer token | Reuses OpenAI types |
| `gemini` | URL query param | Role mapping: assistant→model |

`build_provider()` returns `(Box<dyn Provider>, model_fast, model_complex)`. Claude Code: fast=Sonnet, complex=Opus. HTTP providers: both set to the configured model. See `docs/providers-claude-code.md` for CLI details.

## Documentation

Always consult these before modifying or extending the codebase:

- **`specs/SPECS.md`** — Master index of technical specifications for every file
- **`docs/DOCS.md`** — Master index of developer-facing guides and references
- **`docs/architecture.md`** — Full system design, gateway pipeline, and feature details
