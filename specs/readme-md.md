# README.md Specification

## File Location and Purpose

**File Path:** `README.md` (repository root)

**Purpose:** Primary project documentation and entry point for the Omega repository. Serves as the first point of contact for users and contributors, providing a high-level overview of the project's capabilities, architecture, setup instructions, and development guidelines.

**Target Audience:** New users, contributors, Anthropic, and anyone exploring the Omega project on GitHub.

---

## Sections Breakdown

### 1. Title, Tagline, and One-Line Install
- **Title:** "OMEGA"
- **Tagline:** "Your AI, your server, your rules."
- **Description:** Personal AI agent infrastructure written in Rust. Single binary, no Docker, no cloud dependency.
- **One-line install:** `curl -fsSL ... | bash` — immediately actionable, before any explanation
- **Platform support:** macOS and Linux (x86_64 and ARM64)

### 2. Why Omega (3 pillars)
- **Autonomous, not assistive** — executes tasks, schedules follow-ups, monitors projects, learns from mistakes
- **Powered by Claude Code** — uses Anthropic's Claude Code CLI as the AI engine, no API keys to configure
- **Runs on your machine** — local SQLite, no telemetry, no cloud sync, no account required

### 3. What It Does (11 capability sections)
- **Smart Model Routing** — automatic complexity classification, Sonnet for simple, Opus for complex
- **Real Memory** — SQLite + FTS5 semantic search, cross-session persistence
- **Reward-Based Learning** — self-evaluation, outcome storage, long-term behavioral rules
- **Task Scheduling** — natural language, UTC-aware, reminders + autonomous action tasks, recurrence, quiet hours, duplicate detection
- **Multi-Agent Build Pipeline** — 7 agents in sequence, file-mediated handoffs, TOML-defined topology, bounded corrective loops
- **Heartbeat Monitoring** — clock-aligned, domain-grouped parallel execution, alert-only delivery
- **Skill System** — `~/.omega/skills/*/SKILL.md`, TOML frontmatter, MCP server activation, semantic intent matching
- **Project System** — `~/.omega/projects/*/ROLE.md`, per-project sessions/heartbeats/lessons/tasks, isolation model
- **OS-Level Sandbox** — three layers: code-level blocklist, OS-level (Seatbelt/Landlock), prompt-level (CLAUDE.md)
- **Multi-Language** — 8 languages (English, Spanish, Portuguese, French, German, Italian, Dutch, Russian)
- **Multi-Channel** — Telegram + WhatsApp, voice transcription, photo processing, cross-channel identity resolution

### 4. Architecture
- ASCII diagram: message flow from user through Gateway to Claude Code and back
- 6-crate workspace table with descriptions

### 5. How It Works (12-stage pipeline)
1. Dispatch (concurrent per-sender)
2. Auth (allowed user IDs)
3. Sanitize (prompt injection neutralization)
4. Identity (cross-channel resolution, language detection)
5. Context (history + facts + project + skills)
6. Keywords (9 categories, ~55-70% token savings)
7. Classify (Sonnet decides: simple or complex)
8. Route (Sonnet for simple, Opus for complex)
9. Markers (protocol markers processed and stripped)
10. Store (exchange saved, facts extracted)
11. Audit (full logging)
12. Respond (clean message delivered)

### 6. Token Efficiency Table
| Optimization | Savings |
|--------------|---------|
| Session persistence | ~90-99% on continuations |
| Keyword-gated sections | ~55-70% per first message |
| Keyword-gated DB queries | Skip when not relevant |
| Sonnet before Opus | Cheap routing |

### 7. Background Loops (4)
- Scheduler (60s poll, reminders + autonomous actions)
- Heartbeat (clock-aligned, parallel domain execution)
- Summarizer (auto-summarize idle conversations)
- CLAUDE.md maintenance (24h refresh)

### 8. Marker Protocol (10 markers shown + "12 more")
Key markers: SCHEDULE, SCHEDULE_ACTION, REWARD, LESSON, PROJECT_ACTIVATE, HEARTBEAT_ADD, SKILL_IMPROVE, BUILD_PROPOSAL, BUG_REPORT. Anti-hallucination verification noted.

### 9. Commands (20 commands)
Full table including /token command. Notes /setup as context-dependent (not listed but present).

### 10. HTTP API (3 endpoints)
- GET /api/health
- POST /api/pair
- POST /api/webhook
Bearer token auth with constant-time comparison.

### 11. Installation (3 methods)
- One-line install (curl)
- Build from source (cargo)
- System service (omega service install/status/uninstall)

### 12. Configuration
Minimal config.toml example with omega, provider, channel, heartbeat, scheduler sections.

### 13. Requirements
- macOS or Linux (x86_64 or ARM64)
- claude CLI installed and authenticated
- Telegram bot token and/or WhatsApp

### 14. Codebase Stats
- 154 functionalities, 18 modules
- 6 library crates + 1 binary
- 20 bot commands, 21 protocol markers, 9 keyword categories
- 6 AI providers, 2 messaging channels, 8 languages
- 13 database migrations, 3 security layers

### 15. License
MIT

---

## CLI Commands (Binary Level)

| Command | Description |
|---------|-------------|
| `omega start` | Launch the agent daemon |
| `omega status` | Health check (provider, channels) |
| `omega ask <message>` | One-shot query (no daemon) |
| `omega init` | Interactive setup wizard |
| `omega setup` | Reconfiguration menu |
| `omega pair` | Standalone WhatsApp QR pairing |
| `omega service install` | Install as system service |
| `omega service uninstall` | Remove system service |
| `omega service status` | Check service status |
| `omega uninstall` | Full system removal |

---

## Files Referenced
- `install.sh` -- One-line installer script
- `config.example.toml` -- Template configuration file
- `config.toml` -- User configuration (gitignored)
- `docs/functionalities/FUNCTIONALITIES.md` -- Full functionality inventory

### License
- **MIT License**
