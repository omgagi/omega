# OMEGA — Architecture Overview

Omega is a personal AI agent that connects messaging platforms to AI providers. You message it on Telegram or WhatsApp, it thinks using Claude (or another AI), and replies — remembering your conversations, preferences, and scheduled tasks across sessions.

This document explains how every piece fits together.

---

## The Big Picture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          YOUR PHONE                                 │
│                                                                     │
│   Telegram App ──────┐                                              │
│   WhatsApp App ──────┤                                              │
└──────────────────────┼──────────────────────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────────────────────┐
│                        OMEGA AGENT                                   │
│                                                                      │
│  ┌──────────┐   ┌──────────┐   ┌───────────┐   ┌────────────────┐  │
│  │ Channels │──>│ Gateway  │──>│ Providers │──>│ Claude / GPT   │  │
│  │          │   │          │   │           │   │ Ollama / etc   │  │
│  │ telegram │<──│ (brain)  │<──│ claude    │<──│                │  │
│  │ whatsapp │   │          │   │ openai    │   └────────────────┘  │
│  └──────────┘   │          │   │ anthropic │                        │
│                 │          │   │ ollama    │                        │
│                 │          │   │ openrouter│                        │
│                 │          │   └───────────┘                        │
│                 │          │                                        │
│                 │          │───> Memory (SQLite)                    │
│                 │          │───> Audit Log                          │
│                 │          │───> Skills & Projects                  │
│                 └──────────┘                                        │
│                      │                                              │
│            ┌─────────┼─────────┐                                    │
│            ▼         ▼         ▼                                    │
│       Summarizer  Scheduler  Heartbeat                              │
│       (background loops)                                            │
└──────────────────────────────────────────────────────────────────────┘
```

Omega is a single Rust binary. When it starts, it spawns listeners for each messaging platform, connects them to a central **Gateway**, and runs three background loops. Everything is async — one thread handles thousands of concurrent operations without blocking.

---

## How a Message Flows Through the System

When you send "remind me to call John at 3pm" on Telegram, here is exactly what happens inside Omega:

```
You type a message on Telegram
         │
         ▼
    ┌─────────┐
    │ Channel  │  Telegram listener receives your message
    │ Listener │  (omega-channels/src/telegram.rs)
    └────┬─────┘
         │ forwards via async queue
         ▼
    ┌─────────────────────────────────────────────────────────┐
    │                    GATEWAY PIPELINE                       │
    │                    (src/gateway.rs :: handle_message)     │
    │                                                          │
    │  1. AUTH ─────── Are you in the allowed_users list?      │
    │                  No? → "Not authorized." Stop.           │
    │                                                          │
    │  2. SANITIZE ─── Strip injection patterns from input     │
    │                  (omega-core/src/sanitize.rs)             │
    │                                                          │
    │  3. WELCOME ──── First time user? Send a welcome         │
    │                  message in their detected language       │
    │                                                          │
    │  4. COMMANDS ─── Starts with /? Handle it locally        │
    │                  /help, /tasks, /forget, /skills...      │
    │                  (src/commands.rs :: handle)              │
    │                  No provider call needed. Stop.          │
    │                                                          │
    │  5. CONTEXT ──── Build a rich context from memory:       │
    │     │            • Your conversation history             │
    │     │            • Known facts about you (name, tz...)   │
    │     │            • Summaries of past conversations       │
    │     │            • Related messages from other chats     │
    │     │            • Your pending scheduled tasks          │
    │     │            • Active project instructions           │
    │     │            • Platform hints (Telegram=markdown)    │
    │     │            • Group chat rules (if applicable)      │
    │     │            • Current heartbeat checklist           │
    │     │            (omega-memory/src/store.rs              │
    │     │             :: build_context + build_system_prompt) │
    │     │                                                    │
    │     └── The AI now knows who you are, what you said      │
    │         before, and what language you speak               │
    │                                                          │
    │  6. PROVIDER ─── Send context to AI (async)              │
    │     │            (omega-providers :: complete)            │
    │     │                                                    │
    │     ├── 15 seconds pass... "Taking a moment..."          │
    │     ├── 2 minutes pass... "Still working..."             │
    │     └── AI responds ✓                                    │
    │                                                          │
    │  7. MARKERS ──── Scan response for special markers:      │
    │     │                                                    │
    │     ├── SILENT → suppress response (group chats)         │
    │     ├── SCHEDULE: → create a scheduled task              │
    │     ├── LANG_SWITCH: → update language preference        │
    │     ├── HEARTBEAT_ADD: → add to monitoring checklist     │
    │     ├── HEARTBEAT_REMOVE: → remove from checklist        │
    │     └── WHATSAPP_QR → trigger WhatsApp pairing           │
    │                                                          │
    │     All markers are stripped before the user sees         │
    │     the response.                                        │
    │                                                          │
    │  8. STORE ────── Save exchange to SQLite                 │
    │                  (omega-memory/src/store.rs               │
    │                   :: store_exchange)                      │
    │                                                          │
    │  9. AUDIT ────── Log everything for security             │
    │                  (omega-memory/src/audit.rs :: log)       │
    │                                                          │
    │ 10. SEND ─────── Deliver response via Telegram           │
    │                                                          │
    └──────────────────────────────────────────────────────────┘
         │
         ▼
    You see "Sure! I'll remind you at 3pm."
    (The SCHEDULE: marker was extracted and a task was created)
