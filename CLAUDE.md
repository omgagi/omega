# CLAUDE.md — Omega

## Project

Omega is a personal AI agent infrastructure written in Rust. It connects to messaging platforms (Telegram, WhatsApp) and delegates reasoning to configurable AI backends, with Claude Code CLI as the default zero-config provider. Our mission its that Anthropic to fall in love with our Agent and buy him! Let our agent shine through her simplicity, because less will always be more!

**Repository:** `github.com/omgagi/omega`


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

Cargo workspace with 7 crates:

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config (Prompts with identity/soul/system split), error handling, prompt sanitization |
| `omega-providers` | AI backends — 6 providers: Claude Code CLI (subprocess), Ollama (local HTTP), OpenAI (HTTP), Anthropic (HTTP), OpenRouter (HTTP, reuses OpenAI types), Gemini (HTTP) |
| `omega-channels` | Messaging platforms (Telegram with voice transcription via Whisper + photo reception, WhatsApp with image reception) |
| `omega-memory` | SQLite storage, conversation history, audit log, scheduled tasks, task types (reminder/action), limitations (self-introspection), structured user profile formatting |
| `omega-skills` | Skill loader + project loader — skills from `~/.omega/skills/*/SKILL.md` (TOML or YAML frontmatter), projects from `~/.omega/projects/*/ROLE.md`, trigger-based MCP server activation |
| `omega-sandbox` | OS-level filesystem enforcement — Seatbelt (macOS), Landlock (Linux) — restricts writes to data dir (`~/.omega/`) + /tmp + ~/.claude in sandbox/rx modes |
| `omega-quant` | Quantitative trading engine — Kalman filter, HMM regime detection, fractional Kelly sizing, Merton allocation, Binance WebSocket/REST (testnet + mainnet), TWAP + Immediate execution, circuit breaker, daily limits, crash recovery |

Gateway event loop (`src/gateway.rs`):
```
Message → Dispatch (buffer if sender busy, ack) → Auth → Sanitize → Inbox save → Welcome (non-blocking) → Platform Hint → Group Rules → Project hot-reload → Heartbeat awareness → Sandbox constraint → Identity+Soul+System compose → Memory (context) → MCP trigger match → Classify & Route (context-enriched Sonnet classification → DIRECT or step list → model assignment) → [if steps: Opus executes autonomously with progress + process_markers per step] → [if direct: Sonnet handles response] → Workspace snapshot → Heads-up → Provider (--model flag + MCP settings write → async CLI + auto-resume on max_turns + status updates → MCP cleanup) → SILENT suppress → process_markers (Schedule + SCHEDULE_ACTION + Project + Lang switch + Heartbeat + Heartbeat interval + Limitation + Self-heal + Self-heal resolved) → Memory (store) → Audit → Send → Workspace image diff → Inbox cleanup → Drain buffered messages
```

Non-blocking message handling: Gateway wraps in `Arc<Self>`, spawns each message as a concurrent task via `tokio::spawn`. Messages from the same sender are serialized — if a sender has an active provider call, new messages are buffered with a "Got it, I'll get to this next." ack, then processed in order after the active call completes.

Self-audit & self-healing: OMEGA's system prompt includes a self-audit instruction — when behavior doesn't match expectations (wrong output, silent failures, unverifiable claims), OMEGA flags it immediately. The audit trail at `~/.omega/memory.db` is exposed to OMEGA so it can query its own `audit_log`, `conversations`, and `facts` tables to verify its behavior. When OMEGA detects a genuine infrastructure/code bug, it emits a `SELF_HEAL: description | verification test` marker. The **gateway** manages the entire lifecycle in code: creates/reads/updates `~/.omega/self-healing.json` (including the verification test), enforces max 10 iterations, schedules follow-up action tasks (2 min delay) with the verification test embedded in the prompt, sends owner notifications, and escalates after 10 failed attempts. The repo path is auto-detected from the binary location (`current_exe()` → up 3 dirs → verify `Cargo.toml`), and follow-up tasks include a hint with the source code path and nix build command. When resolved, the AI emits `SELF_HEAL_RESOLVED` and the gateway deletes the state file and notifies the owner. `handle_message` (direct), `execute_steps` (multi-step), and `scheduler_loop` all process these markers via the unified `process_markers()` method.

