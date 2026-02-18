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

    After any development for the Rust parts, run cargo build with nix to ensure it compiles, then cargo clippy to clean up any lint errors. Release the binary, stop and restart my service.

2. **Pre-Commit Gate** (Execute in order, all steps mandatory):

   | Step | Action | Condition |
   |------|--------|-----------|
   | 1 | **Update `specs/`** | **Always** when adding or modifying any functionality — technical specs must reflect current behavior |
   | 2 | **Update `docs/`** | **Always** when adding or modifying any functionality — user-facing docs must match the code |
   | 3 | **Update `CLAUDE.md`** | **Always** when adding or modifying any functionality — architecture and feature list must stay current |
   | 4 | **Verify build** | `cargo build && cargo clippy -- -D warnings && cargo fmt --check` |
   | 5 | **Verify tests** | `cargo test` |
   | 6 | **Commit** | Only after steps 1-5 pass |

   **Commit command** (only after all steps pass):
```bash
   git add -A && git commit -m "<type>(<scope>): <description>"
```

3. **Feature Testing**: Every new or modified functionality **MUST** include a test that verifies it works as expected. No feature is considered complete without a passing test. This applies to:
   - New functions or methods → unit test
   - New API endpoints or bot commands → integration test
   - Bug fixes → regression test that reproduces the bug and confirms the fix
   - Changed behavior → updated existing tests to match new expectations

4. **Output Filtering**: Always filter verbose output:
Apply always outour redirection to a /tmp/ folder to avoid polluting the console to later apply filters.
  command > /tmp/cmd_output.log 2>&1 && grep -iE "error|warn|fail|pass" /tmp/cmd_output.log | head -20

## Architecture

Cargo workspace with 6 crates:

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config (Prompts with identity/soul/system split), error handling, prompt sanitization |
| `omega-providers` | AI backends (Claude Code CLI, Anthropic, OpenAI, Ollama, OpenRouter) |
| `omega-channels` | Messaging platforms (Telegram with voice transcription via Whisper + photo reception, WhatsApp with image reception) |
| `omega-memory` | SQLite storage, conversation history, audit log, scheduled tasks, structured user profile formatting |
| `omega-skills` | Skill loader + project loader — skills from `~/.omega/skills/*/SKILL.md` (TOML or YAML frontmatter), projects from `~/.omega/projects/*/INSTRUCTIONS.md`, trigger-based MCP server activation |
| `omega-sandbox` | OS-level filesystem enforcement — Seatbelt (macOS), Landlock (Linux) — restricts writes to data dir (`~/.omega/`) + /tmp + ~/.claude in sandbox/rx modes |

Gateway event loop (`src/gateway.rs`):
```
Message → Dispatch (buffer if sender busy, ack) → Auth → Sanitize → Inbox save → Welcome (non-blocking) → Platform Hint → Group Rules → Heartbeat awareness → Sandbox constraint → Identity+Soul+System compose → Memory (context) → MCP trigger match → Pre-flight planning (>15 words → dedicated planning call → DIRECT or step list) → [if steps: autonomous execution with progress] → Workspace snapshot → Heads-up → Provider (MCP settings write → async CLI + auto-resume on max_turns + status updates → MCP cleanup) → SILENT suppress → Schedule extract → Lang switch → Heartbeat add/remove → Memory (store) → Audit → Send → Workspace image diff → Inbox cleanup → Drain buffered messages
```

Non-blocking message handling: Gateway wraps in `Arc<Self>`, spawns each message as a concurrent task via `tokio::spawn`. Messages from the same sender are serialized — if a sender has an active provider call, new messages are buffered with a "Got it, I'll get to this next." ack, then processed in order after the active call completes.

Pre-flight planning: For messages with >15 words, Omega sends a dedicated planning call (tiny prompt, no history, no MCP) that forces a DIRECT or numbered step list response. If steps are returned, Omega executes each autonomously in a fresh provider call with accumulated context, reports progress after each step, retries failures up to 3 times, and sends a final summary. Messages with ≤15 words skip planning entirely — greetings and quick questions are answered instantly with zero overhead.

