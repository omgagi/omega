# OMEGA — Architecture Overview

Omega is a personal AI agent that connects messaging platforms to AI providers. You message it on Telegram or WhatsApp, it thinks using Claude (or another AI), and replies — remembering your conversations, preferences, and scheduled tasks across sessions.

But Omega is far more than a chat relay. It has its own native intelligence layer — background loops that monitor and summarize; a marker system that lets the AI trigger real-world side effects; autonomous model routing that picks the right brain for each task; autonomous skill improvement; a quantitative trading engine; and OS-level sandboxing. The AI provider is just one piece. Most of the magic happens in Rust, before and after the AI ever sees your message.

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
│                 │          │   │ gemini    │                        │
│                 │          │   └───────────┘                        │
│                 │          │                                        │
│                 │          │───> Memory (SQLite)                    │
│                 │          │───> Audit Log                          │
│                 │          │───> Skills & Projects                  │
│                 │          │───> Sandbox (OS-level)                 │
│                 │          │───> Quant Engine                       │
│                 └──────────┘                                        │
│                      │                                              │
│          ┌───────────┼───────────┐                                  │
│          ▼           ▼           ▼                                  │
│     Summarizer    Scheduler   Heartbeat                             │
│          │                       │                                  │
│          │    Quant IBKR Feed    │                                  │
│          │     (if enabled)      │                                  │
│          └───────────────────────┘                                  │
│            (background loops)                                       │
└──────────────────────────────────────────────────────────────────────┘
```

Omega is a single Rust binary. When it starts, it spawns listeners for each messaging platform, connects them to a central **Gateway**, and runs background loops. Everything is async — one thread handles thousands of concurrent operations without blocking.

---

## What Omega Does On Its Own (Native Features)

Before diving into the architecture, here's the key distinction: Omega is **not** just a wrapper around an AI provider. The AI provider (Claude Code, OpenAI, etc.) handles reasoning and text generation. Everything else — the features listed below — is implemented natively in Rust by Omega itself.

### Native Intelligence

| Feature | What It Does |
|---------|-------------|
| **Autonomous model routing** | Every message gets a fast Sonnet classification (~90 tokens of context). Returns DIRECT (Sonnet handles it) or a numbered step list (Opus executes each step). The AI decides which model handles each message — no hardcoded rules. |
| **Multi-step execution** | Step lists run sequentially with progress reporting after each step, per-step retries (3x), per-step marker processing, and a final summary. |
| **Auto-resume** | When Claude Code hits its turn limit and returns a session_id, Omega automatically retries with `--session-id` up to 5 times, accumulating results across attempts. |
| **Per-sender serialization** | Concurrent messages from the same sender are buffered with a "Got it, I'll get to this next." ack, then processed in order. Different senders run fully concurrent. |
| **Delayed status updates** | "Taking a moment..." after 15 seconds, "Still working..." every 2 minutes. Localized in 8 languages. If the provider responds within 15s, no status shown. |

### Background Loops (Always Running)

| Loop | What It Does |
|------|-------------|
| **Summarizer** | Polls every 60s for idle conversations (30+ min). Auto-generates summaries + extracts facts as key:value pairs. This is how Omega builds long-term memory. |
| **Scheduler** | Polls every 60s for due tasks. **Reminder** tasks deliver text to the user. **Action** tasks invoke the provider autonomously with full system prompt + MCP tools. |
| **Heartbeat** | Periodic self-check (default 30 min, dynamic interval). Reads `~/.omega/HEARTBEAT.md` checklist, enriches with user facts. Suppresses "HEARTBEAT_OK". Respects active hours. |
| **Quant price feed** | When enabled via `/quant enable`, connects to IB Gateway via TWS API, streams real-time bars, processes each through the quant engine, stores latest signal for injection into user message context. |
| **Graceful shutdown** | On SIGINT, summarizes all active conversations then stops channels cleanly. |

### Marker-Driven Actions

The AI emits text markers in its response. The gateway extracts them, performs the action, and strips them before the user sees the response:

| Marker | What the Gateway Does |
|--------|----------------------|
| `SCHEDULE: desc \| datetime \| repeat` | Creates a reminder task (once, daily, weekly, monthly, weekdays) |
| `SCHEDULE_ACTION: desc \| datetime \| repeat` | Creates an action task (provider-backed autonomous execution) |
| `HEARTBEAT_ADD: item` | Appends item to `~/.omega/HEARTBEAT.md` |
| `HEARTBEAT_REMOVE: item` | Removes matching item from HEARTBEAT.md |
| `HEARTBEAT_INTERVAL: minutes` | Changes heartbeat frequency dynamically at runtime (1-1440) |
| `PROJECT_ACTIVATE: name` | Switches the user's active project context |
| `PROJECT_DEACTIVATE` | Clears active project |
| `LANG_SWITCH: language` | Updates user's preferred language |
| `SKILL_IMPROVE: skill \| lesson` | Appends lesson to skill's SKILL.md under `## Lessons Learned`, confirms to user |
| `SILENT` | Suppresses response entirely (group chats) |
| `WHATSAPP_QR` | Triggers WhatsApp QR pairing flow |