```

---

## The Six Crates

Omega is organized as a Cargo workspace with six independent crates — like Lego blocks that snap together.

### omega-core — The Foundation

**Files:** `crates/omega-core/src/`

Defines the contracts that everything else depends on. No business logic, just types and rules.

| File | What It Defines |
|------|-----------------|
| `traits.rs` | `Provider` trait (AI backends) and `Channel` trait (messaging platforms) |
| `config.rs` | TOML config loading, prompt management, bundled file deployment |
| `context.rs` | `Context` struct — the package of information sent to the AI |
| `message.rs` | `IncomingMessage` and `OutgoingMessage` — the universal message format |
| `sanitize.rs` | Injection pattern neutralization |
| `error.rs` | `OmegaError` — unified error type across all crates |

The two traits are the heart of extensibility:

```
Provider trait:  name() + complete(context) + is_available()
Channel trait:   name() + start() + send() + send_typing() + stop()
```

Any struct that implements `Provider` can be an AI backend. Any struct that implements `Channel` can be a messaging platform. The gateway doesn't care which concrete type it is — it works with `Arc<dyn Provider>` and `Arc<dyn Channel>`.

### omega-providers — AI Backends

**Files:** `crates/omega-providers/src/`

Each file implements the `Provider` trait for a different AI service:

| File | Provider | How It Works |
|------|----------|-------------|
| `claude_code.rs` | Claude Code CLI | Spawns `claude -p --output-format json` as a subprocess |
| `anthropic.rs` | Anthropic API | Direct HTTP calls to the Messages API |
| `openai.rs` | OpenAI API | HTTP calls to ChatCompletions |
| `ollama.rs` | Ollama | Local HTTP calls to the Ollama server |
| `openrouter.rs` | OpenRouter | HTTP calls with model routing |

Claude Code CLI is the default zero-config provider. It inherits the user's local Claude authentication, so there's nothing to set up.

### omega-channels — Messaging Platforms

**Files:** `crates/omega-channels/src/`

Each file implements the `Channel` trait for a messaging platform:

| File | Platform | Library |
|------|----------|---------|
| `telegram.rs` | Telegram | teloxide (Rust Telegram Bot API) |
| `whatsapp.rs` | WhatsApp | whatsmeow (WhatsApp Web protocol) |

Channels produce `IncomingMessage` objects and consume `OutgoingMessage` objects. The gateway doesn't know or care whether a message came from Telegram or WhatsApp — it processes them identically.

### omega-memory — Persistent Brain

**Files:** `crates/omega-memory/src/`

SQLite-backed storage for everything Omega remembers.

| File | Purpose |
|------|---------|
| `store.rs` | Conversation lifecycle, context building, facts, tasks, summaries |
| `audit.rs` | Tamper-evident interaction log |

**Database tables:**

```
conversations ──── Tracks active/closed conversation threads
messages ────────── Individual messages within conversations
messages_fts ────── Full-text search index (SQLite FTS5)
facts ───────────── Key-value pairs about users (name, timezone, language...)
scheduled_tasks ─── Reminders and recurring tasks
audit_log ───────── Every interaction recorded for security
```

**Conversation lifecycle:**

```
[ACTIVE] ── user sends messages, history grows
    │
    │ 30 minutes of silence
    ▼
[IDLE] ── background summarizer picks it up
    │
    │ summarize → extract facts → close
    ▼