Auto-resume: When Claude Code hits `error_max_turns` and returns a `session_id`, the provider automatically retries with `--session-id` and "continue where you left off" up to `max_resume_attempts` times (default 5), accumulating results across attempts.

System prompt composition: The `Prompts` struct splits prompts into three fields — `identity` (autonomous executor with concrete behavioral examples), `soul` (personality, context-aware tone, explicit boundaries, emoji policy), `system` (operational rules + group chat participation) — parsed from `## Identity`, `## Soul`, `## System` sections in `SYSTEM_PROMPT.md`. Gateway composes them: `format!("{}\n\n{}\n\n{}", identity, soul, system)`. Backward compatible: missing sections keep compiled defaults.

User profile: `format_user_profile()` in `omega-memory` replaces the flat "Known facts" dump with a structured "User profile:" block that filters system keys (`welcomed`, `preferred_language`, `active_project`) and groups identity keys first, context keys second, rest last.

Conversational onboarding: No separate welcome message — the AI handles introduction and onboarding naturally. On first contact (0 real facts), a strong onboarding hint tells OMEGA to introduce itself and prioritize getting to know the person. With 1-2 real facts, a lighter "naturally weave in a question" hint. At 3+ real facts, no hint. The `welcomed` fact and language detection are still stored on first contact for tracking.

Background loops (spawned in `gateway::run()`):
- **Scheduler**: polls `scheduled_tasks` table every 60s, delivers due reminders via channel
- **Heartbeat**: periodic context-aware provider check-in (default 30min), enriched with user facts + recent summaries, skips when no `~/.omega/HEARTBEAT.md` checklist is configured, suppresses `HEARTBEAT_OK`, alerts otherwise

Bot commands (`src/commands.rs`): `/help`, `/forget`, `/tasks`, `/cancel <id>`, `/language`, `/personality`, `/skills`, `/projects`, `/project` — dispatched via `commands::handle(cmd, &CommandContext)` where `CommandContext` groups store, channel, sender, text, uptime, provider name, skills, projects, and sandbox mode into a single struct.

Init wizard Google Workspace: auto-detects installed browsers with incognito/private mode (Chrome, Brave, Firefox, Edge), offers to open OAuth URL in incognito via `BROWSER` env var on the `gog auth add` subprocess, cleans up temp script after.

CLI commands: `start`, `status`, `ask`, `init`, `service install|uninstall|status`

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
- **CLI UX uses `cliclack`** — init wizard, self-check, and status command use `cliclack` (styled │ ◆ ◇ prompts) and `console` (terminal styling). No plain `println!` for interactive CLI flows.
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
- **Sandbox**: 3-level workspace isolation with OS-level write enforcement. In `sandbox` and `rx` modes, writes are restricted to the Omega data directory (`~/.omega/`, covering workspace + skills + projects) + `/tmp` + `~/.claude` via Seatbelt (macOS) or Landlock (Linux). `rwx` mode is unrestricted. System prompt enforces read boundaries per mode. Graceful fallback to prompt-only enforcement on unsupported platforms.

## File Conventions