### Autonomous Skill Improvement

When Omega makes a mistake while using a skill (wrong API parameter, stopped a search too early, missed an edge case), it fixes the problem immediately, then emits a `SKILL_IMPROVE: skill-name | lesson learned` marker. The gateway appends the lesson to the skill's `SKILL.md` under a `## Lessons Learned` section (created if missing). Future invocations of that skill automatically benefit from past mistakes — no external tracking needed.

### Proactive Self-Scheduling

After every action it takes, the AI evaluates: "Does this need follow-up?" If yes, it uses SCHEDULE (for time-based checks) or HEARTBEAT_ADD (for ongoing monitoring) autonomously — no user request needed. This applies universally to any context.

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
    │  1. DISPATCH ──── Spawn as concurrent task. If sender    │
    │                   has an active call, buffer with ack.   │
    │                                                          │
    │  2. AUTH ─────── Are you in the allowed_users list?      │
    │                  No? → "Not authorized." Stop.           │
    │                                                          │
    │  3. SANITIZE ─── Neutralize injection patterns:          │
    │                  role impersonation [System], <|im_start │
    │                  instruction overrides "ignore all..."   │
    │                  (omega-core/src/sanitize.rs)             │
    │                                                          │
    │  4. IMAGES ───── Download attachments from channel,      │
    │                  save to ~/.omega/workspace/inbox/        │
    │                                                          │
    │  5. WELCOME ──── First time user? Detect language via    │
    │                  stop-word heuristic, store facts.        │
    │                  Graduated onboarding hints:              │
    │                  0 facts → strong intro                   │
    │                  <3 facts → lighter hint                  │
    │                  3+ facts → no hint                       │
    │                                                          │
    │  6. PROJECTS ─── Hot-reload ~/.omega/projects/*/ROLE.md  │
    │                  from disk (no restart needed)            │
    │                                                          │
    │  7. COMMANDS ─── Starts with /? Handle it locally        │
    │                  /help, /tasks, /forget, /skills...      │
    │                  (src/commands.rs :: handle)              │
    │                  No provider call needed. Stop.          │
    │                                                          │
    │  8. TYPING ───── Send typing indicator immediately,      │
    │                  repeat every 5s until response           │
    │                                                          │
    │  9. CONTEXT ──── Build a rich context from memory:       │
    │     │            • Your conversation history              │
    │     │            • Known facts about you (name, tz...)    │
    │     │            • Summaries of past conversations        │
    │     │            • Related messages via FTS5 recall       │
    │     │            • Your pending scheduled tasks           │
    │     │            • Active project ROLE.md instructions    │
    │     │            • Platform hints (Telegram=markdown)     │
    │     │            • Group chat rules (if applicable)       │
    │     │            • Sandbox constraint (workspace path)    │
    │     │            • Heartbeat checklist awareness          │
    │     │            • Quant advisory signal (if enabled)     │
    │     │            (omega-memory/src/store.rs               │
    │     │             :: build_context + build_system_prompt)  │
    │     │                                                     │
    │     └── The AI now knows who you are, what you said       │
    │         before, what you're working on, and how to format │
    │                                                          │
    │ 10. MCP MATCH ── Scan message against skill trigger      │
    │                  keywords. Matching skills' MCP servers   │
    │                  are injected into the provider call.     │
    │                                                          │
    │ 11. CLASSIFY ─── Fast Sonnet call (~90 tokens context):  │
    │     │            active project + last 3 msgs + skills   │
    │     │            → DIRECT (Sonnet) or step list (Opus)   │
    │     │                                                    │
    │     ├── DIRECT → single provider call with fast model    │
    │     │                                                    │
    │     └── STEPS → each step runs in fresh provider call    │
    │                 with accumulated context, progress after  │
    │                 each, retry up to 3x, markers per step   │
    │                                                          │
    │ 12. PROVIDER ─── Send context to AI (async)              │
    │     │            (omega-providers :: complete)            │
    │     │                                                    │
    │     ├── Sandbox wraps subprocess with OS-level write     │
    │     │   restrictions (Seatbelt/Landlock)                 │
    │     ├── MCP servers written to settings.local.json       │
    │     ├── 15 seconds pass... "Taking a moment..."          │
    │     ├── 2 minutes pass... "Still working..."             │
    │     ├── error_max_turns? → auto-resume with session-id   │
    │     └── AI responds ✓                                    │
    │                                                          │
    │ 13. IMAGES ───── Snapshot workspace before/after.        │
    │                  New images sent via send_photo().        │
    │                                                          │
    │ 14. SILENT ───── Group chat response starts with SILENT? │
    │                  Suppress entirely (don't send).          │
    │                                                          │
    │ 15. MARKERS ──── Scan response for all markers:          │
    │                  SCHEDULE, SCHEDULE_ACTION, HEARTBEAT_*,  │
    │                  PROJECT_*, LANG_SWITCH, SKILL_IMPROVE,   │
    │                  CANCEL_TASK, UPDATE_TASK, WHATSAPP        │
    │                  Extract, act, strip from response.       │
    │                                                          │
    │ 16. STORE ────── Save exchange to SQLite                 │
    │                  (omega-memory :: store_exchange)         │
    │                                                          │
    │ 17. AUDIT ────── Log everything for security             │
    │                  channel, sender, I/O, provider, model,  │
    │                  timing, status, denial reason            │
    │                  (omega-memory :: audit.rs)               │
    │                                                          │
    │ 18. SEND ─────── Deliver response via channel            │
    │                                                          │
    │ 19. CLEANUP ──── Delete workspace/inbox/ files           │
    │                                                          │
    │ 20. DRAIN ────── Process any buffered messages from      │
    │                  same sender (in order)                   │
    │                                                          │
    └──────────────────────────────────────────────────────────┘
         │
         ▼
    You see "Sure! I'll remind you at 3pm."
    (The SCHEDULE: marker was extracted and a task was created)
```

---

## The Seven Crates

Omega is organized as a Cargo workspace with seven independent crates — like Lego blocks that snap together.

### omega-core — The Foundation

**Files:** `crates/omega-core/src/`

Defines the contracts that everything else depends on. No business logic, just types and rules.

| File | What It Defines |
|------|-----------------|
| `traits.rs` | `Provider` trait (AI backends) and `Channel` trait (messaging platforms) |
| `config.rs` | TOML config loading, prompt management (identity/soul/system split), bundled file deployment |
| `context.rs` | `Context` struct — system prompt, history, MCP servers, per-request model/turns/tools overrides |
| `message.rs` | `IncomingMessage` and `OutgoingMessage` with attachments (Image/Document/Audio/Video) |
| `sanitize.rs` | Injection pattern neutralization — role impersonation, instruction overrides, code block tags |
| `error.rs` | `OmegaError` — unified error type across all crates |

The two traits are the heart of extensibility:

```
Provider trait:  name() + complete(context) + is_available() + requires_api_key()
Channel trait:   name() + start() + send() + send_typing() + send_photo() + stop()
```

Any struct that implements `Provider` can be an AI backend. Any struct that implements `Channel` can be a messaging platform. The gateway doesn't care which concrete type it is — it works with `Arc<dyn Provider>` and `Arc<dyn Channel>`.

**Prompt composition:** The `Prompts` struct splits the system prompt into three fields — `identity` (autonomous executor behavior), `soul` (personality, tone, boundaries), `system` (operational rules) — parsed from `## Identity`, `## Soul`, `## System` sections in `SYSTEM_PROMPT.md`. Gateway composes them: `format!("{}\n\n{}\n\n{}", identity, soul, system)`.

### omega-providers — AI Backends

**Files:** `crates/omega-providers/src/`

Each file implements the `Provider` trait for a different AI service:

| File | Provider | How It Works |
|------|----------|-------------|
| `claude_code.rs` | Claude Code CLI | Spawns `claude -p --output-format json --model <model>` as a subprocess |
| `anthropic.rs` | Anthropic API | Direct HTTP to Messages API (`x-api-key` header, system as top-level field) |
| `openai.rs` | OpenAI API | HTTP to ChatCompletions (Bearer token, also works with any OpenAI-compatible endpoint) |
| `ollama.rs` | Ollama | Local HTTP to `{base_url}/api/chat` (no API key) |
| `openrouter.rs` | OpenRouter | Reuses OpenAI types, `openrouter.ai/api/v1` endpoint, namespaced models |
| `gemini.rs` | Gemini | HTTP to `generativelanguage.googleapis.com/v1beta` (URL query param auth, assistant->model role mapping) |

Claude Code CLI is the default zero-config provider. It inherits the user's local Claude authentication, so there's nothing to set up.

**Native provider features** (implemented in Rust, not by the AI):

- **MCP server injection** — writes temporary `.claude/settings.local.json` with `mcpServers` config + `--allowedTools`, cleans up after
- **Auto-resume** — on `error_max_turns` with `session_id`, retries with `--session-id` up to 5 times, accumulating results
- **Per-request model override** — Sonnet for classification, Opus for multi-step execution
- **Sandbox integration** — wraps subprocess with `omega_sandbox::sandboxed_command()` for OS-level write restrictions
- **Friendly error mapping** — raw provider errors are never shown to users

### omega-channels — Messaging Platforms

**Files:** `crates/omega-channels/src/`

Each file implements the `Channel` trait for a messaging platform:

| Feature | Telegram | WhatsApp |
|---------|----------|----------|
| **Protocol** | Bot API long polling (30s timeout) | WhatsApp Web (`whatsapp-rust` crate) |
| **Voice transcription** | OpenAI Whisper API (download via `getFile` + transcribe) | — |
| **Photo reception** | Downloads largest photo size | Downloads via `client.download()` |
| **Photo sending** | `sendPhoto` with multipart form | — |
| **Group chat detection** | `chat_type == "group" \| "supergroup"` | — |
| **Typing indicator** | `sendChatAction` "typing" | WhatsApp chatstate "composing" |
| **Message splitting** | 4096 char limit, splits on newline boundaries | 4096 char limit |
| **Markdown + fallback** | Sends Markdown, retries as plain text on parse error | — |
| **QR pairing flow** | — | Terminal QR (Unicode half-block) + PNG image |
| **Session persistence** | — | SQLite-backed WhatsApp session store (14 tables) |
| **Self-chat filtering** | — | `is_from_me + sender == chat` (personal channel mode) |
| **Echo prevention** | — | Tracks sent message IDs via `HashSet` |
| **Nested message unwrapping** | — | `device_sent`, `ephemeral`, `view_once` |
| **Error recovery** | Exponential backoff (1s to 60s) on poll errors | Auto-reconnect on disconnect (5s delay) |

Channels produce `IncomingMessage` objects and consume `OutgoingMessage` objects. The gateway doesn't know or care whether a message came from Telegram or WhatsApp — it processes them identically.

### omega-memory — Persistent Brain

**Files:** `crates/omega-memory/src/`

SQLite-backed storage with WAL mode for everything Omega remembers.

| File | Purpose |
|------|---------|
| `store.rs` | Conversation lifecycle, context building, facts, tasks, summaries |
| `audit.rs` | Tamper-evident interaction log (channel, sender, I/O, provider, model, timing, status) |

**Database tables (7 schema migrations):**

```
conversations ──── Tracks active/closed conversation threads (30-min inactivity boundary)
messages ────────── Individual messages within conversations
messages_fts ────── Full-text search index (SQLite FTS5) for cross-conversation recall
facts ───────────── Key-value pairs about users (name, timezone, language, preferences...)
summaries ───────── Compressed conversation summaries for long-term memory
scheduled_tasks ─── Reminders and action tasks (one-shot + recurring)
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

**Context building** — the key function `build_context()` assembles everything the AI needs:

1. Your recent messages in this conversation
2. Facts about you (structured user profile: identity keys first, context keys second, rest last)
3. Summaries of your last 3 closed conversations
4. Related messages found via FTS5 full-text search from any past conversation
5. Your pending scheduled tasks
6. Language preference + onboarding hints (graduated by fact count)
7. Dynamic marker instructions (SCHEDULE, SCHEDULE_ACTION, HEARTBEAT_*, SKILL_IMPROVE, LANG_SWITCH)

**Language detection:** Stop-word heuristic for 7 languages (Spanish, Portuguese, French, German, Italian, Dutch, Russian) + English default.

### omega-skills — Extensible Capabilities

**Files:** `crates/omega-skills/src/lib.rs`

Skills are markdown files with TOML or YAML frontmatter that teach Omega new capabilities:

```
~/.omega/skills/google-workspace/SKILL.md
~/.omega/skills/playwright-mcp/SKILL.md
~/.omega/skills/custom-tool/SKILL.md
```

**5 bundled skills** auto-deployed on first run: claude-code, google-workspace, playwright-mcp, skill-creator, ibkr-quant.

**Skill features:**
- **Availability check** — verifies required CLI tools are installed (e.g., `npx`, `gog`)
- **MCP server declarations** — parsed from frontmatter into `McpServer` structs (name, command, args)
- **Trigger matching** — pipe-separated keywords, case-insensitive substring match on user text. Matching skills' MCP servers are injected into the provider call.

Projects are directories with instruction files that give Omega context about what you're working on:

```
~/.omega/projects/my-app/ROLE.md
~/.omega/projects/website/ROLE.md
```

When you say `/project my-app`, those instructions are prepended to the system prompt — Omega now understands your project's architecture, conventions, and goals. Projects are **hot-reloaded from disk on every message** — no restart needed.

### omega-sandbox — OS-Level Filesystem Enforcement

**Files:** `crates/omega-sandbox/src/`

Three-level workspace isolation with OS-level write enforcement:

| Mode | Read | Write | Use Case |
|------|------|-------|----------|
| `sandbox` | Workspace only | Workspace only | Maximum isolation |
| `rx` | Anywhere | Workspace only | Read the system, write to sandbox |
| `rwx` | Anywhere | Anywhere | Unrestricted (trusted environments) |

**Enforcement:**
- **macOS:** Seatbelt profiles via `sandbox-exec -p`. Denies `file-write*`, then allows: `~/.omega/`, `/private/tmp`, `/private/var/folders`, `~/.claude`, `~/.cargo`.
- **Linux:** Landlock LSM via `pre_exec` hook. Allows read+execute on `/`, full access to allowed dirs. Requires kernel 5.13+.
- **Fallback:** Graceful degradation to prompt-only enforcement on unsupported platforms.

The provider subprocess is always started with `current_dir` set to `~/.omega/workspace/`, regardless of mode.

### omega-quant — Quantitative Trading Engine

**Files:** `crates/omega-quant/src/`

A fully native trading pipeline — no external AI involved. Pure Rust math:

```
IBKR TWS real-time bar
         │
         ▼
    Kalman Filter ──── 2D state [price, trend], process/measurement noise
         │
         ▼
    EWMA Volatility + Hurst Exponent (R/S analysis: mean-reverting vs trending)
         │
         ▼
    HMM Regime Detection ──── 3 states: Bull / Bear / Lateral
         │                    5 discrete observations, Baum-Welch training
         ▼
    Merton Allocation ──── (mu - r) / (gamma * sigma^2), clamped [-0.5, 1.5]
         │
         ▼
    Fractional Kelly ──── 25% of full Kelly, max 10% allocation
         │
         ▼
    Direction / Action / Execution Strategy
```

| Component | File | What It Does |
|-----------|------|-------------|
| **Kalman filter** | `kalman.rs` | 2D state (price + trend), 2x2 covariance matrix, no external math library |
| **HMM** | `hmm.rs` | 3-state regime detection, forward belief update, Baum-Welch training |
| **Kelly criterion** | `kelly.rs` | Fractional sizing (25%), max 10% allocation, min 55% confidence threshold |
| **Execution planning** | `execution.rs` | Immediate (<0.1% daily volume) vs TWAP (3-20 slices) vs NoTrade (>1%) |
| **Live executor** | `executor.rs` | Circuit breaker (2% deviation, 3 consecutive failures), daily limits, crash recovery |
| **Market data** | `market_data.rs` | IBKR TWS real-time price feed via `ibapi` crate with auto-reconnect |
| **Signal types** | `signal.rs` | `QuantSignal` with regime, direction, confidence, Kelly sizing, Merton allocation |

The quant engine outputs advisory signals that are injected into the system prompt — the AI sees "ADVISORY (NOT FINANCIAL ADVICE): regime=Bull, direction=Long, confidence=72%..." as context, not as a command.

---

## The Background Loops

While the gateway processes messages, four independent loops run concurrently:

### 1. Summarizer — Long-Term Memory

**Location:** `gateway.rs :: background_summarizer()`

Every 60 seconds, it looks for conversations that have been idle for 30+ minutes. For each one:

1. Asks the AI to write a 1-2 sentence summary
2. Asks the AI to extract facts (name, preferences, interests) as key:value pairs
3. Stores the facts and summary
4. Closes the conversation

This is how Omega builds long-term memory. Your full chat history is compressed into summaries and facts that persist across conversations indefinitely.

### 2. Scheduler — Reminders & Autonomous Actions

**Location:** `gateway.rs :: scheduler_loop()`

Every 60 seconds, it checks the `scheduled_tasks` table for due items. Two task types:

**Reminder tasks** (created by `SCHEDULE:` marker):
1. Sends "Reminder: {description}" via the original channel
2. One-shot tasks → deleted after delivery
3. Recurring tasks → due date advanced (daily, weekly, monthly, weekdays)

**Action tasks** (created by `SCHEDULE_ACTION:` marker):
1. Invokes the provider with full system prompt, MCP servers, and model selection
2. Processes all markers from the response (can chain: an action can schedule more actions)
3. Sends the result to the user

Tasks are created when the AI includes markers in its response. You say "remind me to exercise every morning at 8am" and the AI produces:

```
Sure! I'll remind you daily at 8am.
SCHEDULE: Exercise | 2026-02-18T08:00:00 | daily
```

The gateway extracts the marker, creates the task, and strips the marker before you see the response.

### 3. Heartbeat — Proactive Monitoring & Self-Reflection

**Location:** `gateway.rs :: heartbeat_loop()`

Every N minutes (default 30, adjustable via `HEARTBEAT_INTERVAL:` marker + `Arc<AtomicU64>`), it:

1. Checks active hours before running (configurable, e.g., 8am-11pm only)
2. Reads `~/.omega/HEARTBEAT.md` checklist (skips entirely if file doesn't exist)
3. Enriches the prompt with:
   - User facts and recent summaries
4. Sends to AI for evaluation
5. Suppresses "HEARTBEAT_OK" responses (silently logged)
6. Alerts the user if something needs attention

```
~/.omega/HEARTBEAT.md
─────────────────────
# My Monitoring Checklist
- Am I exercising regularly?
- Am I drinking enough water?
- Are there any system alerts?
```

The checklist is managed conversationally — say "keep an eye on my water intake" and Omega adds it via `HEARTBEAT_ADD:`. Say "stop monitoring water" and it's removed via `HEARTBEAT_REMOVE:`.

### 4. Quant Price Feed (Optional)

**Location:** `gateway.rs` (lazy init via `/quant enable` command)

When enabled via the `/quant` bot command, connects to IB Gateway via TWS API (ibapi crate) and streams 5-second real-time bars. A consumer loop processes each bar through the full `QuantEngine` pipeline (Kalman → HMM → Merton → Kelly), storing the latest signal for injection into the next user message's context. Configuration (symbol, portfolio value, paper/live mode) stored in SQLite facts table — no config.toml needed.

---

## The Marker System

Omega uses a clean pattern to let the AI trigger side effects through its response text. The AI includes special markers, the gateway intercepts them, performs the action, and strips the markers before sending.

| Marker | Trigger | What Happens |
|--------|---------|-------------|
| `SCHEDULE: desc \| datetime \| repeat` | "remind me..." | Creates a scheduled reminder task |
| `SCHEDULE_ACTION: desc \| datetime \| repeat` | "check on X tomorrow" | Creates an autonomous action task |
| `LANG_SWITCH: French` | "speak in French" | Updates language preference |
| `HEARTBEAT_ADD: item` | "monitor my sleep" | Adds to heartbeat checklist |
| `HEARTBEAT_REMOVE: item` | "stop monitoring sleep" | Removes from checklist |
| `HEARTBEAT_INTERVAL: minutes` | "check every 15 minutes" | Changes heartbeat interval at runtime (1-1440) |
| `PROJECT_ACTIVATE: name` | "let's work on my-app" | Switches active project context |
| `PROJECT_DEACTIVATE` | "done with this project" | Clears active project |
| `SKILL_IMPROVE: skill \| lesson` | Mistake detected using a skill | Appends lesson to skill's SKILL.md, confirms |
| `SILENT` | Group chat, nothing to add | Response suppressed entirely |
| `WHATSAPP_QR` | `/whatsapp` command | Triggers QR pairing flow |

This pattern keeps the AI in charge of understanding intent ("remind me to call John at 3pm" → `SCHEDULE: Call John | 2026-02-17T15:00:00 | once`) while the gateway handles the mechanical execution. The user never sees the markers.

---

## The System Prompt — How Omega Thinks

The system prompt is not hardcoded. It lives at `~/.omega/SYSTEM_PROMPT.md` (auto-deployed from `prompts/SYSTEM_PROMPT.md` on first run) and is dynamically enriched before every AI call.

**Static base** (from file, three sections):
- **Identity** — autonomous executor with concrete behavioral examples ("You are an agent that DOES things")
- **Soul** — personality, context-aware tone, explicit boundaries, emoji policy
- **System** — operational rules, group chat participation, marker instructions

**Dynamic enrichment** (from memory + context):
- Your known facts as a structured user profile (identity keys first, context keys second)
- Recent conversation summaries
- Related past messages (FTS5 full-text search recall)
- Active project ROLE.md instructions
- Platform hint: "This is Telegram, markdown is supported"
- Group chat rules + SILENT suppression logic
- Current heartbeat checklist items
- Pending scheduled tasks
- Language instruction: "Always respond in Spanish"
- Sandbox workspace path constraint
- Quant advisory signal (if enabled)
- Onboarding hints (graduated by fact count)

By the time the AI sees your message, it has a complete picture of who you are, what you've discussed before, what you're working on, and how to format its response.

---

## Bot Commands

Commands are handled locally by `commands.rs :: handle()` — they never reach the AI provider.

| Command | What It Does |
|---------|-------------|
| `/help` | List available commands |
| `/status` | Show uptime, provider, sandbox mode, DB size |
| `/memory` | Show conversation count, message count, fact count |
| `/history` | Show last 5 conversation summaries |
| `/facts` | List all known facts about the user |
| `/forget` | Close current conversation, start fresh (triggers summarization) |
| `/tasks` | Show pending scheduled tasks (with `[action]` badge for action tasks) |
| `/cancel <id>` | Cancel a scheduled task by ID prefix match |
| `/language [lang]` | Show or set language preference |
| `/lang [lang]` | Alias for `/language` |
| `/personality [text\|reset]` | Show, set, or reset personality preference |
| `/skills` | List installed skills with availability (`[installed]` / `[not installed]`) |
| `/projects` | List all projects, marks the active one |
| `/project [name\|off]` | Activate, deactivate, or show current project (clears conversation on change) |
| `/whatsapp` | Start WhatsApp QR pairing |
| `/quant` | Manage IBKR quant engine (enable/disable/symbol/portfolio/paper/live) |

Commands use the `CommandContext` struct which bundles the store, channel info, sender, text, uptime, provider name, skills, projects, and sandbox mode into a single parameter.

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

The **init wizard** (`src/init.rs`) is a full `cliclack`-styled interactive setup: creates `~/.omega/`, checks Claude CLI, Anthropic auth, Telegram bot token, user ID, Whisper API key, WhatsApp QR pairing, Google Workspace setup (with incognito browser detection for OAuth), sandbox mode selection, config.toml generation, and optional service installation.

The **self-check** (`src/selfcheck.rs`) runs at startup: verifies database accessibility, provider availability, and Telegram token validity.

---

## Security

| Layer | Protection |
|-------|-----------|
| Root guard | Refuses to run as root (`main.rs :: geteuid()`) |
| Auth | Per-channel `allowed_users` whitelist |
| Sanitization | Role impersonation + instruction override patterns neutralized before AI sees input |
| Sandbox | OS-level filesystem enforcement — Seatbelt (macOS) / Landlock (Linux) |
| Audit | Every interaction logged with full metadata (channel, sender, I/O, model, timing, status) |
| Config isolation | `config.toml` is gitignored — secrets never committed |
| Provider isolation | `CLAUDECODE` env var removed to prevent nested sessions |
| Error masking | Raw errors logged internally; users see friendly localized messages |
| Per-sender serialization | Prevents race conditions — one active call per sender at a time |

---

## File Layout

```
~/.omega/
├── memory.db              ← SQLite database (conversations, facts, tasks, audit, limitations)
├── omega.log              ← Runtime logs
├── SYSTEM_PROMPT.md       ← AI personality and rules (editable, 3 sections: Identity/Soul/System)
├── WELCOME.toml           ← Welcome messages per language (editable)
├── HEARTBEAT.md           ← Monitoring checklist (editable, also managed via chat)
├── workspace/             ← Sandbox working directory (current_dir for provider)
│   ├── inbox/             ← Temporary storage for incoming image attachments
│   └── .claude/           ← Temporary MCP settings (auto-cleaned)
├── skills/
│   ├── claude-code/
│   │   └── SKILL.md       ← Bundled skill (auto-deployed)
│   ├── google-workspace/
│   │   └── SKILL.md
│   ├── playwright-mcp/
│   │   └── SKILL.md
│   ├── skill-creator/
│   │   └── SKILL.md
│   └── ibkr-quant/
│       └── SKILL.md
├── projects/
│   └── my-app/
│       └── ROLE.md         ← Project context (user-created or AI-created)
└── whatsapp_session/
    └── whatsapp.db         ← WhatsApp session persistence
```

```
omega/                     ← Repository root
├── src/
│   ├── main.rs            ← Entry point, CLI parsing, root guard, dual logging, provider factory
│   ├── gateway.rs         ← Central event loop, message pipeline, background loops, markers
│   ├── commands.rs        ← Bot command handlers (/help, /tasks, /forget, /project...)
│   ├── cli.rs             ← CLI command implementations
│   ├── init.rs            ← Setup wizard (cliclack-powered, browser detection, OAuth)
│   ├── selfcheck.rs       ← Pre-flight validation
│   └── service.rs         ← OS service management (LaunchAgent / systemd)
├── crates/
│   ├── omega-core/        ← Types, traits, config, prompt composition, sanitization
│   ├── omega-providers/   ← AI backends (Claude Code, Anthropic, OpenAI, Ollama, OpenRouter, Gemini)
│   ├── omega-channels/    ← Messaging platforms (Telegram, WhatsApp)
│   ├── omega-memory/      ← SQLite storage, audit logging, context building
│   ├── omega-skills/      ← Skills loader, project loader, MCP trigger matching
│   ├── omega-sandbox/     ← OS-level filesystem enforcement (Seatbelt, Landlock)
│   └── omega-quant/       ← Quantitative trading engine (Kalman, HMM, Kelly, IBKR)
├── prompts/
│   ├── SYSTEM_PROMPT.md   ← Bundled AI personality (source of truth)
│   └── WELCOME.toml       ← Bundled welcome messages
├── skills/
│   ├── claude-code/
│   │   └── SKILL.md
│   ├── google-workspace/
│   │   └── SKILL.md
│   ├── playwright-mcp/
│   │   └── SKILL.md
│   ├── skill-creator/
│   │   └── SKILL.md
│   └── ibkr-quant/
│       └── SKILL.md
├── specs/                 ← Technical specifications (mirror of implementation)
├── docs/                  ← Developer-facing guides
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
- **OS-level sandbox** — real filesystem enforcement, not just prompt-based trust
- **Autonomous model routing** — the AI picks its own brain size per task
- **Skill improvement** — Omega learns from its mistakes and updates its own skills
- **Graceful degradation** — if memory fails, responses still work; if audit fails, messages still send
