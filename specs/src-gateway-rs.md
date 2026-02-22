# Specification: src/gateway/ (Directory Module)

## File Path
`src/gateway/` (directory module with 9 files)

## Refactoring History
- **2026-02-20:** Marker extraction/parsing/stripping functions (40+) extracted into `src/markers.rs`. See `specs/src-markers-rs.md`.
- **2026-02-20:** Task confirmation logic extracted into `src/task_confirmation.rs`. See `specs/src-task-confirmation-rs.md`.
- **2026-02-22:** Gateway refactored from a single `src/gateway.rs` (3,449 lines) into a `src/gateway/` directory module with 9 files. All struct fields use `pub(super)` visibility. No changes to `main.rs` or public API.

## Module Structure

| File | Lines (prod) | Responsibility |
|------|-------------|----------------|
| `mod.rs` | ~417 | Struct Gateway, `new()`, `run()`, `dispatch_message()`, `shutdown()`, `send_text()`, tests |
| `keywords.rs` | ~310 | Constants (`MAX_ACTION_RETRIES`, `SCHEDULING_KW`, `RECALL_KW`, `TASKS_KW`, `PROJECTS_KW`, `PROFILE_KW`, `OUTCOMES_KW`, `META_KW`, `SYSTEM_FACT_KEYS`), `kw_match()`, `is_valid_fact()`, tests |
| `summarizer.rs` | ~276 | `summarize_and_extract()`, `background_summarizer()`, `summarize_conversation()`, `handle_forget()` |
| `scheduler.rs` | ~417 | `scheduler_loop()` |
| `heartbeat.rs` | ~370 | `heartbeat_loop()`, `classify_heartbeat_groups()`, `execute_heartbeat_group()`, `process_heartbeat_markers()`, `build_enrichment()`, `build_system_prompt()`, `send_heartbeat_result()` |
| `pipeline.rs` | ~423 | `handle_message()`, `build_system_prompt()` |
| `routing.rs` | ~417 | `classify_and_route()`, `execute_steps()`, `handle_direct_response()` |
| `auth.rs` | ~167 | `check_auth()`, `handle_whatsapp_qr()` |
| `process_markers.rs` | ~498 | `process_markers()`, `process_purge_facts()`, `process_improvement_markers()`, `send_task_confirmation()` |

All submodules access `Gateway` fields via `pub(super)` visibility (module-internal, not public API).

## Purpose
Gateway is the central event loop orchestrator that connects messaging channels, memory persistence, and AI providers. It implements the complete message processing pipeline with authentication, sanitization, context building, provider delegation, audit logging, and graceful shutdown.

## Architecture Overview

### Core Responsibility
The gateway manages the asynchronous event loop that processes incoming messages through a deterministic pipeline:

```
Message → Auth → Sanitize → Command Check → Typing → Keyword Detection (kw_match) →
Conditional Prompt Compose (Identity+Soul+System always, scheduling/projects/meta if keywords match) →
Context (build_context with ContextNeeds gating recall/tasks DB queries) → MCP Trigger Match →
Session Check (strip heavy prompt if continuation) → Classify & Route (always, model selection) →
Provider (MCP settings write → CLI → MCP cleanup) → Session Capture → Memory Store → Audit Log → Send
```

The gateway runs continuously, listening for messages from registered channels via an mpsc channel, and spawns a background task for periodic conversation summarization.

## Data Structures

### Gateway Struct (defined in `mod.rs`)
```rust
pub struct Gateway {
    pub(super) provider: Arc<dyn Provider>,              // AI backend (Claude Code, Anthropic, etc.)
    pub(super) channels: HashMap<String, Arc<dyn Channel>>,  // Messaging platforms (Telegram, WhatsApp)
    pub(super) memory: Store,                             // SQLite conversation/fact storage
    pub(super) audit: AuditLogger,                        // Event audit trail
    pub(super) auth_config: AuthConfig,                   // Authentication rules
    pub(super) channel_config: ChannelConfig,             // Per-channel configuration
    pub(super) heartbeat_config: HeartbeatConfig,         // Periodic AI check-in settings
    pub(super) scheduler_config: SchedulerConfig,         // Scheduled task delivery settings
    pub(super) prompts: Prompts,                          // Externalized prompts & welcome messages
    pub(super) model_fast: String,                        // Model for DIRECT/simple messages (e.g., "claude-sonnet-4-6")
    pub(super) model_complex: String,                     // Model for multi-step/complex messages (e.g., "claude-opus-4-6")
    pub(super) uptime: Instant,                           // Server start time
    pub(super) active_senders: Mutex<HashMap<String, Vec<IncomingMessage>>>,  // Per-sender message buffer for non-blocking dispatch
    pub(super) heartbeat_interval: Arc<AtomicU64>,        // Dynamic heartbeat interval (minutes), updated via HEARTBEAT_INTERVAL: marker
    pub(super) cli_sessions: Arc<std::sync::Mutex<HashMap<String, String>>>,  // CLI session cache per sender
}
```

**Fields:**
- `provider`: Shared reference to the configured AI provider. Must implement the `Provider` trait.
- `channels`: Map of channel names to channel implementations. Each channel independently listens for messages.
- `memory`: Shared SQLite-backed store for conversation history, facts, and metadata.
- `audit`: Logger that records all interactions for security and debugging.
- `auth_config`: Global authentication policy (enabled flag, deny message).
- `channel_config`: Per-channel settings (Telegram allowed_users list).
- `heartbeat_config`: Periodic heartbeat check-in configuration (interval, active hours, channel, reply target).
- `scheduler_config`: Scheduled task delivery configuration (enabled flag, poll interval).
- `prompts`: Externalized prompts and welcome messages, loaded from `~/.omega/prompts/SYSTEM_PROMPT.md` and `~/.omega/prompts/WELCOME.toml` at startup. Falls back to hardcoded defaults if files are missing. The `Prompts` struct has 3 conditional sections — `scheduling`, `projects_rules`, `meta` — parsed from `## Scheduling`, `## Projects`, `## Meta` sections in `SYSTEM_PROMPT.md`. These are only injected when keyword detection triggers them (see Stage 4: Build Context).
- `model_fast`: Model identifier used for DIRECT/simple messages (e.g., `"claude-sonnet-4-6"`). Set from `ClaudeCodeConfig.model` at startup. Injected into `context.model` by `classify_and_route()` for direct-path messages.
- `model_complex`: Model identifier used for multi-step/complex messages (e.g., `"claude-opus-4-6"`). Set from `ClaudeCodeConfig.model_complex` at startup. Injected into `context.model` by `classify_and_route()` for step-based execution.
- `uptime`: Tracks server start time for uptime calculations in commands.
- `active_senders`: A `Mutex<HashMap<String, Vec<IncomingMessage>>>` that tracks which senders currently have an active provider call in flight. When a new message arrives for a sender that is already being processed, the message is buffered here. After the active call completes, buffered messages are dispatched in order.
- `heartbeat_interval`: An `Arc<AtomicU64>` holding the current heartbeat interval in minutes. Initialized from `heartbeat_config.interval_minutes` and shared with the heartbeat loop and scheduler loop. Updated at runtime via `HEARTBEAT_INTERVAL:` markers. Resets to config value on restart.
- `cli_sessions`: Thread-safe map of `channel:sender_id` to CLI session_id. Used for session-based prompt persistence -- subsequent messages in the same conversation skip the heavy system prompt (~2282 tokens) and send only a minimal context update. Cleared on `/forget`, `FORGET_CONVERSATION` marker, and idle conversation timeout.

## Token-Efficient Prompt Architecture (Keyword-Gated Conditional Injection)

The gateway reduces system prompt token overhead by ~55% for typical messages. Instead of injecting all prompt sections unconditionally, it detects keywords in the user's message and conditionally injects only the relevant sections.

### Keyword Constants

| Constant | Triggers | Example Keywords |
|----------|----------|-----------------|
| `SCHEDULING_KW` | Scheduling rules injection (`prompts.scheduling`) | "remind", "schedule", "alarm", "tomorrow", "cancel", "recurring", multilingual variants |
| `RECALL_KW` | Semantic recall DB query (FTS5 related past messages) | "remember", "last time", "you said", "we discussed", multilingual variants |
| `TASKS_KW` | Pending tasks DB query | "task", "reminder", "pending", "scheduled", "my tasks", multilingual variants |
| `PROJECTS_KW` | Projects rules injection (`prompts.projects_rules`) + active project instructions | "project", "activate", "deactivate", multilingual variants |
| `PROFILE_KW` | User profile (facts) injection into prompt | "who am i", "my name", "about me", "my profile", "what do you know", multilingual variants |
| `OUTCOMES_KW` | Recent reward outcomes injection | "how did i", "how am i doing", "reward", "outcome", "feedback", "performance", multilingual variants |
| `META_KW` | Meta rules injection (`prompts.meta`) — skill improvement, bug reporting, WhatsApp, personality, purge | "skill", "improve", "bug", "whatsapp", "qr", "personality", "forget", "purge" |

All keyword lists include multilingual variants (Spanish, Portuguese, French, German, Italian, Dutch, Russian) for the 8 supported languages.

### `fn kw_match(msg_lower: &str, keywords: &[&str]) -> bool` (keywords.rs)
**Purpose:** Check if any keyword in the list is a substring of the lowercased message.

**Logic:** `keywords.iter().any(|kw| msg_lower.contains(kw))` — simple substring match, no tokenization or word boundaries.

### `ContextNeeds` Struct (from `omega_core::context`)
**Purpose:** Gates expensive DB queries and prompt injection in `build_context()`. Fields:
- `recall: bool` — Load semantic recall (FTS5 related past messages). Triggered by `RECALL_KW`.
- `pending_tasks: bool` — Load and inject pending scheduled tasks. Triggered by `SCHEDULING_KW` or `TASKS_KW`.
- `profile: bool` — Inject user facts into prompt. Triggered by `PROFILE_KW` or scheduling/recall/tasks (needs identity context). Facts are always loaded (onboarding/language), but only injected when `true`.
- `summaries: bool` — Load and inject recent conversation summaries. Triggered by `RECALL_KW` (same gate as recall).
- `outcomes: bool` — Load and inject recent reward outcomes. Triggered by `OUTCOMES_KW`.

Default: all `true` (full context). The gateway sets them based on keyword detection before calling `build_context()`. Lessons are always loaded regardless of flags (tiny, high behavioral value).

### Keyword Detection Logic (in `handle_message`)
```rust
let msg_lower = clean_incoming.text.to_lowercase();
let needs_scheduling = kw_match(&msg_lower, SCHEDULING_KW);
let needs_recall    = kw_match(&msg_lower, RECALL_KW);
let needs_tasks     = needs_scheduling || kw_match(&msg_lower, TASKS_KW);
let needs_projects  = kw_match(&msg_lower, PROJECTS_KW);
let needs_meta      = kw_match(&msg_lower, META_KW);
let needs_profile   = kw_match(&msg_lower, PROFILE_KW)
    || needs_scheduling || needs_recall || needs_tasks;
let needs_summaries = needs_recall;
let needs_outcomes  = kw_match(&msg_lower, OUTCOMES_KW);
```

**Key rules:**
- `needs_tasks` is `true` when either scheduling or task keywords match (scheduling implies task awareness).
- `needs_projects` is keyword-only — active project instructions are injected only when project keywords match.
- `needs_profile` cascades from scheduling/recall/tasks (identity context is useful for timezone, past context, and task ownership).
- `needs_summaries` tracks `needs_recall` — summaries are past conversation context.
- `needs_outcomes` is keyword-only (feedback/performance queries).
- Lessons are always injected regardless of keywords (tiny, high behavioral value).
- Core sections (Identity, Soul, System) are always injected regardless of keywords.
- Provider name, current time, and platform hint are always injected.
- Heartbeat interval is now only injected when heartbeat keywords match (moved inside the heartbeat keyword check block).