- Config: `config.toml` (gitignored), `config.example.toml` (committed)
- Database: `~/.omega/memory.db`
- Prompt templates: `prompts/SYSTEM_PROMPT.md` (3 sections: `## Identity`, `## Soul`, `## System`), `prompts/WELCOME.toml` (bundled into binary via `include_str!`)
- Prompts: `~/.omega/SYSTEM_PROMPT.md` (auto-deployed on first run, `## Identity` + `## Soul` + `## System` sections, read at startup)
- Welcome messages: `~/.omega/WELCOME.toml` (auto-deployed on first run, `[messages]` table keyed by language, read at startup)
- Skills: `~/.omega/skills/*/SKILL.md` (auto-deployed on first run, TOML or YAML frontmatter + instructions, scanned at startup)
- Projects: `~/.omega/projects/*/INSTRUCTIONS.md` (user-created, directory name = project name, scanned at startup)
- Workspace: `~/.omega/workspace/` (sandbox working directory, created on startup)
- Inbox: `~/.omega/workspace/inbox/` (temporary storage for incoming image attachments, auto-cleaned after provider response)
- Heartbeat checklist: `~/.omega/HEARTBEAT.md` (optional, read by heartbeat loop)
- Logs: `~/.omega/omega.log`
- Service (macOS): `~/Library/LaunchAgents/com.omega-cortex.omega.plist`
- Service (Linux): `~/.config/systemd/user/omega.service`

## Provider Priority

Claude Code CLI is the primary provider. It invokes `claude -p --output-format json` as a subprocess with `current_dir` set to `~/.omega/workspace/` and a configurable timeout (`timeout_secs`, default 3600s / 60 minutes). When skills declare MCP servers and the user message matches a trigger keyword, the provider writes a temporary `{workspace}/.claude/settings.local.json` with `mcpServers` config, adds `mcp__<name>__*` to `--allowedTools`, and cleans up the settings file after the CLI completes. The JSON response has this structure:
```json
{"type": "result", "subtype": "success", "result": "...", "model": "...", "session_id": "..."}
```
When `subtype` is `error_max_turns` and `session_id` is present, the provider auto-resumes with `--session-id` up to `max_resume_attempts` times (default 5), accumulating results. If no session_id or resume exhausted, extract `result` if available, otherwise return a meaningful fallback.

The gateway runs provider calls asynchronously and non-blocking (each message spawned as a concurrent task). Delayed status updates: a first nudge after 15 seconds ("This is taking a moment..."), then "Still working..." every 2 minutes. If the provider responds within 15 seconds, no status message is shown. Provider errors are mapped to friendly user-facing messages (no raw technical errors shown).

## Documentation

Always consult these before modifying or extending the codebase:

- **`specs/SPECS.md`** — Master index of technical specifications for every file in the repository
- **`docs/DOCS.md`** — Master index of developer-facing guides and references

## Current Status

- **Phase 1** (complete): Workspace, core types, Claude Code provider, CLI (`omega ask`)
- **Phase 2** (complete): Memory, Telegram channel, gateway, audit log, auth, sanitization, LaunchAgent
- **Phase 3** (complete): Conversation boundaries, summaries, facts extraction, enriched context, typing indicator, bot commands, system prompt upgrade, self-check, graceful shutdown, exponential backoff, init wizard
- **Phase 4** (in progress): Scheduler (task queue + heartbeat), alternative providers, skills system, skill-declared MCP servers (trigger-based activation, dynamic `.claude/settings.local.json` + `--allowedTools` injection), 3-level sandbox (sandbox/rx/rwx workspace isolation + OS-level write enforcement via Seatbelt/Landlock), WhatsApp, cliclack CLI UX, Google Workspace init (via `gog` CLI), OS-aware service management (`omega service install|uninstall|status`), group chat awareness (is_group + SILENT suppression), platform formatting hints, context-aware heartbeat, identity/soul/system prompt split, structured user profile, conversational onboarding, privacy-focused welcome messages, guided fact-extraction schema, Telegram voice message transcription (OpenAI Whisper), workspace image diff (auto-send provider-created images via send_photo + cleanup), incoming image support (Telegram photo + WhatsApp image → download at channel layer, save to workspace/inbox, inject paths in prompt text for Claude Code Read tool, auto-cleanup after response), pre-flight planning (dedicated planning call for >15-word messages → DIRECT or autonomous step execution with progress + retry), auto-resume on max_turns (session-id based, up to 5 attempts), non-blocking gateway (Arc + spawn + message batching per sender)
