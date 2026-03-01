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

**Files:** `backend/crates/omega-channels/src/telegram/polling.rs`

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

**File:** `backend/src/gateway/mod.rs`

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

**File:** `backend/src/gateway/pipeline.rs`

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

**File:** `backend/crates/omega-memory/src/store/context.rs`

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

The gateway **always builds** the full system prompt first (needed as fallback and for the first message). The prompt is assembled from sections:

| Section | Injection | Content |
|---------|-----------|---------|
| `## Identity` | First message only | Who OMEGA is, behavioral examples |
| `## Soul` | First message only | Personality, tone, boundaries |
| `## System` | First message only | Core rules, marker quick-reference |
| Project awareness | Always (~40-50 tokens) | Available project names, creation/activation hints |
| Active project ROLE.md | Always when a project is active | Full project instructions from `~/.omega/projects/<name>/ROLE.md` |
| `## Scheduling` | If scheduling keywords match | Task/reminder rules |
| `## Projects` | If project keywords match | Project management conventions |
| `## Meta` | If meta keywords match | Skill improvement, heartbeat, WhatsApp |

The core sections (Identity + Soul + System) are sent **once** on the first message of a conversation. On subsequent messages within the same CLI session, they are completely replaced by a minimal context update (see 3.10 below). The conditional sections (Scheduling, Projects management, Meta) are keyword-gated on every message — included only when relevant keywords appear. Project awareness (~40-50 tokens) and active project ROLE.md are always injected regardless of keywords.

**Token savings:** ~55-70% on first messages (conditional sections skipped), ~90-99% on continuations (entire prompt replaced).

### 3.9 Skill Trigger Matching

Scan the message for trigger keywords defined in `~/.omega/skills/*/SKILL.md`. If matched, attach the skill's MCP server config to the context — the provider will activate it.

### 3.10 Session Check (Claude Code CLI only)

If an active CLI session exists for this sender:
- Set `context.session_id` (provider will pass `--resume`)
- **Replace the entire system prompt** (identity + soul + system + everything) with a minimal context update:
  ```
  Current time: 2026-02-22 14:30 CET
  [+ scheduling section, only if scheduling keywords match]
  [+ projects section, only if project keywords match]
  [+ meta section, only if meta keywords match]
  [+ project awareness hint, if projects exist]
  [+ active project ROLE.md, if a project is active]
  ```
- Clear conversation history (the CLI session already has it)
- **Token savings:** ~90-99% — the CLI session already carries the full identity/soul/system from the first message

If no session exists (first message, after `/forget`, after error), send the full prompt + history.

---

## Phase 4: Classification & Routing

**File:** `backend/src/gateway/routing.rs`

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

**File:** `backend/crates/omega-providers/src/claude_code/`

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

**File:** `backend/src/gateway/routing.rs`, `backend/src/gateway/process_markers.rs`

### 6.1 Capture Session ID

Store the returned `session_id` in SQLite via `store_session(channel, sender_id, project, session_id)`, keyed by `(channel, sender_id, project)`. The next message from this sender in the same project will use `--resume` with this ID. Sessions survive process restarts and are isolated per project.

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
| `PROJECT_ACTIVATE: name` | Switch active project + remove `.disabled` marker |
| `PROJECT_DEACTIVATE` | Deactivate project + create `.disabled` marker |
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

The Claude Code CLI supports session continuity to avoid re-sending the full system prompt on every message. Sessions are persisted in SQLite (the `project_sessions` table) and scoped per `(channel, sender_id, project)`.

### How It Works

1. **First message:** Full context (system prompt + history + user message) with `session_id: None`. The CLI returns a `session_id` in its JSON response.

2. **Subsequent messages:** Gateway sets `Context.session_id`. The provider passes `--resume <session_id>`. System prompt is replaced with a minimal context update (current time + keyword-gated sections only). History is cleared (already in the CLI session). **Token savings: ~90-99%.**

3. **Invalidation:** Session ID is cleared on `/forget`, `FORGET_CONVERSATION` marker, idle timeout, or provider error. `/forget` clears the session for the current project only; a full sender reset clears all project sessions.

4. **Fallback:** If a session-based call fails, the gateway retries with a fresh full-context call. The user never sees the failure.

### Project-Scoped Sessions

Sessions are stored in SQLite via the `project_sessions` table (migration 012), keyed by `(channel, sender_id, project)`. This replaces the previous in-memory `HashMap<String, String>`.