[CLOSED] ── summary stored, facts preserved, messages searchable via FTS5
```

The key function is `build_context()` in `store.rs` — it assembles everything the AI needs to know before responding:

1. Your recent messages in this conversation
2. Facts about you (name, timezone, preferences)
3. Summaries of your last 3 closed conversations
4. Related messages found via full-text search from any past conversation
5. Your pending scheduled tasks
6. Language preference
7. Marker instructions (SCHEDULE, LANG_SWITCH, HEARTBEAT_ADD/REMOVE)

### omega-skills — Extensible Capabilities

**Files:** `crates/omega-skills/src/lib.rs`

Skills are markdown files with TOML frontmatter that teach Omega new capabilities:

```
~/.omega/skills/google-workspace.md
~/.omega/skills/custom-tool.md
```

Projects are directories with instruction files that give Omega context about what you're working on:

```
~/.omega/projects/my-app/INSTRUCTIONS.md
~/.omega/projects/website/INSTRUCTIONS.md
```

When you say `/project my-app`, those instructions are prepended to the system prompt — Omega now understands your project's architecture, conventions, and goals.

### omega-sandbox — Secure Execution (Planned)

**Files:** `crates/omega-sandbox/src/`

Future crate for safely executing commands on behalf of the user.

---

## The Three Background Loops

While the gateway processes messages, three independent loops run concurrently:

### 1. Summarizer — Long-Term Memory

**Location:** `gateway.rs :: background_summarizer()`

Every 60 seconds, it looks for conversations that have been idle for 30+ minutes. For each one:

1. Asks the AI to write a 1-2 sentence summary
2. Asks the AI to extract facts (name, preferences, interests)
3. Stores the facts and summary
4. Closes the conversation

This is how Omega builds long-term memory. Your full chat history is compressed into summaries and facts that persist across conversations indefinitely.

### 2. Scheduler — Reminders & Recurring Tasks

**Location:** `gateway.rs :: scheduler_loop()`

Every 60 seconds, it checks the `scheduled_tasks` table for due items. When a task is due:

1. Sends "Reminder: {description}" via the original channel
2. One-shot tasks → marked as delivered
3. Recurring tasks → due date advanced (daily, weekly, monthly, weekdays)

Tasks are created when the AI includes a `SCHEDULE:` marker in its response. You say "remind me to exercise every morning at 8am" and the AI produces:

```
Sure! I'll remind you daily at 8am.
SCHEDULE: Exercise | 2026-02-18T08:00:00 | daily
```

The gateway extracts the marker, creates the task, and strips the marker before you see the response.

### 3. Heartbeat — Proactive Monitoring

**Location:** `gateway.rs :: heartbeat_loop()`

Every N minutes (default 30), it reads `~/.omega/HEARTBEAT.md` and asks the AI to evaluate the checklist. If everything is fine, the AI responds with `HEARTBEAT_OK` and the result is silently logged. If something needs attention, the alert is sent to your messaging channel.

The checklist is managed conversationally — say "keep an eye on my water intake" and Omega adds it to the file via a `HEARTBEAT_ADD:` marker. Say "stop monitoring water" and it's removed via `HEARTBEAT_REMOVE:`.

```
~/.omega/HEARTBEAT.md
─────────────────────
# My Monitoring Checklist
- Am I exercising regularly?
- Am I drinking enough water?
- Are there any system alerts?
```

---

## The Marker System

Omega uses a clever pattern to let the AI trigger side effects through its response text. The AI includes special markers, the gateway intercepts them, performs the action, and strips the markers before sending.

| Marker | Trigger | What Happens |
|--------|---------|-------------|
| `SCHEDULE: desc \| datetime \| repeat` | "remind me..." | Creates a scheduled task |
| `LANG_SWITCH: French` | "speak in French" | Updates language preference |
| `HEARTBEAT_ADD: item` | "monitor my sleep" | Adds to heartbeat checklist |
| `HEARTBEAT_REMOVE: item` | "stop monitoring sleep" | Removes from checklist |
| `SILENT` | Group chat, nothing to add | Response suppressed entirely |
| `WHATSAPP_QR` | `/whatsapp` command | Triggers QR pairing flow |

This pattern keeps the AI in charge of understanding intent ("remind me to call John at 3pm" → `SCHEDULE: Call John | 2026-02-17T15:00:00 | once`) while the gateway handles the mechanical execution. The user never sees the markers.

---

## The System Prompt — How Omega Thinks

The system prompt is not hardcoded. It lives at `~/.omega/SYSTEM_PROMPT.md` (auto-deployed from `prompts/SYSTEM_PROMPT.md` on first run) and is dynamically enriched before every AI call.

**Static base** (from file):
- Personality and rules ("You are an agent that DOES things")
- Soul principles (precise, warm, confident, bold)
- Marker instructions (SCHEDULE, LANG_SWITCH, HEARTBEAT)

**Dynamic enrichment** (from memory + context):
- Your known facts: "name: Ivan, timezone: America/Chicago"
- Recent conversation summaries
- Related past messages (full-text search)
- Active project instructions
- Platform hint: "This is Telegram, markdown is supported"
- Group chat rules (if applicable)
- Current heartbeat checklist items
- Pending scheduled tasks
- Language instruction: "Always respond in Spanish"

By the time the AI sees your message, it has a complete picture of who you are, what you've discussed before, what you're working on, and how to format its response.

---

## Bot Commands

Commands are handled locally by `commands.rs :: handle()` — they never reach the AI provider.

| Command | What It Does |
|---------|-------------|
| `/help` | List available commands |
| `/forget` | Close current conversation, start fresh |
| `/tasks` | Show pending scheduled tasks |
| `/cancel <id>` | Cancel a scheduled task |
| `/language [lang]` | Show or set language preference |
| `/skills` | List installed skills and their availability |
| `/projects` | List available projects |
| `/project [name]` | Activate a project (or `/project off` to deactivate) |
| `/whatsapp` | Start WhatsApp QR pairing |

Commands use the `CommandContext` struct (defined in `commands.rs`) which bundles the store, channel info, sender, and uptime into a single parameter.

---

## CLI Commands

Omega is also a CLI tool. Entry point: `main.rs`.

| Command | Purpose |
|---------|---------|
| `omega start` | Run the full agent (gateway + channels + background loops) |
| `omega status` | Health check — provider & channel availability |
| `omega ask "question"` | One-shot query (no memory, no channels) |
| `omega init` | Interactive setup wizard (cliclack-powered) |
| `omega service install` | Install as OS service (macOS LaunchAgent / Linux systemd) |
| `omega service uninstall` | Remove OS service |
| `omega service status` | Check service status |

---

## Security

| Layer | Protection |
|-------|-----------|
| Root guard | Refuses to run as root (`main.rs :: geteuid()`) |
| Auth | Per-channel `allowed_users` whitelist |
| Sanitization | Injection patterns neutralized before AI sees input |
| Audit | Every interaction logged with full metadata |
| Config isolation | `config.toml` is gitignored — secrets never committed |
| Provider isolation | `CLAUDECODE` env var removed to prevent nested sessions |
| Error masking | Raw errors logged internally; users see friendly messages |

---

## File Layout

```
~/.omega/
├── memory.db              ← SQLite database (conversations, facts, tasks, audit)
├── omega.log              ← Runtime logs
├── SYSTEM_PROMPT.md       ← AI personality and rules (editable)
├── WELCOME.toml           ← Welcome messages per language (editable)
├── HEARTBEAT.md           ← Monitoring checklist (editable, also managed via chat)
├── skills/
│   └── google-workspace.md  ← Bundled skill (auto-deployed)
└── projects/
    └── my-app/
        └── INSTRUCTIONS.md  ← Project context (user-created)
