# Architecture — End-to-End Message Flow

Omega is a personal AI agent infrastructure written in Rust. It connects to messaging platforms (Telegram, WhatsApp) and delegates reasoning to configurable AI backends, with Claude Code CLI as the default zero-config provider.

The system is a Cargo workspace with 7 crates. See `CLAUDE.md` for the full crate table.

---

## The Big Picture

```
┌──────────┐     long poll      ┌───────────┐     subprocess      ┌──────────────┐
│ Telegram │ ←——(instant push)——→ │  Gateway  │ ——(claude -p)——→   │  Claude Code │
│   API    │                     │  (Rust)   │ ←——(JSON result)——  │     CLI      │
└──────────┘                     └───────────┘                     └──────────────┘
                                      │
                                 ┌────┴────┐
                                 │ SQLite  │
                                 │ memory  │
                                 └─────────┘
```

Three components. One process. No microservices.

---

## Phase 1: Message Arrives (Channel Layer)

**Files:** `crates/omega-channels/src/telegram/polling.rs`

### How Telegram Long Polling Works

This is **not** "check every 30 seconds." It's an always-open HTTP connection:

```
1. Omega opens HTTP request:  GET /getUpdates?timeout=30
2. Telegram holds the connection open, waiting...
3. User sends a message on Telegram
4. Telegram IMMEDIATELY pushes it through the open connection
5. Omega receives the message (~50-200ms latency)
6. Omega processes it, immediately opens a new connection → back to step 1
```

The `timeout=30` is a **keep-alive**: "If nothing arrives in 30 seconds, return empty so we can reconnect." The HTTP client timeout is set to 35 seconds (5s headroom) to avoid false timeouts. On errors, exponential backoff (1s → 2s → 4s → ... → 60s max) prevents hammering.

**Result:** Your message reaches Omega in milliseconds, not seconds.

### What Happens on Arrival

For each incoming Telegram update:

1. **Extract content** — text messages pass through directly; voice messages are downloaded and transcribed via Whisper (`[Voice message] <transcript>`); photos are downloaded as byte arrays
2. **Auth check** — reject if `user.id` not in `allowed_users` list
3. **Group filter** — drop messages from groups/supergroups (private-only mode)
4. **Build `IncomingMessage`** — normalized struct with id, channel, sender_id, sender_name, text, attachments, reply_target
5. **Send to gateway** — via `mpsc::channel` (async, non-blocking)

WhatsApp follows the same pattern but uses the WhatsApp Web protocol instead of HTTP polling.

---

## Phase 2: Gateway Dispatch (Concurrency Control)

**File:** `src/gateway/mod.rs`

The gateway receives messages from all channels through a unified receiver. Each message spawns as an independent async task (`tokio::spawn`), but messages from the **same sender** are serialized:

```
Sender A: msg1 → processing...
Sender A: msg2 → "Got it, I'll get to this next." (buffered)
Sender A: msg3 → (also buffered)
Sender B: msg1 → processing in parallel with A's msg1

A's msg1 completes → drain buffer → process msg2 → process msg3
```

This prevents race conditions (two provider calls for the same user) while keeping different users fully concurrent.

---

## Phase 3: Message Pipeline (Step by Step)

**File:** `src/gateway/pipeline.rs`

Every message passes through these stages in order:

### 3.1 Auth

Verify the sender is in the channel's `allowed_users` list. Unauthorized → audit log + reject.

### 3.2 Sanitize

Strip prompt injection patterns (`[SYSTEM]`, `<claude>` tags, etc.) from user text before it reaches the AI.

### 3.3 Save Attachments

If the message has images, save them to `~/.omega/workspace/inbox/{uuid}.jpg` and prepend `[Attached image: /path]` to the text. An RAII guard auto-deletes these files when processing completes.

### 3.4 Identity Resolution

- **Cross-channel aliases:** If a user messages from WhatsApp after using Telegram, fuzzy name matching links the accounts via the `user_aliases` table
- **First contact:** Store `welcomed: true` fact, detect and store `preferred_language`

### 3.5 Command Dispatch

If the message starts with `/` (e.g., `/help`, `/forget`, `/tasks`), handle it directly and **return early** — no AI provider needed. Commands are fully localized.

### 3.6 Typing Indicator

Send Telegram's "typing..." bubble. A background task re-sends it every 5 seconds to keep it visible during long provider calls.

### 3.7 Build Context from Memory

**File:** `crates/omega-memory/src/store/context.rs`

This is where the gateway decides **what data** to load from SQLite:

| Data | When loaded | Why conditional |
|------|-------------|-----------------|
| Conversation history (last N messages) | Always | Core context |
| User facts (profile) | Always | Needed for onboarding + language |
| Learned lessons | Always | Tiny, high behavioral value |
| Outcomes (last 15 rewards) | If outcome keywords match | Token savings |
| Pending tasks | If scheduling keywords match | Token savings |
| Summaries (past conversations) | If recall keywords match | Token savings |
| Semantic recall | If recall keywords match | Token savings |