**Key behaviors:**
- **Restart survival:** Sessions persist across process restarts — no lost context on service bounce.
- **Project isolation:** Each project gets its own CLI session. Switching from project A to project B does not kill project A's session. Returning to project A resumes via `--resume`.
- **Conversations are also project-scoped:** The `conversations` table carries a `project` column, so conversation history is isolated per project.

```
Gateway                          SQLite                Provider (Claude Code CLI)
  |                                 |                        |
  |-- get_session(ch, sid, proj) -->|                        |
  |<-- None                         |                        |
  |                                 |                        |
  |-- Context(session_id: None) ----|----------------------->|  First call: full prompt
  |<-- MessageMetadata(session_id) -|------------------------|  Returns session_id
  |                                 |                        |
  |-- store_session(ch,sid,proj,s)->|  Persisted to SQLite   |
  |                                 |                        |
  |-- get_session(ch, sid, proj) -->|                        |
  |<-- Some("abc")                  |                        |
  |                                 |                        |
  |-- Context(session_id: "abc") ---|----------------------->|  Continuation: minimal prompt
  |<-- MessageMetadata(session_id) -|------------------------|  Same or new session_id
  |                                 |                        |
  |  [/forget or error]             |                        |
  |-- clear_session(ch,sid,proj) -->|  Removed from SQLite   |
  |                                 |                        |
  |-- Context(session_id: None) ----|----------------------->|  Fresh full prompt (new session)
```

**Scope:** Claude Code CLI only. HTTP-based providers always receive the full context — they have no session mechanism.

---

## Webhook Inbound Path

The HTTP API server (`api.rs`) exposes `POST /api/webhook` for external tools. Two modes:

- **Direct mode**: Builds an `OutgoingMessage` (same pattern as scheduler reminders) and calls `channel.send()`. Returns 200 immediately. Audited in the handler.
- **AI mode**: Builds a synthetic `IncomingMessage` with the real channel name and a valid `sender_id` (from `allowed_users`), then sends it through the gateway's `mpsc` channel via `tx.send()`. Returns 202. The message enters the standard pipeline (Phase 2 onward) and the AI response is delivered to the messaging channel.

The `tx` sender clone is passed to `api::serve()` before `drop(tx)` in `gateway/mod.rs`, ensuring the main loop still exits cleanly when all channels close.

See [webhook.md](webhook.md) for the full API contract, curl examples, and integration guide.

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

## Project-Scoped Learning

OMEGA's reward-based learning system is project-aware. Every outcome (`REWARD:` marker), lesson (`LESSON:` marker), and scheduled task carries a `project` field that isolates learning per project context.

### How `active_project` Flows Through the Pipeline

1. **Keyword detection** resolves the user's `active_project` fact from the database
2. **Context building** passes `active_project` to `build_context()`, which loads project-scoped outcomes and lessons
3. **System prompt** includes a `[Project: name]` badge when a project is active
4. **Marker processing** passes `active_project` to `process_markers()`, which tags all emitted `REWARD:`, `LESSON:`, `SCHEDULE:`, and `SCHEDULE_ACTION:` markers with the project
5. **Heartbeat loop** runs per-project heartbeats for projects that have their own `HEARTBEAT.md` and do NOT have a `.disabled` marker file (filesystem-based discovery). `/project off` creates `.disabled` (stops heartbeat); `/project <name>` activates by removing `.disabled`
6. **Action tasks** inherit the project scope and execute with the project's `ROLE.md` context

### Isolation Model

| Scope | Outcomes | Lessons | Tasks |
|-------|----------|---------|-------|
| General (`project = ""`) | Visible in all contexts | Visible in all contexts | Execute without ROLE.md |
| Project-specific (`project = "omega-trader"`) | Only visible when that project is active | Visible when that project is active, plus general | Execute with project's ROLE.md |

Key rule: **project lessons don't leak into general context, but general lessons always flow into project context**. When a project is active, `build_system_prompt()` shows project-specific lessons first (labeled `[project-name]`), then fills the remaining budget with general lessons. This ensures project-specific behavioral rules take priority while general knowledge remains available.

---

## Multi-Agent Pipeline Architecture

Omega uses a **topology-driven sequential agent pipeline with bounded corrective loops and file-mediated handoffs** — a pattern applied in two contexts:

| Context | Where | Agents | Trigger |
|---------|-------|--------|---------|
| **Runtime builds** | `gateway/builds.rs` | 7 phases via Claude Code CLI `--agent` | Keyword match (`BUILDS_KW`) or AI emits `BUILD_PROPOSAL:` marker |
| **Dev-time workflows** | `.claude/commands/workflow-*.md` | Up to 10 steps via Claude Code subagents | Developer runs `/workflow:feature`, etc. |