### Prompt Composition (Conditional Sections)
```
Always: Identity + Soul + System + provider info + time + platform hint + lessons
If SCHEDULING_KW:  + prompts.scheduling
If PROJECTS_KW:    + prompts.projects_rules + active project instructions
If META_KW:        + prompts.meta
If heartbeat KW:   + heartbeat checklist + heartbeat pulse
If PROFILE_KW/scheduling/recall/tasks: + user profile (facts)
If RECALL_KW:      + summaries + semantic recall
If OUTCOMES_KW:    + recent outcomes
If sandbox:        + sandbox constraint (unchanged)
```

### `Prompts` Struct Fields (Conditional Sections)
| Field | Parsed From | Injected When |
|-------|------------|---------------|
| `scheduling` | `## Scheduling` in SYSTEM_PROMPT.md | `SCHEDULING_KW` matches |
| `projects_rules` | `## Projects` in SYSTEM_PROMPT.md | `PROJECTS_KW` matches |
| `meta` | `## Meta` in SYSTEM_PROMPT.md | `META_KW` matches |

## Functions

### Public Methods (mod.rs)

#### `new(provider, channels, memory, auth_config, channel_config, heartbeat_config, scheduler_config, prompts, model_fast, model_complex) -> Self`
**Purpose:** Construct a new gateway instance.

**Parameters:**
- `provider: Arc<dyn Provider>` - The AI backend (typically Claude Code CLI provider).
- `channels: HashMap<String, Arc<dyn Channel>>` - Map of initialized channel implementations.
- `memory: Store` - SQLite store initialized with database pool.
- `auth_config: AuthConfig` - Authentication configuration.
- `channel_config: ChannelConfig` - Per-channel configuration.
- `heartbeat_config: HeartbeatConfig` - Heartbeat check-in configuration.
- `scheduler_config: SchedulerConfig` - Scheduled task delivery configuration.
- `prompts: Prompts` - Externalized prompts and welcome messages (loaded from `~/.omega/` files or defaults).
- `model_fast: String` - Model identifier for DIRECT/simple messages (from `ClaudeCodeConfig.model`).
- `model_complex: String` - Model identifier for multi-step/complex messages (from `ClaudeCodeConfig.model_complex`).

**Returns:** New `Gateway` instance.

**Logic:**
- Creates an `AuditLogger` from the memory pool.
- Captures current time as `uptime`.
- Stores all parameters as instance fields.

**Error Handling:** None (infallible). Has `#[allow(clippy::too_many_arguments)]` attribute.

#### `async fn run(self: Arc<Self>) -> anyhow::Result<()>`
**Purpose:** Start the gateway event loop and run until shutdown signal.

**Parameters:** None (takes `self: Arc<Self>` instead of `&mut self`).

**Returns:** `anyhow::Result<()>` (Ok on graceful shutdown, Err on critical failure).

**Note:** The method takes `self: Arc<Self>` to allow the gateway to be shared across spawned tasks for non-blocking message dispatch. The gateway is wrapped in `Arc::new()` in `main.rs`.

**Logic:**
1. Log gateway initialization with provider name, channel names, and auth status.
1b. Purge orphaned inbox files from previous runs via `purge_inbox(&self.data_dir)`.
1c. If provider is `"claude-code"`, spawn `crate::claudemd::ensure_claudemd()` as a background task to create `~/.omega/workspace/CLAUDE.md` if missing.
2. Create an mpsc channel with capacity 256 for incoming messages.
3. For each registered channel:
   - Call `channel.start()` to get a receiver for that channel's messages.
   - Spawn a background task that forwards all messages from the channel receiver to the gateway's main mpsc channel.
   - Log successful channel start.
4. Drop the sender to signal EOF when all channels close.
5. Spawn a background summarization task via `tokio::spawn(background_summarizer())`.
6. If `scheduler_config.enabled`, spawn `scheduler_loop()` via `tokio::spawn()`. Store handle as `Option<JoinHandle<()>>`.
7. If `heartbeat_config.enabled`, spawn `heartbeat_loop()` via `tokio::spawn()`. Store handle as `Option<JoinHandle<()>>`.
7b. If provider is `"claude-code"`, spawn `crate::claudemd::claudemd_loop()` with 24-hour interval. Store handle as `Option<JoinHandle<()>>`.
8. Enter the main event loop using `tokio::select!`:
   - Wait for incoming messages via `rx.recv()` and call `dispatch_message()`.
   - Wait for Ctrl+C signal via `tokio::signal::ctrl_c()`.
   - On shutdown signal, break from loop.
9. Call `shutdown(bg_handle, sched_handle, hb_handle, claudemd_handle)` for graceful cleanup.
10. Return Ok(()).

**Async Patterns:**
- Uses `tokio::spawn` to run channel listeners concurrently.
- Uses `tokio::select!` for the main event loop to handle both messages and shutdown signals.
- Scheduler and heartbeat loops are conditionally spawned based on config.
- All channel and provider operations are awaited.

**Error Handling:**
- If `channel.start()` fails, wraps error in anyhow and returns immediately.
- Channel listener tasks suppress errors silently (logs info if gateway receiver drops).

### Summarizer Functions (summarizer.rs)

#### `async fn background_summarizer(store: Store, provider: Arc<dyn Provider>, summarize_prompt: String, facts_prompt: String)`
**Purpose:** Periodically find and summarize idle conversations (infinite background task).

**Parameters:**
- `store: Store` - Shared memory store.
- `provider: Arc<dyn Provider>` - Shared provider reference.
- `summarize_prompt: String` - Prompt template for conversation summarization (from `Prompts.summarize`).
- `facts_prompt: String` - Prompt template for facts extraction (from `Prompts.facts`).

**Returns:** Never returns (infinite loop).

**Logic:**
1. Loop forever with 60-second sleep between iterations.
2. Call `store.find_idle_conversations()` to find conversations inactive for a threshold period.
3. For each idle conversation:
   - Call `summarize_conversation()` to summarize and close it.
   - Log errors but continue processing other conversations.
4. Log errors from `find_idle_conversations()` but continue the loop.

**Async Patterns:**
- Uses `tokio::time::sleep()` for periodic ticking.
- All storage and provider operations are awaited.

**Error Handling:**
- Errors are logged with `error!()` but do not stop the task.
- Task runs indefinitely regardless of errors.

### Scheduler Functions (scheduler.rs)

#### `async fn scheduler_loop(store, channels, poll_secs, provider, skills, prompts, model_complex, sandbox_prompt, heartbeat_interval, audit, provider_name)`
**Purpose:** Background task that periodically checks for due scheduled tasks and delivers them via the appropriate channel. Action tasks include outcome verification, audit logging, and retry logic.

**Parameters:**
- `store: Store` - Shared memory store for task queries.
- `channels: HashMap<String, Arc<dyn Channel>>` - Map of channel implementations for delivery.
- `poll_secs: u64` - Polling interval in seconds (from `SchedulerConfig.poll_interval_secs`).
- `audit: AuditLogger` - Audit logger for action task execution tracking.
- `provider_name: String` - Provider name for audit log entries.

**Returns:** Never returns (infinite loop).

**Logic:**
1. Loop forever with `poll_secs`-second sleep between iterations.
2. Call `store.get_due_tasks()` to find tasks where `status = 'pending'` and `due_at <= now`.
3. For each due task `(id, channel_name, sender_id, reply_target, description, repeat, task_type)`:
   - **Action tasks:**
     a. Start timing with `Instant::now()`.
     b. Enrich system prompt with user profile (facts from DB) and language preference.
     c. Inject delivery context instruction — tells the AI its text response will be delivered directly to the task owner via their messaging channel (no external email/API/curl needed).
     d. Inject `ACTION_OUTCOME:` verification instruction into system prompt.
     e. Invoke provider with full tool/MCP access.
     d. Parse `ACTION_OUTCOME:` marker from response (`Success`, `Failed(reason)`, or missing).
     e. Process response markers (SCHEDULE, SCHEDULE_ACTION, CANCEL_TASK, UPDATE_TASK, REWARD, LESSON, HEARTBEAT).
     f. Write audit log entry with `[ACTION]` prefix, elapsed time, status.
     g. On success (or missing marker — backward compat): call `complete_task()`, send response.
     h. On failure: call `fail_task()` (up to `MAX_ACTION_RETRIES=3` retries, 2-minute delay), notify user.
     i. On provider error: call `fail_task()`, notify user, write error audit entry.
   - **Reminder tasks:** Build an `OutgoingMessage` with text `"Reminder: {description}"` and `reply_target`.
   - Look up the channel by `channel_name` in the channels map.
   - If channel not found, log warning and skip.
   - Send the message via `channel.send()`.
   - If send fails, log error and skip to next task.
   - Call `store.complete_task(id, repeat)` to mark task as delivered (one-shot) or advance `due_at` (recurring).
   - Log success.
   - **Note:** `sender_id` is propagated from the parent task to all nested task operations (create, cancel, update), ensuring correct ownership.
4. Log errors from `get_due_tasks()` but continue the loop.

**Async Patterns:**
- Uses `tokio::time::sleep()` for periodic ticking.
- All storage and channel operations are awaited.

**Error Handling:**
- Channel send errors are logged and the task is skipped (not marked complete).
- Task completion errors are logged but do not stop the loop.
- `get_due_tasks()` errors are logged but do not stop the loop.
- Action task failures trigger `fail_task()` (retry with 2-minute delay, up to 3 retries).

### Heartbeat Functions (heartbeat.rs)

#### `async fn heartbeat_loop(provider, channels, config, prompts, sandbox_prompt, memory, interval, model_complex, model_fast, skills, audit, provider_name)`
**Purpose:** Background task that periodically invokes the AI provider for **active execution** of a health checklist. A fast Sonnet classification call groups related items by domain before execution. Each group gets its own focused Opus session **in parallel**. Falls back to a single call when all items belong to the same domain (≤3 items or closely related). Processes response markers (SCHEDULE, SCHEDULE_ACTION, HEARTBEAT_*, CANCEL_TASK, UPDATE_TASK) identically to the scheduler.

**Parameters:**
- `provider: Arc<dyn Provider>` - Shared provider reference for the check-in call.
- `channels: HashMap<String, Arc<dyn Channel>>` - Map of channel implementations for alert delivery.
- `config: HeartbeatConfig` - Heartbeat configuration (interval, active hours, channel, reply target).
- `prompts: Prompts` - Full prompts struct (Identity/Soul/System + heartbeat_checklist template).
- `sandbox_prompt: Option<String>` - Optional sandbox constraint text appended to the system prompt.
- `memory: Store` - Shared memory store for enriching heartbeat context with user facts and recent summaries. Also used for marker-created tasks.
- `interval: Arc<AtomicU64>` - Shared atomic holding the current interval in minutes.
- `model_complex: String` - Opus model name for active execution.
- `model_fast: String` - Sonnet model name for classification.
- `skills: Vec<omega_skills::Skill>` - Loaded skills for MCP trigger matching per group.
- `audit: AuditLogger` - Audit logger for recording heartbeat executions.
- `provider_name: String` - Provider name for audit entries.

**Returns:** Never returns (infinite loop).

**Logic:**
1. Loop forever, reading the interval from the shared `AtomicU64` on each iteration. Sleep is **clock-aligned**.
2. Check active hours; skip if outside window.
3. Read checklist from `~/.omega/prompts/HEARTBEAT.md` via `read_heartbeat_file()`. Skip if none.
4. **Build enrichment and system prompt once** (shared across all groups) via `build_enrichment()` and `build_system_prompt()`.
5. **Classify checklist** via `classify_heartbeat_groups()` — fast Sonnet call (no tools) that returns `None` for DIRECT or `Some(Vec<String>)` for grouped domains.
6. **DIRECT path (None):** Single Opus call via `execute_heartbeat_group()` with the full checklist. Result handled by `send_heartbeat_result()`.
7. **Grouped path (Some):** Spawn each group as a parallel `tokio::spawn(execute_heartbeat_group(...))`. Collect results. Groups returning `None` (HEARTBEAT_OK) are logged. Non-OK texts are joined with `\n\n---\n\n` and sent as a single combined message via `send_heartbeat_result()`.