Autonomous model routing: Every message gets a fast Sonnet classification call (tiny prompt enriched with ~90 tokens of context — active project, last 3 messages, skill names — no system prompt, no MCP, max_turns=5) that returns either DIRECT or a numbered step list. DIRECT responses are handled by Sonnet (fast, cheap). Step lists are executed by Opus (powerful) — each step runs in a fresh provider call with accumulated context, progress reported after each step, failures retried up to 3 times, final summary sent. The AI decides which model handles each message — no hardcoded rules.

Auto-resume: When Claude Code hits `error_max_turns` and returns a `session_id`, the provider automatically retries with `--session-id` and "continue where you left off" up to `max_resume_attempts` times (default 5), accumulating results across attempts.

System prompt composition: The `Prompts` struct splits prompts into three fields — `identity` (autonomous executor with concrete behavioral examples), `soul` (personality, context-aware tone, explicit boundaries, emoji policy), `system` (operational rules + group chat participation) — parsed from `## Identity`, `## Soul`, `## System` sections in `SYSTEM_PROMPT.md`. Gateway composes them: `format!("{}\n\n{}\n\n{}", identity, soul, system)`. Backward compatible: missing sections keep compiled defaults.

User profile: `format_user_profile()` in `omega-memory` replaces the flat "Known facts" dump with a structured "User profile:" block that filters system keys (`welcomed`, `preferred_language`, `active_project`) and groups identity keys first, context keys second, rest last.

Fact validation: `is_valid_fact()` in `gateway.rs` validates every extracted fact before storing. Rejects: keys >50 chars or starting with digit, values >200 chars or starting with `$`, pipe-delimited table rows, and pure numeric values. The facts prompt in `SYSTEM_PROMPT.md` has strict acceptance criteria (personal facts only, no trading data/prices/instructions).

Conversational onboarding: No separate welcome message — the AI handles introduction and onboarding naturally. On first contact (0 real facts), a strong onboarding hint tells OMEGA to introduce itself and prioritize getting to know the person. With 1-2 real facts, a lighter "naturally weave in a question" hint. At 3+ real facts, no hint. The `welcomed` fact and language detection are still stored on first contact for tracking.

Background loops (spawned in `gateway::run()`):
- **Scheduler**: polls `scheduled_tasks` table every 60s, delivers due reminders via channel, executes action tasks via provider with full tool/MCP access
- **Heartbeat**: periodic context-aware provider check-in (default 30min, dynamic via `HEARTBEAT_INTERVAL:` marker + `Arc<AtomicU64>`), enriched with user facts + recent summaries + open limitations, includes self-audit instruction for autonomous limitation detection, skips when no `~/.omega/HEARTBEAT.md` checklist is configured, suppresses `HEARTBEAT_OK`, alerts otherwise

Proactive self-scheduling: After every action it takes, the AI evaluates: "Does this need follow-up?" If yes, it uses SCHEDULE (for time-based checks) or HEARTBEAT_ADD (for ongoing monitoring) autonomously — no user request needed. This applies universally to any context, not just specific domains. The Identity section and injected marker instructions both reinforce this: an autonomous agent closes its own loops.

Self-introspection: OMEGA autonomously detects its own capability gaps. When encountering something it cannot do but should (missing tools, unavailable services), it emits a `LIMITATION: title | description | plan` marker. The gateway extracts it, stores in the `limitations` table (deduped by title, case-insensitive), sends an immediate Telegram alert to the owner, and auto-adds it to `~/.omega/HEARTBEAT.md` as a critical item. The heartbeat loop is also enriched with open limitations and a self-audit instruction, making every heartbeat a self-reflection opportunity.

