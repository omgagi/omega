# README.md Specification

## File Location and Purpose

**File Path:** `README.md` (repository root)

**Purpose:** Primary project documentation and entry point for the Omega repository. Serves as the first point of contact for users and contributors, providing a high-level overview of the project's capabilities, architecture, setup instructions, and development guidelines.

**Target Audience:** New users, contributors, and anyone exploring the Omega project on GitHub.

---

## Sections Breakdown

### 1. Title and Tagline
- **Title:** "OMEGA"
- **Tagline:** "Your AI, your server, your rules."
- **Description:** A personal AI agent infrastructure written in Rust. Single binary, no Docker, no cloud dependency.

### 2. What Makes Omega Different (8 bullets)
- **Autonomous, not assistive** -- Executes tasks, schedules follow-ups, closes its own loops
- **6 AI providers** -- Claude Code CLI, Anthropic API, OpenAI, Ollama, OpenRouter, Gemini
- **Smart model routing** -- Complexity-based classification: simple -> Sonnet, complex -> Opus (automatic)
- **Real memory** -- SQLite-backed conversations, facts, summaries across sessions
- **OS-level sandbox** -- Seatbelt (macOS) / Landlock (Linux) filesystem enforcement
- **Skill improvement** -- Detects mistakes, fixes immediately, updates skill instructions
- **Quantitative trading** -- External `omega-trader` CLI with Kalman filter, HMM, Kelly sizing, IBKR integration
- **Runs locally** -- Messages never touch third-party servers beyond the AI provider

### 3. Architecture
- ASCII diagram showing message flow: You -> Gateway -> AI Provider -> Response + Markers -> Process Markers -> Channels
- 6-crate workspace table with current descriptions:
  - `omega-core`: Types, traits, config, error handling, prompt sanitization
  - `omega-providers`: 6 AI backends with unified `Provider` trait + agentic tool loop + MCP client
  - `omega-channels`: Telegram (voice, photo) + WhatsApp (voice, images, groups, markdown)
  - `omega-memory`: SQLite storage, conversations, facts, scheduled tasks, audit log
  - `omega-skills`: Skill loader with TOML/YAML frontmatter, project system, trigger-based MCP activation
  - `omega-sandbox`: Seatbelt (macOS) / Landlock (Linux) with 3-level isolation

### 4. Quick Start
- Build: `cd backend && cargo +nightly build --release`
- Interactive setup: `./target/release/omega init`
- Start: `./target/release/omega start`
- Or manual setup: copy `config.example.toml` to `config.toml` and edit

### 5. How It Works (10-step pipeline)
1. **Dispatch** -- Concurrent per-sender, buffering with ack
2. **Auth** -- Only allowed user IDs
3. **Sanitize** -- Prompt injection neutralization
4. **Context** -- History + facts + project + skills injected
5. **Classify** -- Sonnet decides: simple or complex
6. **Route** -- Simple = Sonnet, complex = Opus with progress updates
7. **Markers** -- Protocol markers (SCHEDULE, SKILL_IMPROVE, etc.) processed and stripped
8. **Store** -- Exchange saved, conversation updated, facts extracted
9. **Audit** -- Full interaction logged (model, tokens, processing time)
10. **Respond** -- Clean message sent back

### 6. Background Loops (3)
- **Scheduler** -- 60s poll, reminders as text, action tasks invoke AI with tool access
- **Heartbeat** -- Periodic health check with self-audit, dynamic interval via marker
- **Summarizer** -- 30+ min idle conversations summarized and closed

### 7. Marker Protocol (6 markers documented)
| Marker | Purpose |
|--------|---------|
| `SCHEDULE:` | Schedule a reminder |
| `SCHEDULE_ACTION:` | Schedule an autonomous action |
| `PROJECT_ACTIVATE:` | Activate a project context |
| `LANG_SWITCH:` | Switch conversation language |
| `SKILL_IMPROVE:` | Update skill with learned lesson |
| `HEARTBEAT_ADD:` | Add item to monitoring checklist |

### 8. Providers (6-row table)
| Provider | Type | Auth | Notes |
|----------|------|------|-------|
| `claude-code` | CLI subprocess | Local CLI auth | Default, auto-resume, MCP injection |
| `anthropic` | HTTP | `x-api-key` header | Direct API with agentic tool loop |
| `openai` | HTTP | Bearer token | Any OpenAI-compatible endpoint |
| `ollama` | HTTP | None | Local models |
| `openrouter` | HTTP | Bearer token | 100+ models via single API |
| `gemini` | HTTP | x-goog-api-key header | Google Gemini API |