#### `async fn classify_heartbeat_groups(provider, model_fast, checklist) -> Option<Vec<String>>`
**Purpose:** Fast Sonnet classification that groups related checklist items by domain. Returns `None` for DIRECT (all items closely related or ≤3 items). Returns `Some(groups)` when items span different domains. Reuses `parse_plan_response()` for parsing. On failure, returns `None` (safe fallback).

#### `async fn execute_heartbeat_group(provider, model_complex, group_items, heartbeat_template, enrichment, system_prompt, skills, memory, sender_id, channel_name, interval) -> Option<(String, i64)>`
**Purpose:** Execute a single heartbeat group via Opus. Builds the prompt from the heartbeat template with the group's items, enriches with pre-computed context, matches MCP servers per group, calls the provider, processes markers via `process_heartbeat_markers()`, and evaluates HEARTBEAT_OK. Returns `None` if OK, `Some((text, elapsed_ms))` if content for the user.

#### `async fn process_heartbeat_markers(text, memory, sender_id, channel_name, interval) -> String`
**Purpose:** Process all markers in a heartbeat response (SCHEDULE, SCHEDULE_ACTION, HEARTBEAT_*, CANCEL_TASK, UPDATE_TASK, REWARD, LESSON). REWARD markers are stored via `store_outcome()` with source=`"heartbeat"`. LESSON markers are stored via `store_lesson()`. Returns text with all markers stripped. Shared by both single-call and per-group paths to avoid duplication.

#### `async fn build_enrichment(memory) -> String`
**Purpose:** Build enrichment context from user facts, recent conversation summaries, learned lessons (via `get_all_lessons()`), and recent outcomes (via `get_all_recent_outcomes(24, 20)`). Computed once and shared across all groups. Lessons are formatted as `"Learned rules:\n- [domain] rule"`. Outcomes are formatted as `"Recent outcomes (last 24h):\n- [+/-/~] domain: lesson (timestamp)"`.

#### `fn build_system_prompt(prompts, sandbox_prompt) -> String`
**Purpose:** Build the heartbeat system prompt (Identity + Soul + System + sandbox + current time). Computed once and shared across all groups.

#### `async fn send_heartbeat_result(result, channel_name, sender_id, channels, config, audit, provider_name, model)`
**Purpose:** Audit log and send a heartbeat result to the user. If result is `None`, logs OK and returns. Otherwise creates audit entry and sends to channel.

**Error Handling:**
- Classification errors fall back to single-call (zero regression risk).
- Provider errors per group are logged but do not affect other groups.
- Spawned group panics are caught and logged.
- Channel send errors are logged but do not stop the loop.

### Summarizer (continued)

#### `async fn summarize_conversation(store: &Store, provider: &Arc<dyn Provider>, conversation_id: &str, summarize_prompt: &str, facts_prompt_template: &str) -> Result<(), anyhow::Error>`
**Purpose:** Summarize a conversation, extract user facts, and close it.

**Parameters:**
- `store: &Store` - Reference to the memory store.
- `provider: &Arc<dyn Provider>` - Reference to the AI provider.
- `conversation_id: &str` - ID of the conversation to summarize.
- `summarize_prompt: &str` - Prompt for conversation summarization (from `Prompts.summarize`).
- `facts_prompt_template: &str` - Prompt for facts extraction (from `Prompts.facts`).

**Returns:** `Result<(), anyhow::Error>`.

**Logic:**
1. Fetch all messages from the conversation via `store.get_conversation_messages()`.
2. If empty, close the conversation with "(empty conversation)" summary and return Ok.
3. Build a plain-text transcript by iterating messages and formatting as "User: ..." or "Assistant: ...".
4. **Summarization step:**
   - Create prompt by concatenating `summarize_prompt` + "\n\n" + transcript.
   - Call `provider.complete(Context::new(prompt))`.
   - On success, use the response text as summary.
   - On failure, use fallback: "({count} messages, summary unavailable)".
5. **Facts extraction step:**
   - Create prompt by concatenating `facts_prompt_template` + "\n\n" + transcript.
   - Call `provider.complete(Context::new(prompt))`.
   - Parse response line by line as "key: value" pairs.
   - For each valid pair (non-empty key and value), validate via `is_valid_fact(key, value)` before storing.
   - `is_valid_fact()` rejects: system-managed keys (`welcomed`, `preferred_language`, `active_project`, `personality`), keys >50 chars, values >200 chars, numeric-only keys, values starting with `$`, pipe-delimited table rows (2+ pipes), and pure numeric values (price patterns).
   - If response is "none", skip fact storage.
6. Query the database directly via `sqlx::query_as()` to get the `sender_id` from the conversations table.
7. Call `store.close_conversation(conversation_id, summary)` to mark conversation as closed and store summary.
8. Log success.
9. Return Ok(()).

**Async Patterns:**
- Uses `provider.complete()` twice in sequence (summarization, then facts).
- Uses `sqlx::query_as()` directly for database queries.
- All operations are awaited.

**Error Handling:**
- Early return on `get_conversation_messages()` failure.
- Summarization failure falls back to message count.
- Facts extraction errors are caught with `if let Ok()` and skipped.
- Database query errors are suppressed via `.ok().flatten()`.
- Returns top-level error on `close_conversation()` failure.

### Dispatch and Shutdown (mod.rs)

#### `async fn shutdown(&self, bg_handle: &JoinHandle<()>, sched_handle: &Option<JoinHandle<()>>, hb_handle: &Option<JoinHandle<()>>, claudemd_handle: &Option<JoinHandle<()>>)`
**Purpose:** Gracefully shut down the gateway.

**Parameters:**
- `bg_handle: &tokio::task::JoinHandle<()>` - Handle to the background summarizer task.
- `sched_handle: &Option<tokio::task::JoinHandle<()>>` - Optional handle to the scheduler loop task.
- `hb_handle: &Option<tokio::task::JoinHandle<()>>` - Optional handle to the heartbeat loop task.
- `claudemd_handle: &Option<tokio::task::JoinHandle<()>>` - Optional handle to the CLAUDE.md maintenance loop task.

**Returns:** None (void).

**Logic:**
1. Log "Shutting down...".
2. Abort the background summarizer task via `bg_handle.abort()`.
3. If `sched_handle` is `Some`, abort the scheduler task.
4. If `hb_handle` is `Some`, abort the heartbeat task.
4b. If `claudemd_handle` is `Some`, abort the CLAUDE.md maintenance task.
5. Find all active conversations via `store.find_all_active_conversations()`.
6. For each active conversation, call `summarize_conversation()` to summarize before closing.
7. Log warnings for summarization errors but continue.
8. For each channel, call `channel.stop()` to cleanly shut down the channel.
9. Log warnings for channel stop errors but continue.
10. Log "Shutdown complete.".

**Error Handling:**
- Errors are logged with `warn!()` but do not stop the shutdown process.
- All channels are stopped regardless of individual failures.

#### `async fn dispatch_message(self: &Arc<Self>, incoming: IncomingMessage)`
**Purpose:** Non-blocking message dispatcher that buffers messages when a sender is already being processed.

**Parameters:**
- `incoming: IncomingMessage` - The message to dispatch.

**Returns:** None (void).

**Logic:**
1. Acquire lock on `self.active_senders`.
2. Check if the sender (`incoming.sender_id`) already has an active provider call in flight:
   - **If busy**: Append the message to the sender's buffer. Send a "Got it, I'll get to this next." acknowledgment to the user. Release the lock and return.
   - **If not busy**: Insert an empty buffer for the sender (marking them as active). Release the lock.
3. Clone `Arc<Self>` and the incoming message.
4. Spawn a `tokio::spawn` task that:
   a. Calls `self.handle_message(incoming)` for the current message.
   b. After completion, enters a loop:
      - Acquire lock on `active_senders`.
      - Pop the next buffered message for this sender.
      - If no more buffered messages, remove the sender from `active_senders` and break.
      - Release lock and call `self.handle_message(buffered_msg)`.
   c. This ensures all buffered messages are processed in order.

**Concurrency Model:**
- Messages for different senders are processed concurrently via `tokio::spawn`.
- Messages for the same sender are serialized: only one provider call per sender at a time.
- The `Mutex` is only held briefly to check/update the buffer, never across async operations.

### Pipeline (pipeline.rs)

#### `async fn handle_message(&self, incoming: IncomingMessage)`
**Purpose:** Process a single incoming message through the complete pipeline.

**Parameters:**
- `incoming: IncomingMessage` - The message to process.

**Returns:** None (void, logging errors).

**Pipeline Stages:**

**Stage 1: Auth Check (Lines 262-292)**
- If auth is enabled, call `check_auth()`.
- If denied, log warning, audit the denial, send deny message, and return.
- Audit status: `AuditStatus::Denied`.
- Does not process message further.

**Stage 2: Input Sanitization (Lines 294-305)**
- Call `sanitize::sanitize(&incoming.text)`.
- If modified, log warning with sanitization warnings.
- Clone the incoming message and replace its text with sanitized version.

**Stage 2a: Inbox Image Save**
- If `incoming.attachments` is non-empty:
  - Call `ensure_inbox_dir(data_dir)` to create `{data_dir}/workspace/inbox/` if it does not exist.
  - Call `save_attachments_to_inbox(&inbox_dir, &incoming.attachments)` to save Image-type attachments to disk (zero-byte data is rejected, writes use `sync_all` for durability).
  - For each saved image path, prepend `[Attached image: /full/path.jpg]` to `clean_incoming.text`.
  - Wrap paths in `InboxGuard` (RAII) — cleanup is guaranteed on Drop regardless of early returns.

**Stage 2b: Welcome Check (First-Time Users)**
- If the sender has no `welcomed` fact (first-time user):
  - Detect language from the incoming message text.
  - Send the localized welcome message from `self.prompts.welcome`.
  - Store `welcomed = "true"` and `preferred_language` facts.
  - Log the welcome event.
  - **Does not return** — the first message falls through to normal processing (command dispatch, context building, provider call, etc.).

**Stage 3: Command Dispatch (Lines 307-320)**
- Call `commands::Command::parse()` to check if input is a bot command.
- If command detected:
  - Call `commands::handle()` to process the command.
  - Send response text.
  - Return (skip provider call).
- Examples: `/uptime`, `/help`, `/status`.

**Stage 3b: Platform Formatting Hint**
- Platform-specific formatting hint is injected as part of the keyword-gated prompt composition (Stage 4b), always included:
  - **WhatsApp**: "Platform: WhatsApp. Avoid markdown tables and headers — use bold (*text*) and bullet lists instead."
  - **Telegram**: "Platform: Telegram. Markdown is supported (bold, italic, code blocks)."
  - Other channels: no hint injected.

**Stage 4: Typing Indicator (Lines 322-342)**
- Get the channel for the incoming message.
- If channel exists and incoming has a `reply_target`, spawn a repeating task:
  - Send initial typing action immediately.
  - Every 5 seconds, resend typing action.
  - Abort if channel send fails.
- Store handle for later abort.

