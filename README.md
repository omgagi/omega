# OMEGA

**Your AI, your server, your rules.**

A personal AI agent infrastructure written in Rust. Single binary. No Docker. No cloud dependency. Omega connects to your messaging platforms, delegates reasoning to Claude Code, and acts autonomously on your behalf — scheduling tasks, building software, learning from mistakes, and closing its own loops.

```
curl -fsSL https://raw.githubusercontent.com/omgagi/omega/main/install.sh | bash
```

One command. Works on macOS and Linux (x86_64 and ARM64). Runs the setup wizard automatically.

---

## Why Omega

Most AI tools wait for you to ask. Omega doesn't.

**Autonomous, not assistive.** Omega executes tasks, schedules follow-ups, monitors your projects on a heartbeat, detects its own mistakes, and learns from them — permanently. It doesn't wait to be asked twice.

**Powered by Claude Code.** Omega uses Anthropic's Claude Code CLI as its AI engine — the same agentic tool developers use daily. Claude handles reasoning, tool execution, file operations, and multi-step workflows out of the box. No API keys to configure. Install `claude`, authenticate once, and Omega inherits the full power of Claude's agent capabilities.

**Runs on your machine.** Your messages never touch third-party servers beyond the AI provider. Your data stays in a local SQLite database. Your config stays on disk. No telemetry, no cloud sync, no account required.

---

## What It Does

### Smart Model Routing
Every message is classified by complexity in real-time. Simple tasks (greetings, reminders, lookups) are handled by Sonnet — fast and cheap. Complex work (multi-file code changes, deep research, sequential dependencies) is automatically decomposed into steps and executed by Opus with progress updates. No configuration needed.

### Real Memory
SQLite-backed conversations, user facts, summaries, and FTS5 semantic search. Omega remembers who you are, what you've told it, and what happened in past conversations — across sessions, across channels.

### Reward-Based Learning
Omega evaluates its own responses. Helpful? Stored as a positive outcome. Redundant or wrong? Stored as a negative outcome with a lesson. Lessons persist as long-term behavioral rules that shape future responses. The agent improves by using itself.

### Task Scheduling
Natural language scheduling with UTC-aware timezone conversion. Reminders are delivered as messages. Action tasks invoke the AI with full tool access — Omega executes autonomously and reports the result. Supports one-time, daily, weekly, monthly, and yearly recurrence. Quiet hours deferral. Duplicate detection.

### Multi-Agent Build Pipeline
Ask Omega to build something and it orchestrates a full software development pipeline: Analyst, Architect, Test Writer, Developer, QA, Reviewer, Delivery — seven specialized agents executing in sequence with file-mediated handoffs. QA failures loop back to the developer (max 3 rounds). Reviewer failures are fatal. The pipeline is defined in TOML, not code — swap agents, change retry limits, or reorder phases without touching Rust.

### Heartbeat Monitoring
Clock-aligned periodic check-ins. Define a checklist of things to monitor (servers, deployments, metrics, anything). Omega classifies items by domain, executes related groups in parallel via Opus, and only sends you alerts when something needs attention. Per-project heartbeats supported.

### Skill System
Extensible capabilities loaded from `~/.omega/skills/*/SKILL.md`. Each skill is a markdown file with TOML frontmatter that teaches the AI a new domain — trading, DevOps, data analysis, anything. Skills with MCP server definitions get automatic tool activation. The AI uses semantic intent matching to pick the right skill for each request.

### Project System
Domain contexts loaded from `~/.omega/projects/*/ROLE.md`. Each project gets its own session, its own heartbeat, its own lessons, and its own scheduled tasks. Switch between projects without losing context. Project-specific learning never leaks into general context, but general lessons always flow into project context.

### OS-Level Sandbox
Not just prompt-based protection. Omega enforces filesystem security at three layers:
1. **Code-level** — blocklist checks before any file operation
2. **OS-level** — Seatbelt (macOS) / Landlock (Linux) kernel enforcement
3. **Prompt-level** — workspace CLAUDE.md restricts AI behavior

Protected: `~/.omega/data/memory.db`, `~/.omega/config.toml`. The AI subprocess cannot read or write these files even if instructed to.

### Multi-Language
Full i18n for 8 languages: English, Spanish, Portuguese, French, German, Italian, Dutch, Russian. All commands, confirmations, error messages, and system prompts are localized.

### Multi-Channel
Talk to Omega from Telegram or WhatsApp. Voice messages are transcribed. Photos are processed. Cross-channel identity resolution links your accounts automatically via fuzzy name matching.

---

## Architecture

```
You (Telegram / WhatsApp)
        |
        v
  +-----------+     +----------------+     +-------------+
  |  Gateway   |---->|  Claude Code   |---->|   Response   |
  |            |     |  (CLI agent)   |     |   + Markers  |
  |  Auth      |     |                |     +------+------+
  |  Sanitize  |     +----------------+            |
  |  Classify  |                              +----v----+
  |  Route     |<-----------------------------| Process  |
  |  Audit     |      Memory (SQLite)         | Markers  |
  +-----------+      Facts, Summaries         +---------+
        |             Scheduled Tasks          SCHEDULE:
        v             Audit Log                REWARD:
  +----------+                                 LESSON:
  | Channels |                                 BUILD_PROPOSAL:
  | Telegram |                                 HEARTBEAT_ADD:
  | WhatsApp |                                 ...
  +----------+
```