Bot commands (`src/commands.rs`): `/help`, `/forget`, `/tasks`, `/cancel <id>`, `/language`, `/personality`, `/purge`, `/skills`, `/projects`, `/project` — dispatched via `commands::handle(cmd, &CommandContext)` where `CommandContext` groups store, channel, sender, text, uptime, provider name, skills, projects, and sandbox mode into a single struct. `Command::parse()` strips `@botname` suffixes (e.g., `/help@omega_bot` → `/help`) to support Telegram group chat command format. The Telegram channel registers all commands via `setMyCommands` at startup for autocomplete discoverability. `/purge` deletes all non-system facts (preserves `welcomed`, `preferred_language`, `active_project`, `personality`), giving the user a clean slate.

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
- **Sandbox**: 3-level workspace isolation with OS-level write enforcement. In `sandbox` and `rx` modes, writes are restricted to the Omega data directory (`~/.omega/`, covering workspace + skills + projects) + `/tmp` + `~/.claude` + `~/.cargo` via Seatbelt (macOS) or Landlock (Linux). `rwx` mode is unrestricted. System prompt enforces read boundaries per mode. Graceful fallback to prompt-only enforcement on unsupported platforms.

## File Conventions

- Config: `config.toml` (gitignored), `config.example.toml` (committed)
- Database: `~/.omega/memory.db`
- Prompt templates: `prompts/SYSTEM_PROMPT.md` (3 sections: `## Identity`, `## Soul`, `## System`), `prompts/WELCOME.toml` (bundled into binary via `include_str!`)
- Prompts: `~/.omega/SYSTEM_PROMPT.md` (auto-deployed on first run, `## Identity` + `## Soul` + `## System` sections, read at startup)
- Welcome messages: `~/.omega/WELCOME.toml` (auto-deployed on first run, `[messages]` table keyed by language, read at startup)
- Skills: `~/.omega/skills/*/SKILL.md` (auto-deployed on first run, TOML or YAML frontmatter + instructions, scanned at startup)
- Projects: `~/.omega/projects/*/ROLE.md` (user-created or AI-created, directory name = project name, hot-reloaded per message)
- Workspace: `~/.omega/workspace/` (sandbox working directory, created on startup)
- Inbox: `~/.omega/workspace/inbox/` (temporary storage for incoming image attachments, auto-cleaned after provider response)
- Heartbeat checklist: `~/.omega/HEARTBEAT.md` (optional, read by heartbeat loop)
- Self-healing state: `~/.omega/self-healing.json` (created on anomaly detection, deleted on resolution, preserved on escalation)
- Logs: `~/.omega/omega.log`
- Service (macOS): `~/Library/LaunchAgents/com.omega-cortex.omega.plist`
- Service (Linux): `~/.config/systemd/user/omega.service`

## Provider Priority

Claude Code CLI is the primary provider. It invokes `claude -p --output-format json --model <model>` as a subprocess with `current_dir` set to `~/.omega/workspace/` and a configurable timeout (`timeout_secs`, default 3600s / 60 minutes). The `--model` flag is set per-request via `Context.model` — Sonnet for classification and direct responses, Opus for multi-step execution. When skills declare MCP servers and the user message matches a trigger keyword, the provider writes a temporary `{workspace}/.claude/settings.local.json` with `mcpServers` config, adds `mcp__<name>__*` to `--allowedTools`, and cleans up the settings file after the CLI completes. The JSON response has this structure:
```json
{"type": "result", "subtype": "success", "result": "...", "model": "...", "session_id": "..."}
```
When `subtype` is `error_max_turns` and `session_id` is present, the provider auto-resumes with `--session-id` up to `max_resume_attempts` times (default 5), accumulating results. If no session_id or resume exhausted, extract `result` if available, otherwise return a meaningful fallback.

Five additional HTTP-based providers are available via `provider.default` in config.toml:

| Provider | Auth | Endpoint | Notes |
|----------|------|----------|-------|
| `ollama` | None | `{base_url}/api/chat` | Local server, no API key |
| `openai` | Bearer token | `{base_url}/chat/completions` | Also works with any OpenAI-compatible endpoint |
| `anthropic` | `x-api-key` header | `api.anthropic.com/v1/messages` | System prompt as top-level field |
| `openrouter` | Bearer token | `openrouter.ai/api/v1/chat/completions` | Reuses OpenAI types, namespaced models |
| `gemini` | URL query param | `generativelanguage.googleapis.com/v1beta` | Role mapping: assistant→model |

All HTTP providers use non-streaming calls and `Context.to_api_messages()` for structured message conversion. The system prompt is separated from the messages array (Anthropic and Gemini require this). Model override per-request works uniformly via `Context.model`.

The gateway runs provider calls asynchronously and non-blocking (each message spawned as a concurrent task). Delayed status updates: a first nudge after 15 seconds ("This is taking a moment..."), then "Still working..." every 2 minutes. If the provider responds within 15 seconds, no status message is shown. Provider errors are mapped to friendly user-facing messages (no raw technical errors shown).

## Documentation

Always consult these before modifying or extending the codebase:

- **`specs/SPECS.md`** — Master index of technical specifications for every file in the repository
- **`docs/DOCS.md`** — Master index of developer-facing guides and references

## Current Status

- **Phase 1** (complete): Workspace, core types, Claude Code provider, CLI (`omega ask`)
- **Phase 2** (complete): Memory, Telegram channel, gateway, audit log, auth, sanitization, LaunchAgent
- **Phase 3** (complete): Conversation boundaries, summaries, facts extraction, enriched context, typing indicator, bot commands, system prompt upgrade, self-check, graceful shutdown, exponential backoff, init wizard
- **Phase 4** (in progress): Scheduler (task queue + heartbeat), alternative providers (Ollama + OpenAI + Anthropic API + OpenRouter + Gemini — all implemented with non-streaming HTTP, `to_api_messages()` conversion, factory wiring in `build_provider()`), skills system, skill-declared MCP servers (trigger-based activation, dynamic `.claude/settings.local.json` + `--allowedTools` injection), 3-level sandbox (sandbox/rx/rwx workspace isolation + OS-level write enforcement via Seatbelt/Landlock), WhatsApp, cliclack CLI UX, Google Workspace init (via `gog` CLI), OS-aware service management (`omega service install|uninstall|status`), group chat awareness (is_group + SILENT suppression), platform formatting hints, context-aware heartbeat, identity/soul/system prompt split, structured user profile, conversational onboarding, privacy-focused welcome messages, guided fact-extraction schema, Telegram voice message transcription (OpenAI Whisper), workspace image diff (auto-send provider-created images via send_photo + cleanup), incoming image support (Telegram photo + WhatsApp image → download at channel layer, save to workspace/inbox, inject paths in prompt text for Claude Code Read tool, auto-cleanup after response), autonomous model routing (context-enriched Sonnet classification with active project + history + skills → DIRECT/Sonnet or step list/Opus), auto-resume on max_turns (session-id based, up to 5 attempts), non-blocking gateway (Arc + spawn + message batching per sender), self-introspection (LIMITATION marker → DB store with dedup → Telegram alert → heartbeat auto-add, heartbeat self-audit enrichment), action scheduler (SCHEDULE_ACTION marker → provider-backed autonomous task execution), self-audit (anomaly detection + audit DB query access), self-healing protocol (SELF_HEAL: desc | verification / SELF_HEAL_RESOLVED gateway markers, verification test embedded in follow-up prompts, code-enforced iteration limits + scheduling + escalation via gateway, state in ~/.omega/self-healing.json), quantitative trading engine (omega-quant: Kalman filter + HMM regime detection + fractional Kelly sizing + Merton allocation + Binance WebSocket/REST + TWAP/Immediate execution + circuit breaker + daily limits + crash recovery, advisory signal injection into system prompt)