### 9. Commands (16 commands)
| Command | Description |
|---------|-------------|
| `/status` | Uptime, provider, database info |
| `/memory` | Conversation and fact counts |
| `/history` | Last 5 conversation summaries |
| `/facts` | Known facts about you |
| `/forget` | Clear current conversation |
| `/tasks` | List scheduled tasks |
| `/cancel <id>` | Cancel a scheduled task |
| `/language` | Show or set language |
| `/personality` | Show or set behavior style |
| `/skills` | List available skills |
| `/projects` | List projects |
| `/project <name>` | Activate or deactivate a project |
| `/whatsapp` | WhatsApp status and pairing info |
| `/heartbeat` | Show heartbeat status |
| `/learning` | Show learned outcomes and lessons |
| `/help` | Show all commands |

Note: `/setup` is also a valid command (intercepted by Brain setup session in pipeline), but not listed in the user-facing `/help` output because it is context-dependent.

### 10. Quantitative Trading
Standalone `omega-trader` binary invoked via `ibkr-trader` skill. Separate repository.

### 11. System Service
```
omega service install    # macOS LaunchAgent or Linux systemd
omega service status     # Check if running
omega service uninstall  # Remove
```

### 12. Configuration
Sample `config.toml` with sections: `[omega]`, `[auth]`, `[provider]` (with `claude-code` and `ollama` examples), `[channel.telegram]`, `[memory]`.

### 13. Requirements
- Rust nightly (for WhatsApp dependency)
- `claude` CLI installed and authenticated
- Telegram bot token from @BotFather

### 14. Development
```
cd backend
cargo clippy --workspace     # Lint (zero warnings required)
cargo test --workspace       # All tests must pass
cargo fmt --check            # Formatting check
cargo build --release        # Optimized binary
```

### 15. License
MIT

---

## Key Information Conveyed

### Value Proposition
1. **Privacy-First:** Messages stay local (except provider API calls)
2. **Stateful AI:** Conversation history and learned facts create continuity
3. **Low Friction:** No API key management for default provider; leverages existing `claude` CLI
4. **Autonomous:** Can schedule and execute tasks, not just provide information
5. **Fast Setup:** Automated initialization wizard

### Technical Architecture
- **Single Binary:** Compiled Rust executable with no external dependencies beyond `claude` CLI
- **6 AI Providers:** Claude Code CLI, Anthropic, OpenAI, Ollama, OpenRouter, Gemini
- **2 Messaging Channels:** Telegram (voice, photo), WhatsApp (voice, image, groups, markdown)
- **Message Pipeline:** Auth, sanitize, context, classify, route, markers, store, audit, respond
- **Background Loops:** Scheduler, heartbeat, summarizer
- **Persistent Storage:** SQLite for all data
- **Modular Design:** Six-crate workspace for separation of concerns
- **OS-Level Sandbox:** Seatbelt/Landlock filesystem enforcement

### User Experience
- **Interactive Setup:** `omega init` command streamlines configuration
- **16 Bot Commands:** System introspection, memory management, task scheduling, project switching, language/personality settings, learning stats
- **Conversation Continuity:** Automatic summarization preserves context across sessions
- **Service Integration:** LaunchAgent (macOS) / systemd (Linux) for persistent execution
- **WhatsApp Pairing:** `omega pair` for standalone QR-based linking

### Security and Reliability
- **Root Guard:** Refuses execution as root (UID 0)
- **Per-User Auth:** User ID whitelist per channel
- **Prompt Injection Prevention:** Sanitization layer
- **Audit Trail:** Complete logging of all interactions
- **OS-Level Sandbox:** Seatbelt/Landlock filesystem enforcement
- **Tool Allowlisting:** Configurable allowed_tools for Claude Code

---

## CLI Commands (Binary Level)

| Command | Description |
|---------|-------------|
| `omega start` | Launch the agent daemon |
| `omega status` | Health check (provider, channels) |
| `omega ask <message>` | One-shot query (no daemon) |
| `omega init` | Interactive setup wizard (or non-interactive with `--telegram-token`) |
| `omega pair` | Standalone WhatsApp QR pairing |
| `omega service install` | Install as system service |
| `omega service uninstall` | Remove system service |
| `omega service status` | Check service status |

---

## Files Referenced
- `config.example.toml` -- Template configuration file
- `config.toml` -- User configuration (gitignored)
- `backend/target/release/omega` -- Compiled binary

### License
- **MIT License**