Both use the same underlying pattern. The only difference is the execution environment: runtime builds run inside the OMEGA process (Rust orchestrator), dev-time workflows run inside Claude Code's agent system (markdown command files).

### Topology Format

Runtime builds are defined by a **TOPOLOGY.toml** file that describes the phase sequence, agent assignments, model tiers, retry limits, and validation rules. The default "development" topology lives at `~/.omega/topologies/development/`:

```
~/.omega/topologies/development/
  TOPOLOGY.toml              <-- Phase sequence and configuration
  agents/
    build-analyst.md          <-- Agent instructions (one file per agent)
    build-architect.md
    build-test-writer.md
    build-developer.md
    build-qa.md
    build-reviewer.md
    build-delivery.md
    build-discovery.md
```

The topology TOML defines each phase with its agent, model tier, execution type, retry rules, and validation:

```toml
[topology]
name = "development"
description = "Default 7-phase TDD build pipeline"
version = 1

[[phases]]
name = "analyst"
agent = "build-analyst"
model_tier = "complex"
max_turns = 25
phase_type = "parse-brief"

[[phases]]
name = "qa"
agent = "build-qa"
model_tier = "complex"
phase_type = "corrective-loop"

[phases.pre_validation]
type = "file_patterns"
patterns = [".rs", ".py", ".js", ".ts"]

[phases.retry]
max = 3
fix_agent = "build-developer"
```

**Four phase types** dispatch to existing Rust functions:

| PhaseType | Behavior | Used By |
|-----------|----------|---------|
| `standard` | Run agent, check for error, proceed | architect, test-writer, developer |
| `parse-brief` | Run agent, parse output via `parse_project_brief()`, create project dir | analyst |
| `corrective-loop` | Run agent, parse result, on fail re-invoke fix_agent, retry up to max | qa, reviewer |
| `parse-summary` | Run agent, parse output via `parse_build_summary()`, format final message | delivery |

**Bundling and deployment:** The topology files are compiled into the binary via `include_str!()` from `topologies/development/` in the source repo. On the first build request, they are deployed to `~/.omega/topologies/development/` if the directory does not exist. Existing files are never overwritten, preserving user customizations.

### Pattern: Sequential Agent Chain

Each agent in the chain is a **specialist with a single responsibility**. Agents execute in the order defined by the topology's `[[phases]]` array:

```
Analyst -> Architect -> Test Writer -> Developer -> QA -> Reviewer -> Delivery
```

No agent knows about the others. No agent calls another agent directly. The **orchestrator** (`builds.rs`) reads the topology and dispatches each phase based on its `phase_type`.

### Topology-Driven Orchestrator

The orchestrator in `builds.rs` loads the topology once per build request and iterates over its phases:

```
load_topology("development")
        |
        v
for phase in topology.phases:
  1. Send localized progress message (phase name -> i18n lookup)
  2. Run pre-validation if configured (file_exists or file_patterns)
  3. Dispatch based on phase_type (Standard/ParseBrief/CorrectiveLoop/ParseSummary)
  4. Run post-validation if configured (check required output files)
  5. Update orchestrator state (brief, project_dir, completed phases)
```

An `OrchestratorState` struct carries state between phases: the parsed project brief, project directory path, and list of completed phases (for chain state on failure).

### Communication: File-Mediated Handoffs

Agents do not share memory, state, or context windows. All inter-agent communication happens through **files on disk**:

```
+----------+    writes specs/    +-----------+    reads specs/     +-------------+
| Analyst  | ------------------> | Architect | ------------------> | Test Writer |
+----------+    requirements     +-----------+    writes specs/     +-------------+
                                                  architecture           |
                                                                         | reads specs/
                                                                         | writes tests/
                                                                         v
+----------+    reads code/      +-----------+    reads tests/     +-----------+
| Reviewer | <------------------ |    QA     | <------------------ | Developer |
+----------+    writes reviews/  +-----------+    writes qa/       +-----------+
                                                                    reads tests/
                                                                    writes code/
```

**Why files, not in-memory state:**
- Each agent gets a **fresh context window** -- no accumulated token bloat
- Artifacts are **inspectable** -- you can read what any agent produced at any step
- The chain is **resumable** -- if step 4 fails, re-run from step 4 with the same files
- **No coupling** -- swap an agent's implementation without touching others

### Bounded Corrective Loops

The pipeline is not purely linear. Phases with `phase_type = "corrective-loop"` have **retry loops** where a verifier can send work back to a fix agent:

```
Developer <--fix--> QA        (max from topology, default 3)
Developer <--fix--> Reviewer  (max from topology, default 2)
```

Retry limits and the fix agent are configured per phase in the topology's `[phases.retry]` section. Every loop has a **hard iteration cap**. If the cap is reached, the chain **stops and escalates to the user** rather than spinning indefinitely. This prevents:
- Infinite loops from contradictory requirements
- Cost explosion from agents disagreeing
- Silent failures from agents that "agree to disagree"

### Pre/Post Validation

Validation rules are defined in the topology per phase:

- **Pre-validation** (`[phases.pre_validation]`): Runs before the phase. Two types:
  - `file_exists`: Check that specific files exist in the project directory
  - `file_patterns`: Check that at least one file matching the patterns exists in the directory tree
- **Post-validation** (`post_validation`): Runs after the phase. A list of file paths that must exist after the phase completes.

If validation fails, the chain stops and a chain state file is saved for inspection.

### Self-Healing Post-Commit Audit

After the main chain commits, a **bounded audit loop** automatically verifies the result:

```
[Main chain commits]
        |
        v
Reviewer (scoped audit) --> AUDIT-VERDICT: clean --> DONE
        |
        +---> AUDIT-VERDICT: requires-fix
                    |
                    v
            Analyst -> Developer -> QA (reduced fix cycle)
                    |
                    +---> Re-audit (round 2, max 2 rounds)
```

The audit loop uses a **machine-parseable verdict** (`AUDIT-VERDICT: clean` or `AUDIT-VERDICT: requires-fix`) to branch deterministically -- no ambiguous natural language parsing.

### Why Choreography, Not Orchestration

The orchestrator does **not** make dynamic routing decisions mid-chain. It runs the fixed sequence defined in the topology. This is **choreography** (each agent knows what artifact to read/write) rather than **orchestration** (a central brain deciding who runs next based on runtime state).

The only dynamic behavior is the corrective loops, and even those follow a fixed pattern (verifier rejects -> implementer retries -> verifier re-checks) with topology-defined bounds.

**Why this matters:**
- The pipeline is **predictable** -- same topology always produces the same agent sequence
- **Debugging** is straightforward -- check which file artifact is wrong, that's the agent that failed
- **Cost is bounded** -- you can calculate worst-case agent calls from the topology before the chain starts
- **Customizable** -- change retry limits, swap agents, or reorder phases by editing TOML, not Rust code

### Runtime vs Dev-Time Comparison

| Aspect | Runtime (builds.rs) | Dev-Time (workflow commands) |
|--------|---------------------|------------------------------|
| Orchestrator | Rust code (`handle_build_request()`) | Markdown command file |
| Pipeline definition | `TOPOLOGY.toml` + agent `.md` files | Inline in workflow command markdown |
| Agent invocation | `claude --agent <name>` subprocess | Claude Code subagent system |
| Agent definitions | `~/.omega/topologies/<name>/agents/*.md` | `.claude/agents/*.md` files |
| Artifact location | `~/.omega/workspace/builds/<name>/` | `specs/`, `docs/`, source code |
| QA retry cap | From topology (default 3, fatal) | 3 iterations (escalate) |
| Reviewer retry cap | From topology (default 2, fatal) | 2 iterations (escalate) |
| Post-commit audit | Not applicable (runtime) | 2 rounds max, auto-fix cycle |
| User interaction | Progress messages via Telegram/WhatsApp | Terminal output |

### Agent RAII Guard (Runtime Only)

Runtime builds use an **RAII guard** (`AgentFilesGuard`) that writes agent `.md` files from the loaded topology to `~/.omega/workspace/.claude/agents/` before the first phase and **automatically deletes them on drop**. The guard reads agent content from the `LoadedTopology` struct (which loaded them from `~/.omega/topologies/<name>/agents/`). This ensures agent definitions don't leak between builds or persist after failures.

The guard is reference-counted per directory: multiple concurrent builds share the same agent files, and only the last guard to drop performs cleanup.

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

---

## Concrete Example: Scheduling a Reminder

To see all the phases working together, let's trace what happens when you send:

> "Schedule for tomorrow to call Juan at 5pm"

### 1. Keyword Detection (`gateway/keywords.rs`)

The message is lowercased and scanned against keyword arrays. Two hits:

- `"schedule"` matches `SCHEDULING_KW`
- `"tomorrow"` matches `SCHEDULING_KW`

This triggers a cascade of flags:

```
needs_scheduling = true     ← "schedule" + "tomorrow" matched
needs_tasks      = true     ← automatically true when needs_scheduling is true
needs_profile    = true     ← automatically true when needs_scheduling is true
                              (timezone/location needed for UTC conversion)
```

### 2. Prompt Injection — What Gets Appended

Because `needs_scheduling = true`, the `## Scheduling` section from `SYSTEM_PROMPT.md` is appended to the system prompt. This teaches Claude **how** to create tasks:

```
You have a built-in scheduler — an internal task queue stored in your own
database, polled every 60 seconds...

Initial due_at rule: Always set the first due_at to the NEXT upcoming
occurrence... The scheduler uses UTC — convert the user's local time to
UTC before emitting the marker.

Reminders: To schedule a reminder, use this marker on its own line:
SCHEDULE: <description> | <ISO 8601 datetime> | <once/daily/weekly/...>

Action Tasks: For tasks that require you to EXECUTE an action:
SCHEDULE_ACTION: <what to do> | <ISO 8601 datetime> | <once/daily/...>

Task awareness (MANDATORY): Before creating ANY new reminder, you MUST
review the "User's scheduled tasks" section in your context...
```

Without the scheduling keywords, this entire block is **not sent** — Claude doesn't even know the marker syntax exists. This is the core of the conditional injection: teach Claude capabilities only when they're relevant.

### 3. Context Enrichment — What Gets Loaded from SQLite

Because the keyword cascade set `needs_profile` and `needs_tasks`, `build_context()` loads additional data:

**User profile** (because `needs_profile = true` — timezone needed for UTC conversion):
```
User profile:
- name: Daniel
- timezone: Europe/Lisbon
- location: Portugal
```

**Pending tasks** (because `needs_tasks = true` — Claude must check for duplicates before creating):
```
User's scheduled tasks:
- [a1b2c3d4] Daily standup reminder (due: 2026-02-24 09:00, daily)
- [e5f6g7h8] Call dentist [action] (due: 2026-02-25 10:00, once)
```

**Learned lessons** (always loaded, tiny):
```
Learned behavioral rules:
- [scheduling] User prefers reminders 15 minutes before, not at the exact time
```

### 4. What Claude Actually Sees

The full assembled prompt sent to `claude -p`:

```
[Identity section]           ← first message only (skipped on session continuation)
[Soul section]               ← first message only
[System section]             ← first message only

[Scheduling section]         ← INJECTED because "schedule" keyword matched

User profile:                ← INJECTED because scheduling needs timezone
- name: Daniel
- timezone: Europe/Lisbon

User's scheduled tasks:      ← INJECTED so Claude checks for duplicates
- [a1b2c3d4] Daily standup reminder (due: 2026-02-24 09:00, daily)

Learned behavioral rules:    ← always injected
- [scheduling] User prefers reminders 15 minutes before

IMPORTANT: Always respond in English.

--- conversation history ---
user: Schedule for tomorrow to call Juan at 5pm
```

### 5. What Claude Responds (raw, before processing)

```
I'll set that up for you — a reminder to call Juan tomorrow at 5pm.

SCHEDULE: Call Juan | 2026-02-24T17:00:00Z | once
REWARD: +1|scheduling|straightforward reminder request
```

### 6. What the Gateway Does (marker processing)

1. **Extract** `SCHEDULE:` → parse description, datetime, repeat → call `memory.create_task()` → insert row in `scheduled_tasks` table
2. **Extract** `REWARD:` → parse score, domain, lesson → call `memory.store_outcome()` → insert row in `outcomes` table
3. **Strip** both marker lines from the text
4. **Run** `strip_all_remaining_markers()` safety net (catches anything missed)

### 7. What You See on Telegram

**Message 1** (the response):
```
I'll set that up for you — a reminder to call Juan tomorrow at 5pm.
```

**Message 2** (task confirmation from `task_confirmation.rs`):
```
✓ Reminder created: Call Juan — Feb 24 at 5:00 PM (once)
```

The markers are invisible. The scheduling instructions were only injected because your message contained the right keywords. If you had said "hello, how are you?" — none of the scheduling section, profile data, or pending tasks would have been loaded or sent to Claude.

### Comparison: What a Simple "Hello" Triggers

For contrast, sending just "hello" matches **no** keyword arrays:

```
needs_scheduling = false
needs_tasks      = false
needs_profile    = false
needs_recall     = false
needs_outcomes   = false
```

Claude receives only the core sections (Identity + Soul + System) with no conditional sections, no profile, no tasks, no summaries. The prompt is ~55-70% smaller than the scheduling example above.