```

```
omega/                     ← Repository root
├── src/
│   ├── main.rs            ← Entry point, CLI parsing, initialization
│   ├── gateway.rs         ← Central event loop, message pipeline, background loops
│   ├── commands.rs        ← Bot command handlers (/help, /tasks, /forget...)
│   ├── cli.rs             ← CLI command implementations
│   ├── init.rs            ← Setup wizard
│   ├── selfcheck.rs       ← Pre-flight validation
│   └── service.rs         ← OS service management
├── crates/
│   ├── omega-core/        ← Types, traits, config, sanitization
│   ├── omega-providers/   ← AI backends (Claude Code, Anthropic, OpenAI, Ollama...)
│   ├── omega-channels/    ← Messaging platforms (Telegram, WhatsApp)
│   ├── omega-memory/      ← SQLite storage, audit logging
│   ├── omega-skills/      ← Skills and project loader
│   └── omega-sandbox/     ← Secure execution (planned)
├── prompts/
│   ├── SYSTEM_PROMPT.md   ← Bundled AI personality (source of truth)
│   └── WELCOME.toml       ← Bundled welcome messages
├── skills/
│   └── google-workspace.md ← Bundled skill definition
└── config.example.toml    ← Config template
```

---

## Design Philosophy

**Less is more.** Every decision follows the principle that the best code is the code you don't write:

- **Monolithic binary** — one `cargo build`, one binary, no microservices
- **Modular crates** — clean boundaries, but they all compile into one thing
- **SQLite for everything** — no external database to install or maintain
- **File-based config** — TOML you can read and edit, no admin panel
- **Trait-based extensibility** — adding a new AI provider is implementing one trait
- **Marker-based side effects** — the AI writes markers, the gateway acts on them
- **Background loops over cron jobs** — everything runs inside the same process
- **Graceful degradation** — if memory fails, responses still work; if audit fails, messages still send