**Stage 4b: Keyword-Gated Prompt Composition**
*Note: This stage consolidates the former Stage 4b (Project Injection), 4c (Heartbeat Awareness), 4e (Heartbeat Pulse), and 4f (Sandbox Injection) into a unified keyword-gated prompt composition.*
- Lowercase the user's message text once as `msg_lower`.
- Run `kw_match()` against each keyword list to compute `needs_scheduling`, `needs_recall`, `needs_tasks`, `needs_projects`, `needs_meta`.
- Compose the system prompt:
  1. Always: `format!("{}\n\n{}\n\n{}", identity, soul, system)` + provider info + current time + platform hint.
  2. Conditionally append `prompts.scheduling` (if `needs_scheduling`), `prompts.projects_rules` (if `needs_projects`), `prompts.meta` (if `needs_meta`).
  3. Conditionally append active project instructions, heartbeat checklist, heartbeat pulse, sandbox constraint (unchanged from prior behavior).
- Build `ContextNeeds { recall, pending_tasks, profile, summaries, outcomes }` to gate DB queries and prompt injection.
- This architecture reduces average token overhead by ~55% for typical messages (e.g., "good morning" skips scheduling rules, task lists, project rules, and meta instructions).

**Stage 5: Build Context from Memory**
- Call `self.memory.build_context(&clean_incoming, &system_prompt, &context_needs)` to build enriched context.
- The `&context_needs` parameter gates expensive DB queries: semantic recall (FTS5) is skipped unless `needs_recall` is true, and pending tasks are skipped unless `needs_tasks` is true.
- This includes recent conversation history, relevant facts (always loaded), and the keyword-composed system prompt.
- If error, abort typing task, send error message, and return.

**Stage 5b: MCP Trigger Matching**
- Call `omega_skills::match_skill_triggers(&self.skills, &clean_incoming.text)` to check if the message matches any skill triggers.
- If matched, populate `context.mcp_servers` with the declared MCP servers from matching skills.
- The provider will use these to write temporary `.claude/settings.local.json` and add `mcp__<name>__*` to `--allowedTools`.

**Stage 6: Get Response from Provider (async with delayed, localized status updates)**
- Resolve the user's `preferred_language` fact from memory (defaults to English).
- Get localized status messages via `status_messages(lang)`.
- Spawn `provider.complete(&context)` as a background task via `tokio::spawn`.
- Spawn a delayed status updater task: sends a localized first nudge after 15 seconds, then localized "Still working..." every 120 seconds. If the provider responds within 15 seconds, the updater is aborted and the user sees no extra messages.
- Wait for the provider result; abort the status updater when the result arrives.
- Map provider errors to user-friendly messages via `friendly_provider_error()`:
  - On timeout -> "I took too long to respond. Please try again..."
  - On other errors -> "Something went wrong. Please try again."
  - On JoinError (task panic) -> "Something went wrong. Please try again."
- On success, set `reply_target` from incoming message.
- On error:
  - Abort typing task.
  - Audit the error with status `AuditStatus::Error`.
  - Send friendly error message.
  - Return.

**Stage 5: Classify and Route (Model Selection)**
- After SILENT suppression check:
- Call `self.classify_and_route()` unconditionally (no word-count gate), always passing `full_history` (not `context.history`, which may be empty during session continuation). This sends a complexity-aware classification prompt enriched with lightweight context (active project name, last 3 history messages truncated to 80 chars, available skill names) to the provider (no system prompt, no MCP servers) with `ctx.max_turns = Some(25)` (generous limit — classification is best-effort, falls through to DIRECT on failure), `ctx.allowed_tools = Some(vec![])` (disables all tool use), and `ctx.model = Some(self.model_fast.clone())` (classification always uses the fast model). The prompt routes DIRECT for simple questions, conversations, and routine actions (reminders, scheduling, lookups) regardless of quantity, and only produces a step list for genuinely complex work (multi-file code changes, deep research, building, sequential dependencies). When in doubt, prefers DIRECT.
  - The classification response is parsed by `parse_plan_response()`:
    - If the response contains "DIRECT" (any case) → returns `None`.
    - If the response contains only a single step → returns `None`.
    - If the response contains a multi-step numbered list → returns `Some(steps)`.
    - If the response is unparseable → returns `None`.
  - If `None` is returned (DIRECT path): set `context.model = Some(self.model_fast.clone())` and fall through to the normal single provider call.
  - If `Some(steps)` is returned (Steps path): set `context.model = Some(self.model_complex.clone())`.
    - Call `self.execute_steps(incoming, original_task, context, steps, inbox_images)` for autonomous multi-step execution.
    - `execute_steps()` announces the plan to the user, executes each step in a fresh provider call with accumulated context (inheriting `step_ctx.model = context.model.clone()`), retries failed steps up to 3 times, calls `self.process_markers()` on each step result to extract all markers (SCHEDULE, SKILL_IMPROVE, etc.), sends progress messages after each step, audits the exchange, sends a final summary, and cleans up inbox images.
    - Return (skip normal provider call).

**Stage 5: Process Markers**
- After SILENT suppression check:
- Call `self.process_markers(&incoming, &mut response.text)` — a unified method that extracts and processes all markers from the provider response. This same method is called on each step result in `execute_steps()`, ensuring markers work in both direct and multi-step paths.
- Markers processed in order: SCHEDULE, SCHEDULE_ACTION, PROJECT_ACTIVATE/DEACTIVATE, WHATSAPP_QR, LANG_SWITCH, REWARD, LESSON, HEARTBEAT_ADD/REMOVE/INTERVAL, SKILL_IMPROVE.
- Each marker is stripped from the response text after processing.

**Stage 6: Store Exchange in Memory (Lines 579-582)**
- Call `self.memory.store_exchange(&incoming, &response)` to save the exchange.
- Log error if storage fails but continue.

**Stage 7: Audit Log (Lines 584-599)**
- Log the successful exchange via `self.audit.log()`.
- Include all metadata: provider, model, processing time.
- Status: `AuditStatus::Ok`.

**Stage 8: Send Response (Lines 601-608)**
- Get the channel for the incoming message.
- Call `channel.send(response)` to deliver the response.
- Log error if channel send fails.