### 3.8 Compose System Prompt

**File:** `prompts/SYSTEM_PROMPT.md` (6 sections)

The system prompt is assembled from sections, not sent as a monolith:

| Section | Injection | Content |
|---------|-----------|---------|
| `## Identity` | Always | Who OMEGA is, behavioral examples |
| `## Soul` | Always | Personality, tone, boundaries |
| `## System` | Always | Core rules, marker quick-reference |
| `## Scheduling` | If scheduling keywords match | Task/reminder rules |
| `## Projects` | If project keywords match | Project conventions |
| `## Meta` | If meta keywords match | Skill improvement, heartbeat, WhatsApp |

**Token savings:** ~55-70% on typical messages by skipping irrelevant sections.

### 3.9 Skill Trigger Matching

Scan the message for trigger keywords defined in `~/.omega/skills/*/SKILL.md`. If matched, attach the skill's MCP server config to the context — the provider will activate it.

### 3.10 Session Check (Claude Code CLI only)

If an active CLI session exists for this sender:
- Set `context.session_id` (provider will pass `--resume`)
- Replace full system prompt with minimal context update (just current time + keyword-gated sections)
- Clear history (the CLI session already has it)
- **Token savings:** ~90-99% on continuation messages

If no session exists, send full prompt + history (first message in conversation).

---

## Phase 4: Classification & Routing

**File:** `src/gateway/routing.rs`

Every message gets a fast **Sonnet classification call** before the real work:

```
User message + ~90 tokens of context (active project, last 3 messages, skill names)
  → Sonnet (no tools, no system prompt, cheap and fast)
  → "DIRECT" or numbered step list
```

**Routing rules:**
- **DIRECT** (routine): greetings, reminders, scheduling, lookups, simple questions → handled by **Sonnet** (fast, cheap)
- **Steps** (complex): multi-file code changes, deep research, sequential dependencies → executed by **Opus** (powerful), one provider call per step, with progress updates after each

When in doubt, the classifier prefers DIRECT.

---

## Phase 5: Claude Code CLI Invocation

**File:** `crates/omega-providers/src/claude_code/`

### Command Construction

```bash
claude -p "<prompt>" \
  --output-format json \
  --max-turns <N> \
  --model sonnet|opus \
  --resume <session_id>                 # if continuation
  --dangerously-skip-permissions        # OS sandbox is the real boundary
```

### Security: OS-Level Sandbox

Before execution, the command is wrapped in an OS-level sandbox:
- **macOS:** Seatbelt profile blocks writes to `/System`, `/bin`, `/sbin`, `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/private/etc`, `/Library`, and `~/.omega/data/` (memory.db)
- **Linux:** Landlock — system dirs read-only, user dirs writable

The sandbox is always active. `--dangerously-skip-permissions` only skips Claude Code's own permission prompts — the OS-level protection remains.

### Working Directory

All Claude Code work happens in `~/.omega/workspace/`. This is where it reads/writes files, runs commands, etc.

### MCP Server Activation

If skills matched triggers in Phase 3.9, the provider writes a temporary `~/.omega/workspace/.claude/settings.local.json` with MCP server configs. After the call completes, this file is deleted.

### Execution

- Timeout: 60 minutes (configurable)
- Environment: `CLAUDECODE` env var removed to prevent nested session errors
- Output: JSON with `result` (text), `session_id`, `model`, `num_turns`

### Auto-Resume

If Claude hits the turn limit (`error_max_turns`) and returns a `session_id`:
1. Wait with exponential backoff (2s, 4s, 8s, 16s, 32s)
2. Retry with `--resume` and "Continue where you left off..."
3. Accumulate results across attempts
4. Repeat up to 5 times (configurable)

### Status Updates

While Claude works, the gateway sends delayed status messages to keep the user informed:
- **15 seconds:** "This is taking a moment..."
- **Every 2 minutes:** "Still working..."
- If Claude responds within 15 seconds, no status message is shown.

---

## Phase 6: Response Processing

**File:** `src/gateway/routing.rs`, `src/gateway/process_markers.rs`

### 6.1 Capture Session ID

Store the returned `session_id` in the `cli_sessions` map (keyed by `channel:sender_id`). The next message from this sender will use `--resume` with this ID.

### 6.2 Process Markers

The AI embeds structured commands in its response. The gateway extracts, executes, and strips them before the user sees the text:

| Marker | Effect |
|--------|--------|
| `SCHEDULE: desc \| datetime \| repeat` | Create reminder in DB |
| `SCHEDULE_ACTION: desc \| datetime \| repeat` | Create action task (AI will execute it later with full tool access) |
| `CANCEL_TASK: id_prefix` | Cancel a scheduled task |
| `UPDATE_TASK: id \| desc \| due \| repeat` | Modify a scheduled task |
| `REWARD: +1\|domain\|lesson` | Store outcome in working memory (+1 helpful, 0 neutral, -1 redundant) |
| `LESSON: domain\|rule` | Upsert long-term behavioral rule |
| `PERSONALITY: style` | Update personality fact |
| `LANG_SWITCH: code` | Change preferred language |
| `FORGET_CONVERSATION` | Close conversation + clear CLI session |
| `HEARTBEAT_ADD: item` | Add item to monitoring checklist |
| `HEARTBEAT_REMOVE: item` | Remove item from monitoring checklist |
| `HEARTBEAT_INTERVAL: minutes` | Change heartbeat frequency |
| `SKILL_IMPROVE: name \| lesson` | Append lesson to skill's SKILL.md |
| `BUG_REPORT: description` | Append to ~/.omega/BUG.md |
| `PROJECT_ACTIVATE: name` | Switch active project |
| `PURGE_FACTS` | Delete all non-system user facts |

### 6.3 Store in Memory

Save both the user message and AI response in the `messages` table. Update conversation `last_activity`.

### 6.4 Audit Log

Insert a row in `audit_log` with: channel, sender, input/output text, provider, model, timing, status.

### 6.5 Send Response to Telegram

- Split into 4096-character chunks (Telegram limit)
- Send each chunk with `parse_mode: "Markdown"`
- If Markdown parsing fails, retry as plain text

### 6.6 Task Confirmation

If markers created, cancelled, or updated tasks: send a localized confirmation with actual DB results (anti-hallucination — verifies what really happened, warns about similar/duplicate tasks).

### 6.7 Workspace Image Delivery

Compare workspace images before and after the provider call. New/modified images are sent to the user via `sendPhoto` and deleted from the workspace.

### 6.8 Drain Buffered Messages

If other messages from this sender were buffered during processing, pop and process them in order. When the buffer is empty, remove the sender from the active set.

---

## Session-Based Prompt Persistence

The Claude Code CLI supports session continuity to avoid re-sending the full system prompt on every message.

### How It Works

1. **First message:** Full context (system prompt + history + user message) with `session_id: None`. The CLI returns a `session_id` in its JSON response.

2. **Subsequent messages:** Gateway sets `Context.session_id`. The provider passes `--resume <session_id>`. System prompt is replaced with a minimal context update (current time + keyword-gated sections only). History is cleared (already in the CLI session). **Token savings: ~90-99%.**

3. **Invalidation:** Session ID is cleared on `/forget`, `FORGET_CONVERSATION` marker, idle timeout, or provider error.

4. **Fallback:** If a session-based call fails, the gateway retries with a fresh full-context call. The user never sees the failure.

```
Gateway                          Provider (Claude Code CLI)
  |                                    |
  |-- Context(session_id: None) ------>|  First call: full prompt
  |<-- MessageMetadata(session_id) ----|  Returns session_id
  |                                    |
  |  [stores session_id per user]      |
  |                                    |
  |-- Context(session_id: "abc") ----->|  Continuation: minimal prompt
  |<-- MessageMetadata(session_id) ----|  Same or new session_id
  |                                    |
  |  [/forget or error]                |
  |  [clears session_id]               |
  |                                    |
  |-- Context(session_id: None) ------>|  Fresh full prompt (new session)
```

**Scope:** Claude Code CLI only. HTTP-based providers always receive the full context — they have no session mechanism.

---

## Background Loops

Four independent loops run alongside message processing:

### Scheduler (`gateway/scheduler.rs`)
Polls `scheduled_tasks` table every 60 seconds. Delivers due **reminders** as channel messages. Executes due **action tasks** via a full provider call with tool/MCP access — the AI acts autonomously and reports the result.

### Heartbeat (`gateway/heartbeat.rs`)
Clock-aligned periodic execution (default 30 min, fires at clean boundaries like :00/:30). Reads `~/.omega/prompts/HEARTBEAT.md` checklist, classifies items by domain, executes related groups in parallel via Opus. Processes response markers independently per group. Suppresses `HEARTBEAT_OK` when no meaningful content remains.

### Summarizer (`gateway/summarizer.rs`)
Finds idle conversations (no activity for configured duration), generates a summary via the AI, and closes them. Summaries are stored for future context recall.

### CLAUDE.md Maintenance (`claudemd.rs`)
Deploys a workspace `CLAUDE.md` template on startup, appends dynamic content (skills/projects tables) via `claude -p`. Refreshes every 24 hours.

---

## Efficiency Summary

| Optimization | Savings |
|--------------|---------|
| Session persistence (`--resume`) | ~90-99% tokens on continuation messages |
| Keyword-gated prompt sections | ~55-70% fewer tokens per message |
| Keyword-gated DB queries | Skip expensive queries when not relevant |
| Sonnet classification before Opus | Cheap routing, expensive model only when needed |
| Per-sender serialization | No race conditions, no duplicate provider calls |
| Auto-resume on max_turns | Long tasks complete without user intervention |
| Delayed status updates | No noise for fast responses (<15s) |