Cargo workspace with 6 crates:

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization |
| `omega-providers` | Claude Code CLI + 5 additional backends (Anthropic, OpenAI, Ollama, OpenRouter, Gemini). Agentic tool loop + MCP client |
| `omega-channels` | Telegram (voice, photo) + WhatsApp (voice, image, groups, markdown) |
| `omega-memory` | SQLite storage — conversations, facts, summaries, scheduled tasks, sessions, outcomes, lessons, audit log |
| `omega-skills` | Skill loader + project loader + MCP server activation |
| `omega-sandbox` | Seatbelt (macOS) / Landlock (Linux) filesystem enforcement |

---

## How It Works

Every message flows through a deterministic 12-stage pipeline:

1. **Dispatch** — Concurrent per-sender. Same-sender messages are serialized; different users are fully parallel.
2. **Auth** — Only allowed user IDs pass through.
3. **Sanitize** — Prompt injection patterns neutralized (role tags, override phrases, zero-width bypasses).
4. **Identity** — Cross-channel user identity resolved. New users auto-detected with language detection.
5. **Context** — Conversation history + user facts + active project + skills injected into the system prompt.
6. **Keywords** — 9 keyword categories gate conditional prompt sections, reducing token usage by ~55-70%.
7. **Classify** — Sonnet decides: simple task = direct response, complex work = step-by-step plan.
8. **Route** — Simple tasks handled by Sonnet. Complex tasks decomposed and executed by Opus.
9. **Markers** — AI emits protocol markers that the gateway processes and strips before delivery.
10. **Store** — Exchange saved, conversation updated, facts extracted.
11. **Audit** — Full interaction logged with model, tokens, processing time.
12. **Respond** — Clean message delivered to the user.

### Token Efficiency

| Optimization | Savings |
|--------------|---------|
| Session persistence (`--resume`) | ~90-99% tokens on continuation messages |
| Keyword-gated prompt sections | ~55-70% fewer tokens per first message |
| Keyword-gated DB queries | Skip expensive queries when not relevant |
| Sonnet classification before Opus | Cheap routing, expensive model only when needed |

### Background Loops

- **Scheduler** — Polls every 60s. Reminders delivered as text; action tasks invoke the AI autonomously.
- **Heartbeat** — Clock-aligned periodic check-ins with parallel domain execution.
- **Summarizer** — Idle conversations auto-summarized with fact extraction.
- **CLAUDE.md maintenance** — Workspace context refreshed every 24h.

### Marker Protocol

The AI communicates structured commands through protocol markers embedded in response text. The gateway extracts, executes, and strips them before the user sees anything:

| Marker | Purpose |
|--------|---------|
| `SCHEDULE:` | Schedule a reminder |
| `SCHEDULE_ACTION:` | Schedule an autonomous action |
| `REWARD:` | Store a reward outcome |
| `LESSON:` | Store a learned behavioral rule |
| `PROJECT_ACTIVATE:` | Switch project context |
| `HEARTBEAT_ADD:` | Add monitoring item |
| `SKILL_IMPROVE:` | Update skill with learned lesson |
| `BUILD_PROPOSAL:` | Propose a software build |
| `BUG_REPORT:` | Log a self-detected bug |
| + 12 more | Task updates, language, personality, cleanup |

All markers include anti-hallucination verification against the actual database.

---

## Commands

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
| `/setup <desc>` | Create a new project interactively |
| `/heartbeat` | Show heartbeat status |
| `/learning` | Show reward outcomes and rules |
| `/token` | Show estimated context token usage |
| `/purge` | Purge all user facts |
| `/whatsapp` | Start WhatsApp QR pairing |
| `/help` | Show all commands |

---

## HTTP API

Lightweight axum server for dashboard integration:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/health` | GET | Health check with uptime, channel status |
| `/api/pair` | POST | Trigger WhatsApp pairing, returns QR as base64 PNG |
| `/api/webhook` | POST | Inbound message injection (direct or AI mode) |

Bearer token authentication with constant-time comparison.

---

## Installation

### One-Line Install

```bash
curl -fsSL https://raw.githubusercontent.com/omgagi/omega/main/install.sh | bash
```

Downloads the latest release binary for your platform, installs to `~/.local/bin/`, and runs the interactive setup wizard.

### Build from Source

```bash
git clone https://github.com/omgagi/omega
cd omega/backend
cargo +nightly build --release
./target/release/omega init
```

### System Service

```bash
omega service install    # macOS LaunchAgent or Linux systemd (auto-start, restart on crash)
omega service status     # Check if running
omega service uninstall  # Remove
```

---

## Configuration

`config.toml` is generated by `omega init`. Minimal example:

```toml
[omega]
name = "Omega"
data_dir = "~/.omega"

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 15

[channel.telegram]
enabled = true
bot_token = "YOUR_TOKEN"
allowed_users = [123456789]

[heartbeat]
enabled = true
interval_minutes = 60

[scheduler]
enabled = true
```

---

## Requirements

- macOS or Linux (x86_64 or ARM64)
- `claude` CLI installed and authenticated ([install guide](https://docs.anthropic.com/en/docs/claude-code/overview))
- Telegram bot token (from [@BotFather](https://t.me/BotFather)) and/or WhatsApp

---

## Codebase

- **154 functionalities** across 18 modules
- **6 library crates** + 1 binary crate
- **20 bot commands**, **21 protocol markers**, **9 keyword categories**
- **6 AI providers**, **2 messaging channels**, **8 languages**
- **13 database migrations**, **3 security layers**
- Full inventory: [`docs/functionalities/FUNCTIONALITIES.md`](docs/functionalities/FUNCTIONALITIES.md)

---

## License

MIT