**Stage 8b: Send New Workspace Images**
- After sending the text response, compute a diff of workspace images (before vs. after the provider call).
- For each new image file (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`) found in the top-level workspace directory:
  - Read the file bytes via `std::fs::read()`.
  - Call `channel.send_photo(target, &bytes, filename)` to deliver the image.
  - Delete the file after sending (cleanup).
  - Log with `tracing`: `info!` on success, `warn!` on failure.
- Uses `snapshot_workspace_images()` to collect top-level image files before and after the provider call, then computes the set difference.

**Stage 9: Cleanup Inbox Images**
- Call `cleanup_inbox_images(&inbox_images)` to remove temporary inbox files that were saved in Stage 2a.
- Each file is removed individually; errors are logged at warn level but do not stop the cleanup.

**Async Patterns:**
- All stages are awaited sequentially.
- Typing task runs concurrently via `tokio::spawn()`.
- Task is aborted early if error occurs before response.

**Error Handling:**
- Auth failure: deny message sent, audit logged, early return.
- Sanitization warnings: logged but processing continues.
- Context build failure: error message sent, early return.
- Provider error: error message sent, audit logged, early return.
- Memory storage failure: logged but does not stop response delivery.
- Channel send failure: logged but pipeline completes.

### Authentication (auth.rs)

#### `fn check_auth(&self, incoming: &IncomingMessage) -> Option<String>`
**Purpose:** Verify if a message sender is authorized.

**Parameters:**
- `incoming: &IncomingMessage` - The incoming message to check.

**Returns:** `Option<String>` where `None` = allowed, `Some(reason)` = denied.

**Logic:**
1. Match on `incoming.channel`:
   - **"telegram"**:
     - Get `allowed_users` list from `channel_config.telegram`.
     - If list is empty, allow all (returns `None`). Used for testing.
     - If list exists:
       - Parse `incoming.sender_id` as i64 (default to -1 on parse error).
       - If sender_id is in allowed_users, return `None`.
       - Otherwise, return `Some("telegram user X not in allowed_users")`.
     - If telegram config not set, return `Some("telegram channel not configured")`.
   - **Other channels**:
     - Return `Some("unknown channel: {name}")`.

**Error Handling:**
- Parsing sender_id as i64 uses `unwrap_or(-1)` (will never match valid user, causing denial).
- No panics.

### Utility (mod.rs)

#### `async fn send_text(&self, incoming: &IncomingMessage, text: &str)`
**Purpose:** Send a plain text response message.

**Parameters:**
- `incoming: &IncomingMessage` - The original incoming message (used for channel and reply target).
- `text: &str` - The text to send.

**Returns:** None (void).

**Logic:**
1. Create an `OutgoingMessage` with:
   - `text: text.to_string()`.
   - `metadata: MessageMetadata::default()`.
   - `reply_target: incoming.reply_target.clone()`.
2. Get the channel for `incoming.channel`.
3. Call `channel.send(msg)`.
4. Log error if send fails.

**Error Handling:**
- Send errors are logged but do not return an error code.

### Routing (routing.rs)

#### `async fn classify_and_route(&self, message: &str, active_project: Option<&str>, recent_history: &[ContextEntry], skill_names: &[&str]) -> Option<Vec<String>>`
**Purpose:** Send a context-enriched classification call to the provider to determine if the message requires multi-step execution. Always runs (no word-count gate). Uses the fast model for classification.

**Parameters:**
- `message: &str` - The user's original message text.
- `active_project: Option<&str>` - The user's currently active project (if any).
- `recent_history: &[ContextEntry]` - Conversation history entries (last 3 used).
- `skill_names: &[&str]` - Names of available skills.

**Returns:** `Option<Vec<String>>` — `Some(steps)` if the classifier identifies a multi-step task, `None` if the response is "DIRECT", single-step, or on error.

**Logic:**
1. Call `build_classification_context()` to produce a lightweight context block (~90 tokens) from the active project, last 3 history messages (truncated to 80 chars each), and skill names. Empty inputs produce an empty block (identical to previous behavior).
2. Build the complexity-aware classification prompt: routes DIRECT for simple questions, conversations, and routine actions (reminders, scheduling, lookups) regardless of quantity; produces a step list only for genuinely complex work (multi-file code changes, deep research, building, sequential dependencies); defaults to DIRECT when in doubt. The context block is injected between the instructions and the user's request.
3. Set `ctx.max_turns = Some(25)` (generous limit — classification is best-effort, falls through to DIRECT on failure), `ctx.allowed_tools = Some(vec![])` (disables all tool use via `--allowedTools ""`), and `ctx.model = Some(self.model_fast.clone())` so classification uses the fast model with no tool access.
4. Call `provider.complete()` with this minimal context (no system prompt, no MCP servers, no tools).
5. On success, pass the response text to `parse_plan_response()`.
6. On error, log the error and return `None` (falls through to normal single provider call).

#### `fn build_classification_context(active_project: Option<&str>, recent_history: &[ContextEntry], skill_names: &[&str]) -> String`
**Purpose:** Build a lightweight context string for the classification prompt. Pure function, no async.

**Parameters:**
- `active_project: Option<&str>` - Active project name (if any).
- `recent_history: &[ContextEntry]` - Conversation history (last 3 entries used, each truncated to 80 chars).
- `skill_names: &[&str]` - Available skill names.

**Returns:** A context string with sections separated by newlines. Empty sections are omitted. Returns empty string when all inputs are empty.

#### `async fn execute_steps(&self, incoming: &IncomingMessage, original_task: &str, context: &Context, steps: Vec<String>, inbox_images: Vec<PathBuf>)`
**Purpose:** Execute a multi-step plan autonomously, reporting progress to the user after each step.

**Parameters:**
- `incoming: &IncomingMessage` - The original incoming message (used for channel, sender, and reply target).
- `original_task: &str` - The user's original message text (used for context in step execution).
- `context: &Context` - The enriched context (system prompt, history, facts).
- `steps: Vec<String>` - The list of step descriptions to execute.
- `inbox_images: Vec<PathBuf>` - Temporary inbox image files to clean up after execution.

**Returns:** None (void).

**Logic:**
1. Announce the plan to the user (list of steps).
2. For each step:
   - Build a step context with `step_ctx.model = context.model.clone()` (inherits model from the parent context, ensuring all steps use the complex model).
   - Execute the step by calling `provider.complete()` with the step description and accumulated context.
   - On failure, retry up to 3 times.
   - If all retries fail, send an error message and continue to next step.
   - On success, call `self.process_markers()` on the step result to extract all markers (SCHEDULE, SKILL_IMPROVE, etc.).
   - Send a progress message (e.g., "✓ Step (1/N)").
   - Audit the step exchange.
3. After all steps complete, send a final summary message to the user.
4. Inbox images are cleaned up by `InboxGuard` (RAII Drop) in `handle_message`.

**Error Handling:**
- Per-step failures are retried up to 3 times before continuing to the next step.
- Provider errors are logged and a user-friendly error message is sent.

### Process Markers (process_markers.rs)

#### `async fn process_markers(&self, incoming: &IncomingMessage, text: &mut String)`
**Purpose:** Extract and process all markers from a provider response text. Unified method called by both `handle_message` (direct path) and `execute_steps` (multi-step path) to ensure markers work in all execution modes.

**Markers processed (in order):**
1. SCHEDULE — create reminder task
2. SCHEDULE_ACTION — create action task
3. PROJECT_ACTIVATE / PROJECT_DEACTIVATE — activate/deactivate project
4. WHATSAPP_QR — trigger WhatsApp QR pairing
5. LANG_SWITCH — persist language preference
6. PERSONALITY — set/reset personality preference (conversational `/personality`)
7. FORGET_CONVERSATION — close current conversation (conversational `/forget`)
8. CANCEL_TASK — cancel scheduled tasks by ID prefix (conversational `/cancel`), processes ALL markers via `extract_all_cancel_tasks()`, pushes `MarkerResult::TaskCancelled` or `MarkerResult::TaskCancelFailed` per task
9. UPDATE_TASK — update fields of pending tasks by ID prefix (description, due_at, repeat; empty fields keep existing values), processes ALL markers via `extract_all_update_tasks()`, pushes `MarkerResult::TaskUpdated` or `MarkerResult::TaskUpdateFailed` per task
10. PURGE_FACTS — delete all non-system facts, preserving system keys (conversational `/purge`)
11. REWARD — process ALL markers via `extract_all_rewards()`, parse each via `parse_reward_line()`, store via `store_outcome()` with source=`"conversation"`, strip all markers
12. LESSON — process ALL markers via `extract_all_lessons()`, parse each via `parse_lesson_line()`, store via `store_lesson()` (upsert by sender_id+domain), strip all markers
13. HEARTBEAT_ADD / HEARTBEAT_REMOVE / HEARTBEAT_INTERVAL — update heartbeat checklist or interval
14. SKILL_IMPROVE — read skill's SKILL.md, append lesson under `## Lessons Learned`, confirm to user (extracted into `process_improvement_markers()` helper)
15. BUG_REPORT — append self-detected limitation to `{data_dir}/BUG.md` with date grouping, confirm to user

**Logic:** For each marker type: extract from text, process side effects (DB writes, notifications, file updates), strip the marker from text. Mutates `text` in place. PURGE_FACTS is extracted into `process_purge_facts()` helper. SKILL_IMPROVE and BUG_REPORT are extracted into `process_improvement_markers()` helper.

## Free Functions (Distributed Across Submodules)

> **Note:** Marker extraction/parsing/stripping functions (`extract_schedule_marker`, `parse_schedule_line`, `strip_schedule_marker`, `extract_heartbeat_markers`, `strip_heartbeat_markers`, `apply_heartbeat_changes`, `extract_lang_switch`, `strip_lang_switch`, `extract_personality`, `strip_personality`, `has_forget_marker`, `strip_forget_marker`, `extract_all_cancel_tasks`, `strip_cancel_task`, `extract_all_update_tasks`, `strip_update_task`, `has_purge_marker`, `strip_purge_marker`, `extract_project_activate`, `has_project_deactivate`, `strip_project_markers`, `read_heartbeat_file`, `HeartbeatAction`, etc.) were extracted into `src/markers.rs` in a prior refactor. See `specs/src-markers-rs.md` for their specifications. They are still documented below for historical completeness but live in `src/markers.rs`.

### `async fn summarize_and_extract(store, provider, conversation_id, summarize_prompt, facts_prompt) -> Result<(), anyhow::Error>` (summarizer.rs)
**Purpose:** Summarize a conversation and extract facts in a single provider call. Used by `handle_forget()` for background summarization after instant close.

**Parameters:**
- `store: &Store` — Reference to the memory store.
- `provider: &Arc<dyn Provider>` — Reference to the AI provider.
- `conversation_id: &str` — ID of the conversation to summarize.
- `summarize_prompt: &str` — Prompt for conversation summarization.
- `facts_prompt: &str` — Prompt for facts extraction.

**Logic:**
1. Load messages via `get_conversation_messages()` (no status filter — works after close).
2. Build transcript from messages.
3. Send a single combined prompt asking for both summary and facts in a structured format (`SUMMARY: ... FACTS: ...`).
4. Parse response: split on `FACTS:` line to extract summary and facts sections.
5. Store valid facts using existing `is_valid_fact()` validation.
6. Update the already-closed conversation with the summary via `close_conversation()` (idempotent — sets summary on the closed row).
7. All errors are logged via `warn!()`, never surfaced to user.

**Difference from `summarize_conversation()`:** Uses one provider call instead of two. Does not close the conversation itself (expects it to be already closed). Designed for background spawning.

### `fn parse_plan_response(text: &str) -> Option<Vec<String>>` (routing.rs)
**Purpose:** Parse the planning provider response into actionable steps.

**Returns:** `None` if the response contains "DIRECT" (any case), has only a single step, or is unparseable. Returns `Some(steps)` for multi-step numbered lists (e.g., `1. Do something`, `2. Do something else`). Non-numbered preamble lines before the numbered list are ignored during parsing.

### `fn extract_schedule_marker(text: &str) -> Option<String>` (markers.rs)
**Purpose:** Extract the first `SCHEDULE:` line from response text.

**Logic:** Iterates through lines, finds the first line whose trimmed form starts with `"SCHEDULE:"`, returns it trimmed.

**Returns:** `Some(line)` if found, `None` otherwise.

### `fn parse_schedule_line(line: &str) -> Option<(String, String, String)>`
**Purpose:** Parse a `SCHEDULE:` line into `(description, due_at, repeat)`.

**Format:** `SCHEDULE: <description> | <ISO 8601 datetime> | <once|daily|weekly|monthly|weekdays>`

**Logic:**
1. Strip `"SCHEDULE:"` prefix.
2. Split on `|` into exactly 3 parts.
3. Trim each part.
4. Validate that description and due_at are non-empty.
5. Lowercase the repeat value.
6. Return the tuple.

**Returns:** `None` if format is invalid (wrong number of parts or empty fields).

### `fn strip_schedule_marker(text: &str) -> String`
**Purpose:** Remove all `SCHEDULE:` lines from response text so the marker is not shown to the user.

**Logic:** Filters out any line whose trimmed form starts with `"SCHEDULE:"`, then joins remaining lines and trims the result.

### `fn extract_project_activate(text: &str) -> Option<String>`
**Purpose:** Extract the project name from a `PROJECT_ACTIVATE: <name>` line in response text.

**Logic:** Iterates through lines, finds the first line whose trimmed form starts with `"PROJECT_ACTIVATE:"`, strips the prefix, trims, and returns the project name. Returns `None` if not found or if name is empty.

### `fn has_project_deactivate(text: &str) -> bool`
**Purpose:** Check if response text contains a `PROJECT_DEACTIVATE` marker line.

**Logic:** Returns `true` if any line's trimmed form equals `"PROJECT_DEACTIVATE"`.

### `fn strip_project_markers(text: &str) -> String`
**Purpose:** Remove all `PROJECT_ACTIVATE:` and `PROJECT_DEACTIVATE` lines from response text so the markers are not shown to the user.

**Logic:** Filters out any line whose trimmed form starts with `"PROJECT_ACTIVATE:"` or equals `"PROJECT_DEACTIVATE"`, then joins remaining lines and trims the result.

### `fn extract_lang_switch(text: &str) -> Option<String>`
**Purpose:** Extract the language name from a `LANG_SWITCH:` line in response text.

**Logic:** Iterates through lines, finds the first line whose trimmed form starts with `"LANG_SWITCH:"`, strips the prefix, trims, and returns the language name. Returns `None` if not found or if language is empty.

### `fn strip_lang_switch(text: &str) -> String`
**Purpose:** Remove all `LANG_SWITCH:` lines from response text so the marker is not shown to the user.

**Logic:** Filters out any line whose trimmed form starts with `"LANG_SWITCH:"`, then joins remaining lines and trims the result.

### `fn extract_personality(text: &str) -> Option<String>`
**Purpose:** Extract the personality value from a `PERSONALITY:` line in response text. Conversational equivalent of `/personality`.

**Logic:** Same pattern as `extract_lang_switch` — finds the first line starting with `"PERSONALITY:"`, strips prefix, trims, returns `None` if empty.

### `fn strip_personality(text: &str) -> String`
**Purpose:** Remove all `PERSONALITY:` lines from response text.

**Logic:** Filters out lines starting with `"PERSONALITY:"`, joins, trims.

### `fn has_forget_marker(text: &str) -> bool`
**Purpose:** Check if response text contains a `FORGET_CONVERSATION` marker line. Conversational equivalent of `/forget`.

**Logic:** Returns `true` if any line's trimmed form equals exactly `"FORGET_CONVERSATION"`.

### `fn strip_forget_marker(text: &str) -> String`
**Purpose:** Remove all `FORGET_CONVERSATION` lines from response text.

**Logic:** Filters out lines whose trimmed form equals `"FORGET_CONVERSATION"`, joins, trims.

### `fn extract_all_cancel_tasks(text: &str) -> Vec<String>`
**Purpose:** Extract ALL task ID prefixes from `CANCEL_TASK:` lines in response text. Conversational equivalent of `/cancel`. Supports cancelling multiple tasks in a single response.

**Logic:** Iterates through all lines, finds every line whose trimmed form starts with `"CANCEL_TASK:"`, strips the prefix, trims each value, filters out empty values, and collects into a `Vec<String>`.

**Returns:** A `Vec<String>` of ID prefixes. Empty vec if no markers found.

### `fn strip_cancel_task(text: &str) -> String`
**Purpose:** Remove all `CANCEL_TASK:` lines from response text.

**Logic:** Filters out lines starting with `"CANCEL_TASK:"`, joins, trims.

### `fn has_purge_marker(text: &str) -> bool`
**Purpose:** Check if response text contains a `PURGE_FACTS` marker line. Conversational equivalent of `/purge`.

**Logic:** Returns `true` if any line's trimmed form equals exactly `"PURGE_FACTS"`.

### `fn strip_purge_marker(text: &str) -> String`
**Purpose:** Remove all `PURGE_FACTS` lines from response text.

**Logic:** Filters out lines whose trimmed form equals `"PURGE_FACTS"`, joins, trims.

### `fn read_heartbeat_file() -> Option<String>`
**Purpose:** Read `~/.omega/prompts/HEARTBEAT.md` if it exists, for use as a heartbeat checklist.

**Logic:**
1. Get `$HOME` env var.
2. Read `{home}/.omega/prompts/HEARTBEAT.md`.
3. Return `None` if file does not exist, is unreadable, or has only whitespace.
4. Return `Some(content)` otherwise.

### `enum HeartbeatAction`
**Purpose:** Represents an action extracted from a `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, or `HEARTBEAT_INTERVAL:` marker.

**Variants:**
- `Add(String)` — Item to add to the heartbeat checklist.
- `Remove(String)` — Keyword to match and remove from the heartbeat checklist.
- `SetInterval(u64)` — Dynamically change the heartbeat interval (minutes, 1–1440).

### `fn extract_heartbeat_markers(text: &str) -> Vec<HeartbeatAction>`
**Purpose:** Extract all `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, and `HEARTBEAT_INTERVAL:` markers from response text.

**Logic:** Iterates through lines, finds lines whose trimmed form starts with `"HEARTBEAT_ADD:"`, `"HEARTBEAT_REMOVE:"`, or `"HEARTBEAT_INTERVAL:"`, strips the prefix, trims, and collects into a `Vec<HeartbeatAction>`. Empty items (marker with no description) are skipped. For `HEARTBEAT_INTERVAL:`, the value must parse as a `u64` between 1 and 1440 (inclusive); invalid values are silently ignored.

### `fn strip_heartbeat_markers(text: &str) -> String`
**Purpose:** Remove all `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, and `HEARTBEAT_INTERVAL:` lines from response text so the markers are not shown to the user.

**Logic:** Filters out any line whose trimmed form starts with `"HEARTBEAT_ADD:"`, `"HEARTBEAT_REMOVE:"`, or `"HEARTBEAT_INTERVAL:"`, then joins remaining lines and trims the result.

### `fn apply_heartbeat_changes(actions: &[HeartbeatAction])`
**Purpose:** Apply heartbeat add/remove actions to `~/.omega/prompts/HEARTBEAT.md`.

**Logic:**
1. Get `$HOME` env var. Return silently if not set.
2. Read existing file lines (or start with empty vec if file does not exist).
3. For each `Add(item)`: check if item already exists (case-insensitive, ignoring `- ` prefix). If not, append `- {item}`.
4. For each `Remove(item)`: remove all non-comment lines whose content contains the item (case-insensitive partial match). Comment lines (starting with `#`) are never removed.
5. Ensure `~/.omega/` directory exists.
6. Write the updated lines back to the file.

### `const IMAGE_EXTENSIONS: &[&str]`
**Purpose:** List of image file extensions recognized for workspace diff: `["png", "jpg", "jpeg", "gif", "webp"]`.

### `fn snapshot_workspace_images(workspace: &Path) -> HashSet<PathBuf>`
**Purpose:** Snapshot top-level image files in the workspace directory.

**Parameters:**
- `workspace: &Path` - Path to the workspace directory.

**Returns:** `HashSet<PathBuf>` containing paths to image files. Returns empty set on any error (non-existent dir, permission issues).

**Logic:**
1. Read the workspace directory via `std::fs::read_dir()`. Return empty set on error.
2. Filter to regular files whose extension (case-insensitive) matches `IMAGE_EXTENSIONS`.
3. Collect into a `HashSet<PathBuf>`.

### `fn ensure_inbox_dir(data_dir: &str) -> PathBuf`
**Purpose:** Create and return the inbox directory path at `{data_dir}/workspace/inbox/`.

**Parameters:**
- `data_dir: &str` - The Omega data directory (e.g., `~/.omega`).

**Returns:** `PathBuf` pointing to `{data_dir}/workspace/inbox/`.

**Logic:**
1. Build the path `{data_dir}/workspace/inbox/`.
2. Create the directory (and parents) if it does not exist via `std::fs::create_dir_all()`.
3. Return the path.

### `fn save_attachments_to_inbox(inbox: &Path, attachments: &[Attachment]) -> Vec<PathBuf>`
**Purpose:** Save Image-type attachments to the inbox directory on disk and return the list of saved file paths.

**Parameters:**
- `inbox: &Path` - Path to the inbox directory.
- `attachments: &[Attachment]` - Slice of attachments from the incoming message.

**Returns:** `Vec<PathBuf>` containing paths to saved image files. Non-image attachments are skipped.

**Logic:**
1. Iterate over attachments.
2. Skip any attachment whose `attachment_type` is not `AttachmentType::Image`.
3. Skip zero-byte attachment data (`data.is_empty()`).
4. For each image attachment, create file via `File::create` + `write_all` + `sync_all` for guaranteed disk flush.
5. Log written file size at debug level, failures at warn level.
6. Collect and return the paths of successfully written files.

### `fn cleanup_inbox_images(paths: &[PathBuf])`
**Purpose:** Remove temporary inbox image files after the provider response has been processed.

**Parameters:**
- `paths: &[PathBuf]` - Slice of file paths to remove.

**Returns:** None (void).

**Logic:**
1. Iterate over paths.
2. Remove each file via `std::fs::remove_file()`.
3. Log failures at warn level but continue removing remaining files.

### `fn status_messages(lang: &str) -> (&'static str, &'static str)`
**Purpose:** Return localized status messages for the delayed provider nudge.

**Parameters:**
- `lang: &str` - Language name (e.g., "English", "Spanish").

**Returns:** Tuple of `(first_nudge, still_working)` static strings.

**Logic:** Match on language name. Supports English, Spanish, Portuguese, French, German, Italian, Dutch, Russian. Unknown languages fall back to English.

### `fn friendly_provider_error(raw: &str) -> String`
**Purpose:** Map raw provider error messages to user-friendly messages.

**Parameters:**
- `raw: &str` - The raw error message from the provider or task join error.

**Returns:** `String` - A friendly, user-facing error message.

**Logic:**
1. If `raw` contains "timed out" or similar timeout indicators, return "I took too long to respond. Please try again..."
2. Otherwise, return "Something went wrong. Please try again."

### `fn is_within_active_hours(start: &str, end: &str) -> bool`
**Purpose:** Check if the current local time is within the active hours window.

**Parameters:**
- `start: &str` - Start time in `"HH:MM"` format.
- `end: &str` - End time in `"HH:MM"` format.

**Logic:**
1. Get current local time as `"HH:MM"` string via `chrono::Local::now()`.
2. If `start <= end` (normal range, e.g., `"08:00"` to `"22:00"`): return `now >= start && now < end`.
3. If `start > end` (midnight wrap, e.g., `"22:00"` to `"06:00"`): return `now >= start || now < end`.

**Dependencies:** `chrono` crate for local time formatting.

## Message Flow Diagram

```
[Telegram/WhatsApp] → [Channel Receiver] → [MPSC Channel]
                                               ↓
                                        [Gateway Event Loop]
                                               ↓
                                          [handle_message()]
                                               ↓
                    ┌──────────────────────────┼──────────────────────────┐
                    ↓                          ↓                          ↓
            [1. Auth Check]         [2. Sanitize]            [3. Command?]
                    ↓                          ↓                          ↓
              [ALLOWED?]              [Clean Text]          [Yes] → [Handle Cmd]
              /        \                      ↓                      [Send Response]
          [No]        [Yes]          [4. Typing Indicator]          [Return]
            ↓            ↓                      ↓
        [Deny]      [Continue]         [Spawn Repeat Task]
         [Audit]         ↓                     ↓
        [Send Msg]   [5. Build Context]     [Continue]
        [Return]          ↓                     ↓
                    [With History + Facts] [5a. Provider.complete()]
                           ↓                   ↓
                      [Enriched CTX]    [Get Response/Error]
                           ↓                   ↓
                           └───────────────────┤
                                               ↓
                                          [Error?]
                                          /      \
                                      [Yes]     [No]
                                        ↓         ↓
                                    [Send Err] [Abort Typing]
                                    [Audit Err]    ↓
                                    [Abort Type]  [5b. Extract SCHEDULE:]
                                    [Return]       ↓
                                            [create_task() if marker]
                                            [strip marker from response]
                                                   ↓
                                            [6. Store Exchange]
                                                   ↓
                                            [7. Audit Log (Success)]
                                                   ↓
                                            [8. Send Response]
                                                   ↓
                                                [Done]

--- Background Tasks ---

[Scheduler Loop]   ←── polls every poll_interval_secs
       ↓
  get_due_tasks()
       ↓
  [For each due task] → channel.send("Reminder: ...")
       ↓                       ↓
  complete_task()         [Advance due_at for recurring]

[Heartbeat Loop]   ←── polls every interval_minutes * 60s
       ↓
  is_within_active_hours()
       ↓ (if active)
  read_heartbeat_file()
       ↓ (None → skip, no API call)
  enrich prompt with memory.get_all_facts() + memory.get_all_recent_summaries(3)
       ↓
  provider.complete(enriched heartbeat prompt)
       ↓
  [HEARTBEAT_OK?] → suppress / [Alert?] → channel.send(alert)
```

## Error Handling Strategy

### Error Propagation Levels

1. **Critical Errors (Early Return):**
   - Channel startup failure → breaks gateway initialization.
   - Auth denial → deny message sent, audit logged, message dropped.
   - Context build failure → error message sent, message dropped.
   - Provider error → error message sent, audit logged, message dropped.

2. **Non-Critical Errors (Log and Continue):**
   - Memory store errors → logged, response still sent if provider succeeded.
   - Audit logging errors → logged, processing continues.
   - Channel send errors → logged, does not block completion.
   - Background summarization errors → logged, loop continues.
   - Idle conversation query errors → logged, loop continues.

3. **Error Auditing:**
   - All auth denials are logged with `AuditStatus::Denied`.
   - All provider errors are logged with `AuditStatus::Error`.
   - All successful exchanges are logged with `AuditStatus::Ok`.

### Error Types Used
- `anyhow::anyhow!()` for wrapping errors in run().
- `anyhow::Error` for Result types.
- `sqlx` errors from database operations (caught with `.ok().flatten()`).
- Tracing logs: `error!()`, `warn!()`, `info!()`.

## Async Runtime Patterns

### Concurrency Model
- **Non-blocking message dispatch:** The gateway dispatches messages via `dispatch_message()` which spawns a `tokio::spawn()` task per sender. Messages for different senders are processed concurrently; messages for the same sender are serialized via the `active_senders` buffer.
- **Multiple channels:** Each channel listener runs in its own `tokio::spawn()` task.
- **Background summarizer:** Runs in a dedicated `tokio::spawn()` task.
- **Scheduler loop:** Conditionally runs in a dedicated `tokio::spawn()` task (when `scheduler_config.enabled`).
- **Heartbeat loop:** Conditionally runs in a dedicated `tokio::spawn()` task (when `heartbeat_config.enabled`).
- **CLAUDE.md maintenance:** Conditionally runs in a dedicated `tokio::spawn()` task (when provider is `"claude-code"`). On startup, `ensure_claudemd` is spawned as a one-shot task. `claudemd_loop` runs every 24 hours.
- **Typing repeater:** For each message, a separate `tokio::spawn()` task repeats typing every 5 seconds.
- **Provider task:** For each message, `provider.complete()` is spawned via `tokio::spawn()` to run in the background.
- **Status updater:** For each message, a delayed status updater task is spawned that sends a localized first nudge after 15 seconds, then periodic localized "Still working..." messages every 120 seconds; aborted when the provider result arrives. If the provider responds within 15 seconds, no status message is sent. Messages are localized to the user's `preferred_language` fact via `status_messages()`.

### Synchronization
- **MPSC Channel:** All incoming messages from channels are collected on a single 256-capacity mpsc queue.
- **Arc Sharing:** Provider, channels, and the gateway itself are shared via `Arc` for thread-safe access.
- **Mutex:** The `active_senders` field uses a `tokio::sync::Mutex` to coordinate per-sender message buffering across concurrent tasks. The lock is held only briefly for buffer check/update operations, never across async provider calls.

### Shutdown Coordination
- **Signal handling:** `tokio::signal::ctrl_c()` breaks the main loop.
- **Task abortion:** Background summarizer is aborted via `bg_handle.abort()`. Scheduler, heartbeat, and CLAUDE.md maintenance handles are aborted if present.
- **Channel stopping:** Each channel's `stop()` method is called.
- **Graceful conversation closure:** All active conversations are summarized before shutdown completes.

## Channel Integration

### Channel Trait Requirements
- **`start() -> Result<mpsc::Receiver<IncomingMessage>>`:** Returns a receiver for messages from that channel. Must be called once at gateway startup.
- **`send(OutgoingMessage) -> Result<()>`:** Sends a message back to the user.
- **`send_typing(target) -> Result<()>`:** Sends a typing indicator. Called repeatedly every 5 seconds.
- **`stop() -> Result<()>`:** Cleanly shuts down the channel.

### Telegram Integration Details
- Auth is enforced via `channel_config.telegram.allowed_users` list.
- Empty list allows all users (for testing).
- Non-empty list restricts to specified user IDs.
- `sender_id` is parsed as i64.

## Provider Integration

### Provider Trait Requirements
- **`name() -> &'static str`:** Returns the provider name (e.g., "Claude Code CLI").
- **`complete(Context) -> Result<Response>`:** Takes a context with full prompt and returns a response.

### Response Metadata
The response includes:
- `text: String` - The assistant's reply.
- `metadata.provider_used: String` - Name of the provider (e.g., "Claude Code CLI").
- `metadata.model: Option<String>` - Model name (e.g., "claude-opus-4-6").
- `metadata.processing_time_ms: u32` - Time taken by the provider.
- `reply_target: Option<String>` - Set to incoming message's reply_target for threading.

## Memory Integration

### Store Operations Used
- **`build_context(&IncomingMessage, &str, &ContextNeeds) -> Result<Context>`:** Builds enriched context with history and facts, using the provided base system prompt. The `ContextNeeds` parameter gates expensive DB queries (semantic recall, pending tasks) — when a field is `false`, the corresponding query is skipped entirely.
- **`store_exchange(&IncomingMessage, &OutgoingMessage) -> Result<()>`:** Saves the message pair to conversation history.
- **`store_fact(&sender_id, &key, &value) -> Result<()>`:** Stores an extracted fact about a user.
- **`get_conversation_messages(&conversation_id) -> Result<Vec<(String, String)>>`:** Fetches all messages in a conversation.
- **`close_conversation(&conversation_id, &summary) -> Result<()>`:** Marks a conversation as closed with a summary.
- **`find_idle_conversations() -> Result<Vec<(String, String, String)>>`:** Finds conversations inactive for a threshold.
- **`find_all_active_conversations() -> Result<Vec<(String, String, String)>>`:** Finds all currently active conversations.
- **`pool() -> &SqlitePool`:** Provides direct database access for queries.
- **`create_task(&channel, &sender_id, &reply_target, &description, &due_at, repeat) -> Result<String>`:** Creates a scheduled task. Called from handle_message Stage 5b.
- **`get_due_tasks() -> Result<Vec<(String, String, String, String, Option<String>)>>`:** Fetches tasks where status is pending and due_at <= now. Called by scheduler_loop.
- **`complete_task(&id, repeat) -> Result<()>`:** Marks a one-shot task as delivered or advances due_at for recurring tasks. Called by scheduler_loop.
- **`get_all_facts() -> Result<Vec<(String, String)>>`:** Gets all facts across all users (excluding `welcomed`). Called by heartbeat_loop for context enrichment.
- **`get_all_recent_summaries(limit) -> Result<Vec<(String, String)>>`:** Gets recent conversation summaries across all users. Called by heartbeat_loop with `limit = 3` for context enrichment.

## Configuration Parameters

### AuthConfig
- `enabled: bool` - Whether authentication is enforced.
- `deny_message: String` - Message to send when auth fails.

### ChannelConfig
- `telegram: Option<TelegramConfig>` - Telegram-specific settings.
  - `allowed_users: Vec<i64>` - Whitelist of user IDs. Empty = allow all.

### HeartbeatConfig
- `enabled: bool` - Whether the heartbeat loop is spawned.
- `interval_minutes: u64` - Minutes between heartbeat checks (default: 30).
- `active_start: String` - Start of active hours window (`"HH:MM"` format). Empty = always active.
- `active_end: String` - End of active hours window (`"HH:MM"` format). Empty = always active.
- `channel: String` - Channel name for alert delivery (e.g., `"telegram"`).
- `reply_target: String` - Platform-specific delivery target (e.g., chat ID).

### SchedulerConfig
- `enabled: bool` - Whether the scheduler loop is spawned (default: true).
- `poll_interval_secs: u64` - Seconds between scheduler polls (default: 60).

## Logging and Observability

### Log Levels Used
- **INFO:** Gateway startup, channel starts, message previews, conversation summaries, shutdown.
- **WARN:** Auth denials, sanitization warnings, shutdown summarization errors, channel stop errors, conversation summarization errors.
- **ERROR:** Context build failures, provider errors, memory storage errors, channel send errors, idle conversation query errors.

### Audit Logging
All interactions are logged to SQLite with:
- Channel name, sender_id, sender_name.
- Input text and output text.
- Provider name and model.
- Processing time in milliseconds.
- Status (Ok, Denied, Error).
- Denial reason (if denied).

## Security Considerations

1. **Input Sanitization:** All user input is sanitized before reaching the provider to neutralize injection patterns.
2. **Auth Enforcement:** Access control is enforced before any processing begins.
3. **Audit Trail:** All interactions are logged for security review.
4. **No Secrets in Logs:** User text is logged but no API keys or credentials are logged.
5. **Error Suppression:** Detailed errors are logged but user-facing messages are generic to avoid info leaks.

## Performance Characteristics

- **Non-blocking Gateway:** Messages for different senders are processed concurrently via `tokio::spawn()`. Messages for the same sender are serialized to maintain conversation coherence, with buffered messages processed in order after the active call completes.
- **Concurrent Channels:** Multiple channels can deliver messages concurrently via tokio tasks.
- **Background Summarization:** Idle conversation summarization happens every 60 seconds without blocking the main loop.
- **MPSC Buffering:** Up to 256 incoming messages can be buffered while waiting for dispatch.

## Dependencies

### External Crates
- `anyhow` - Error handling.
- `tokio` - Async runtime, synchronization primitives.
- `tracing` - Structured logging.
- `sqlx` - Database queries (via Store).
- `chrono` - Local time formatting for active hours check.

### Internal Dependencies
- `omega_core` - Core types, traits, config (including `Prompts`), sanitization.
- `omega_memory` - Store, AuditLogger, AuditEntry, AuditStatus.
- `crate::commands` - Command parsing and handling.

## Invariants

1. Only one task in the main event loop at a time (tokio::select!).
2. All channels are started before the main loop.
3. The background summarizer is spawned and never joined (infinite loop).
4. Auth is checked before any message processing.
5. Sanitization happens before command dispatch and provider call.
6. Response is sent to the channel that received the message.
7. All exchanges are stored in memory before audit logging.
8. Typing indicator is aborted when response is sent or on error.
9. On shutdown, all active conversations are summarized, all background tasks are aborted, and all channels are stopped.
10. SCHEDULE: markers are stripped from the response before sending to the user.
11. LANG_SWITCH: markers are stripped from the response before sending to the user. The extracted language is persisted as a `preferred_language` fact.
12. Scheduler loop only runs when `scheduler_config.enabled` is true.
13. Heartbeat loop only runs when `heartbeat_config.enabled` is true.
14. Heartbeat suppression is content-aware: `HEARTBEAT_OK` is stripped from the response, and only if no meaningful content remains is the message suppressed. Responses containing both user-facing content and `HEARTBEAT_OK` deliver the content (with the token removed).
15. Status updater is aborted when provider result arrives.
16. Platform formatting hints are injected into the system prompt based on `incoming.channel` (WhatsApp avoids markdown tables/headers; Telegram supports full markdown).
19. Heartbeat loop skips API calls entirely when no checklist file (`~/.omega/prompts/HEARTBEAT.md`) is configured.
20. Heartbeat prompt is enriched with user facts and recent conversation summaries from memory.
21. HEARTBEAT_ADD:, HEARTBEAT_REMOVE:, and HEARTBEAT_INTERVAL: markers are stripped from the response before sending to the user. Adds are appended to `~/.omega/prompts/HEARTBEAT.md`; removes use case-insensitive partial matching and never remove comment lines. HEARTBEAT_INTERVAL: updates the shared `AtomicU64` interval (valid range: 1–1440 minutes) and sends a localized confirmation notification (via `i18n::heartbeat_interval_updated()`) to the owner via the heartbeat channel.
22. The current heartbeat checklist is injected into the system prompt only when the user's message contains monitoring-related keywords ("heartbeat", "watchlist", "monitoring", "checklist"). This prevents token waste on unrelated conversations while ensuring OMEGA has awareness when the user asks about monitoring.
23. Filesystem protection is always-on via `omega_sandbox::protected_command()` and `omega_sandbox::is_write_blocked()` — no configuration needed.
24. After sending the text response, new image files created in the workspace by the provider are delivered via `channel.send_photo()` and then deleted from the workspace.
26. Incoming image attachments are saved to `{data_dir}/workspace/inbox/` before the provider call (Stage 2a). Cleanup is guaranteed by `InboxGuard` (RAII Drop), regardless of early returns. Zero-byte attachments are rejected. Writes use `sync_all` for durability.
27. On startup, `purge_inbox()` deletes all files in the inbox directory to clear orphans from previous runs.
28. Messages for different senders are dispatched concurrently via `tokio::spawn()`. Messages for the same sender are serialized: only one provider call per sender at a time, with additional messages buffered and processed in order.
29. When a message arrives for a busy sender, a "Got it, I'll get to this next." acknowledgment is sent immediately.
30. Classify-and-route: every message triggers a complexity-aware classification call (using the fast model with active project, last 3 messages, and skill names); routine actions (reminders, scheduling, lookups) are always DIRECT regardless of quantity, step lists only for genuinely complex work (code changes, research, building). DIRECT messages use `model_fast`, multi-step plans use `model_complex`. Multi-step plans are executed autonomously with per-step progress, retry (up to 3 attempts), and a final summary.
31. Planning steps are tracked in-memory (ephemeral) and are not persisted to the database.
32. Model routing: `context.model` is set by classify-and-route before the provider call. The provider resolves the effective model via `context.model.as_deref().unwrap_or(&self.model)`.
33. SKILL_IMPROVE: markers (format: `SKILL_IMPROVE: skill_name | description`) are processed after HEARTBEAT markers. The gateway extracts the skill name and improvement description, stores them for review, and strips the marker from the response. Processed in `handle_message` (direct), `execute_steps` (multi-step), and `scheduler_loop` — all via `process_markers()`.
34. All response markers (SCHEDULE, SCHEDULE_ACTION, PROJECT, LANG_SWITCH, PERSONALITY, FORGET_CONVERSATION, CANCEL_TASK, UPDATE_TASK, PURGE_FACTS, REWARD, LESSON, HEARTBEAT, SKILL_IMPROVE, BUG_REPORT) are processed via the unified `process_markers()` method, ensuring they work in both the direct response path (`handle_message`) and the multi-step execution path (`execute_steps`).
35. All system markers must use their exact English prefix regardless of conversation language. The gateway parses markers as literal string prefixes — a translated or paraphrased marker is a silent failure. The system prompt explicitly instructs the AI: "Speak to the user in their language; speak to the system in markers."
36. Conversational command markers (PERSONALITY, FORGET_CONVERSATION, CANCEL_TASK, UPDATE_TASK, PURGE_FACTS) provide zero-friction equivalents of slash commands — users can say "be more casual" instead of `/personality casual`. The AI emits the marker; `process_markers()` handles it identically to the slash command.
37. PURGE_FACTS preserves system fact keys (`welcomed`, `preferred_language`, `active_project`, `personality`) — same logic as `/purge` in `commands.rs`.
38. CANCEL_TASK and UPDATE_TASK use multi-extraction (`extract_all_cancel_tasks()`, `extract_all_update_tasks()`) to process ALL markers in a single response, not just the first. Each marker pushes a `MarkerResult` (TaskCancelled/TaskCancelFailed/TaskUpdated/TaskUpdateFailed) for gateway confirmation display.
39. The scheduler action loop also processes CANCEL_TASK and UPDATE_TASK markers from action task responses, using the same `extract_all_*` multi-extraction functions (with empty sender_id since action tasks run autonomously).
40. Token-efficient prompt architecture: `Prompts.scheduling`, `Prompts.projects_rules`, and `Prompts.meta` are only injected when `kw_match()` detects relevant keywords in the user's message. `ContextNeeds` gates DB queries (recall, pending tasks, summaries, outcomes) and prompt injection (user profile) based on keyword detection. Active project instructions are keyword-gated. Heartbeat interval is only injected when heartbeat keywords match. Lessons are always injected (tiny, high value). Core sections (Identity, Soul, System) are always injected. Average token reduction is ~55-70% for typical messages.
41. REWARD markers (format: `REWARD: +1|domain|lesson`) are processed via multi-extraction. Score must be `-1`, `0`, or `+1`. Stored via `store_outcome()` with source `"conversation"` (regular messages) or `"heartbeat"` (heartbeat responses). Stripped from response before sending.
42. LESSON markers (format: `LESSON: domain|rule`) are processed via multi-extraction. Stored via `store_lesson()` which upserts by `(sender_id, domain)` — existing rules are replaced and occurrences incremented. Stripped from response before sending.
43. Outcomes and lessons are injected into the system prompt by `build_context()` (always, not keyword-gated). Outcomes use relative timestamps via `format_relative_time()`. Lessons show `[domain] rule`. Token budget: ~225-450 tokens for both.

## Tests

### `test_friendly_provider_error_timeout`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `friendly_provider_error()` returns the timeout-specific friendly message when the raw error string contains a timeout indicator.

### `test_friendly_provider_error_generic`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `friendly_provider_error()` returns the generic friendly message ("Something went wrong. Please try again.") when the raw error string does not contain a timeout indicator.

### `test_status_messages_all_languages`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `status_messages()` returns non-empty nudge and still-working strings for all 8 supported languages.

### `test_status_messages_unknown_falls_back_to_english`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `status_messages()` falls back to English for unrecognized language names.

### `test_status_messages_spanish`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `status_messages("Spanish")` returns Spanish-language status messages.

### `test_read_heartbeat_file_returns_none_when_missing`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `read_heartbeat_file()` returns `None` when `~/.omega/prompts/HEARTBEAT.md` does not exist or is empty, confirming the skip-when-no-checklist behavior.

### `test_bundled_system_prompt_contains_identity_soul_system`

**Type:** Synchronous unit test (`#[test]`)

Verifies that the bundled `SYSTEM_PROMPT.md` (via `include_str!`) contains all three sections (`## Identity`, `## Soul`, `## System`) with key phrases from each.

### `test_bundled_facts_prompt_guided_schema`

**Type:** Synchronous unit test (`#[test]`)

Verifies that the bundled `SYSTEM_PROMPT.md` (via `include_str!`) contains the guided fact-extraction schema with canonical fields like "preferred_name", "pronouns", and "timezone".

### `test_extract_heartbeat_add`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `extract_heartbeat_markers()` correctly extracts a single `HEARTBEAT_ADD:` marker from response text.

### `test_extract_heartbeat_remove`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `extract_heartbeat_markers()` correctly extracts a single `HEARTBEAT_REMOVE:` marker from response text.

### `test_extract_heartbeat_multiple`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `extract_heartbeat_markers()` extracts both `HEARTBEAT_ADD:` and `HEARTBEAT_REMOVE:` markers from the same response text.

### `test_extract_heartbeat_empty_ignored`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `extract_heartbeat_markers()` ignores markers with empty descriptions (e.g., `HEARTBEAT_ADD: ` with trailing whitespace only).

### `test_strip_heartbeat_markers`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `strip_heartbeat_markers()` removes `HEARTBEAT_ADD:` lines from response text while preserving other lines.

### `test_strip_heartbeat_both_types`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `strip_heartbeat_markers()` removes both `HEARTBEAT_ADD:` and `HEARTBEAT_REMOVE:` lines from the same response text.

### `test_extract_heartbeat_interval`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `extract_heartbeat_markers()` correctly extracts a `HEARTBEAT_INTERVAL:` marker as `HeartbeatAction::SetInterval(15)`.

### `test_extract_heartbeat_interval_invalid`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `extract_heartbeat_markers()` ignores invalid interval values: zero, negative, non-numeric, and values above 1440. Confirms boundary values (1 and 1440) are accepted.

### `test_strip_heartbeat_interval`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `strip_heartbeat_markers()` removes `HEARTBEAT_INTERVAL:` lines from response text while preserving other lines.

### `test_extract_heartbeat_mixed`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `extract_heartbeat_markers()` extracts all three marker types (`HEARTBEAT_INTERVAL:`, `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`) from the same response text in the correct order.

### `test_apply_heartbeat_add`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `apply_heartbeat_changes()` adds new items to `~/.omega/prompts/HEARTBEAT.md`, preserves existing items, and prevents duplicate adds (case-insensitive). Uses a temporary directory with overridden `$HOME`.

### `test_apply_heartbeat_remove`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `apply_heartbeat_changes()` removes matching items from `~/.omega/prompts/HEARTBEAT.md` using case-insensitive partial matching, preserves comment lines, and keeps non-matching items. Uses a temporary directory with overridden `$HOME`.

### `test_snapshot_workspace_images_finds_images`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `snapshot_workspace_images()` finds `.png` and `.jpg` files but ignores `.txt` files.

### `test_snapshot_workspace_images_empty_dir`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `snapshot_workspace_images()` returns an empty set for an empty directory.

### `test_snapshot_workspace_images_nonexistent_dir`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `snapshot_workspace_images()` returns an empty set gracefully for a non-existent directory.

### `test_snapshot_workspace_images_all_extensions`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `snapshot_workspace_images()` detects all 5 supported image extensions (png, jpg, jpeg, gif, webp).

### `test_ensure_inbox_dir`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `ensure_inbox_dir()` creates the `{data_dir}/workspace/inbox/` directory and returns the correct path. Uses a temporary directory.

### `test_save_and_cleanup_inbox_images`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `save_attachments_to_inbox()` writes Image-type attachments to disk, returns the correct paths, and that `cleanup_inbox_images()` removes the files afterwards. Uses a temporary directory.

### `test_save_attachments_skips_non_images`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `save_attachments_to_inbox()` skips non-Image attachment types (e.g., audio, document) and only saves Image-type attachments.

### `test_save_attachments_rejects_empty_data`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `save_attachments_to_inbox()` rejects zero-byte image attachments (empty `data` vec).

### `test_inbox_guard_cleans_up_on_drop`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `InboxGuard` cleans up inbox files when dropped (RAII pattern). Creates a temp file, wraps its path in an `InboxGuard`, and confirms the file is deleted after the guard goes out of scope.

### `test_inbox_guard_empty_is_noop`

**Type:** Synchronous unit test (`#[test]`)

Verifies that an `InboxGuard` with an empty path list does not panic or error on drop.

### `test_parse_plan_response_direct`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `parse_plan_response()` returns `None` when the response contains "DIRECT" (any case).

### `test_parse_plan_response_numbered_list`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `parse_plan_response()` returns `Some(vec)` for a multi-step numbered list response.

### `test_parse_plan_response_single_step`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `parse_plan_response()` returns `None` when the response contains only a single step.

### `test_parse_plan_response_with_preamble`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `parse_plan_response()` correctly parses a numbered list that has non-numbered preamble text before it, returning the steps while ignoring the preamble.

### `test_detect_repo_path`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `detect_repo_path()` returns a `Some` value containing "omega" when running from within the project directory.

### `test_extract_personality`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_personality()` extracts the value from a `PERSONALITY:` line in multi-line text.

### `test_extract_personality_none`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_personality()` returns `None` when no marker is present.

### `test_extract_personality_empty`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_personality()` returns `None` when the value after `PERSONALITY:` is empty.

### `test_extract_personality_reset`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_personality()` returns `Some("reset")` for the reset case.

### `test_strip_personality`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `strip_personality()` removes `PERSONALITY:` lines while preserving other content.

### `test_has_forget_marker`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `has_forget_marker()` returns `true` when `FORGET_CONVERSATION` is present.

### `test_has_forget_marker_false`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `has_forget_marker()` returns `false` when no marker is present.

### `test_has_forget_marker_partial_no_match`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `has_forget_marker()` rejects partial matches like `FORGET_CONVERSATION_EXTRA`.

### `test_strip_forget_marker`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `strip_forget_marker()` removes `FORGET_CONVERSATION` lines while preserving other content.

### `test_extract_all_cancel_tasks_single`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_cancel_tasks()` extracts a single task ID prefix from a `CANCEL_TASK:` line.

### `test_extract_all_cancel_tasks_none_found`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_cancel_tasks()` returns an empty vec when no marker is present.

### `test_extract_all_cancel_tasks_empty_value`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_cancel_tasks()` returns an empty vec when the value after `CANCEL_TASK:` is empty.

### `test_extract_all_cancel_tasks_multiple`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_cancel_tasks()` extracts multiple task ID prefixes when several `CANCEL_TASK:` lines are present.

### `test_extract_all_cancel_tasks_skips_empty`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_cancel_tasks()` skips `CANCEL_TASK:` lines with empty values while still collecting valid ones.

### `test_strip_cancel_task`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `strip_cancel_task()` removes `CANCEL_TASK:` lines while preserving other content.

### `test_extract_all_update_tasks_single_line`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_update_tasks()` extracts a single `UPDATE_TASK:` line from response text.

### `test_extract_all_update_tasks_none_found`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_update_tasks()` returns an empty vec when no marker is present.

### `test_parse_update_task_line_all_fields`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `parse_update_task_line()` extracts all four fields (id, desc, due_at, repeat) from a complete `UPDATE_TASK:` line.

### `test_parse_update_task_line_empty_fields`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `parse_update_task_line()` returns `None` for empty fields (between pipes), representing "keep existing".

### `test_parse_update_task_line_only_description`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `parse_update_task_line()` extracts only the description when other fields are empty.

### `test_parse_update_task_line_invalid`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `parse_update_task_line()` returns `None` for malformed lines (missing pipes, empty id, non-matching prefix).

### `test_extract_all_update_tasks_multiple`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_all_update_tasks()` extracts multiple `UPDATE_TASK:` lines from the same response text.

### `test_strip_update_task`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `strip_update_task()` removes `UPDATE_TASK:` lines while preserving other content.

### `test_has_purge_marker`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `has_purge_marker()` returns `true` when `PURGE_FACTS` is present.

### `test_has_purge_marker_false`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `has_purge_marker()` returns `false` when no marker is present.

### `test_has_purge_marker_partial_no_match`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `has_purge_marker()` rejects partial matches like `PURGE_FACTS_EXTRA`.

### `test_strip_purge_marker`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `strip_purge_marker()` removes `PURGE_FACTS` lines while preserving other content.

### `test_kw_match_scheduling`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `kw_match()` matches scheduling keywords ("remind", "schedule", "alarm", "cancel") and rejects non-scheduling messages ("good morning", "how are you today").

### `test_kw_match_recall`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `kw_match()` matches recall keywords ("remember", "you told", "you mentioned") and rejects non-recall messages ("hello omega").

### `test_kw_match_tasks`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `kw_match()` matches task keywords ("tasks", "scheduled", "pending reminders") and rejects non-task messages.

### `test_kw_match_projects`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `kw_match()` matches project keywords ("activate", "deactivate", "project") and rejects non-project messages.

### `test_kw_match_meta`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `kw_match()` matches meta keywords ("skill", "improve", "bug", "whatsapp", "personality") and rejects non-meta messages.
