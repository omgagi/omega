# Gateway Architecture & Message Flow

## Overview

The **Gateway** is the central orchestrator of Omega's event loop, implemented as a directory module at `src/gateway/` with 12 files. It sits at the intersection of:
- **Messaging channels** (Telegram, WhatsApp) — where users send messages.
- **AI providers** (Claude Code CLI, Anthropic API, etc.) — where reasoning happens.
- **Memory store** (SQLite) — where conversation history and user facts are persisted.
- **Audit system** — where all interactions are logged for security and debugging.

The gateway's job is simple: listen for messages, process them through a deterministic pipeline, get a response from an AI provider, store the exchange, and send the response back to the user.

## Module Structure

The gateway was modularized from a single `src/gateway.rs` into `src/gateway/` with the following files:

| File | Responsibility |
|------|----------------|
| `mod.rs` | Gateway struct, `new()`, `run()`, `dispatch_message()`, `shutdown()`, `send_text()` |
| `pipeline.rs` | `handle_message()` — the full message processing pipeline, `build_system_prompt()` |
| `routing.rs` | `classify_and_route()`, `execute_steps()`, `handle_direct_response()` |
| `process_markers.rs` | `process_markers()`, `send_task_confirmation()` |
| `auth.rs` | `check_auth()`, `handle_whatsapp_qr()` |
| `scheduler.rs` | `scheduler_loop()` — background task delivery |
| `heartbeat.rs` | `heartbeat_loop()` — periodic AI check-ins |
| `summarizer.rs` | `summarize_and_extract()`, `background_summarizer()`, `summarize_conversation()`, `handle_forget()` |
| `keywords.rs` | Keyword constants (`SCHEDULING_KW`, `RECALL_KW`, etc.), `kw_match()`, `is_valid_fact()` |

All struct fields use `pub(super)` visibility, keeping the public API unchanged.

## Conceptual Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         GATEWAY EVENT LOOP                          │
│                                                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Incoming Messages (via MPSC)                                  │  │
│  │                                                                 │  │
│  │ Telegram → Channel Listener → ┐                               │  │
│  │ WhatsApp → Channel Listener → ├→ MPSC Queue → Main Loop       │  │
│  │                                                                 │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Main Event Loop (tokio::select!)                              │  │
│  │ • Wait for message from MPSC                                  │  │
│  │ • Wait for Ctrl+C shutdown signal                             │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Background Tasks (concurrent, non-blocking)                   │  │
│  │ • Conversation Summarizer (every 60s)                         │  │
│  │ • Scheduler Loop (polls every 60s for due tasks)              │  │
│  │ • Heartbeat Loop (check-in every N minutes)                   │  │
│  │ • Typing Indicators (every 5s per message)                    │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                       │
└─────────────────────────────────────────────────────────────────────┘
```

## The Message Processing Pipeline

When a user sends a message, it travels through the gateway in eight sequential stages. Understanding each stage is key to understanding how Omega works.

### Stage 1: Authentication Check

**What happens:** The gateway verifies that the sender is authorized to use Omega.

**Implementation:**
- Calls `check_auth()` which examines:
  - Which channel the message came from (Telegram, WhatsApp, etc.).
  - Per-channel allow-lists (e.g., Telegram user IDs).
- Empty allow-lists default to "allow all" (useful for testing).
- Non-empty allow-lists are strict whitelists.

**On Failure:**
- The message is rejected immediately.
- A denial message is sent back to the user.
- The denial is logged in the audit system with `AuditStatus::Denied`.
- The message never reaches the provider.

**Security Model:**
This is a simple but effective defense. Omega will not process messages from unauthorized users, preventing unauthorized access to your AI assistant.

### Stage 2: Input Sanitization

**What happens:** User input is cleaned to prevent injection attacks and prompt manipulation.

**Implementation:**
- Calls `sanitize()` from `omega_core`.
- Detects patterns that could break out of the system prompt or manipulate the AI backend.
- Returns the cleaned text and a list of detected issues.

**Examples of What Gets Sanitized:**
- Control sequences (newlines in unexpected places).
- Attempts to override the system prompt.
- Shell metacharacters if the backend were to execute commands.

**Result:**
- Input text is replaced with the sanitized version.
- If sanitization modified the text, a warning is logged.
- All subsequent processing uses the clean text.

**Security Model:**
Sanitization is a defense-in-depth measure. Even if an injection pattern gets through, it's neutralized before reaching the AI provider.

### Stage 2a: Inbox Image Save

**What happens:** If the incoming message has image attachments, the gateway saves them to a local inbox directory and prepends their paths to the message text so the AI provider can access them.

**Implementation:**
- Calls `ensure_inbox_dir(data_dir)` to create `{data_dir}/workspace/inbox/` if it does not exist.
- Calls `save_attachments_to_inbox(&inbox_dir, &incoming.attachments)` to write Image-type attachments to disk. Non-image attachments and zero-byte data are skipped. Writes use `File::create` + `write_all` + `sync_all` for guaranteed disk flush.
- For each saved file, a line `[Attached image: /full/path.jpg]` is prepended to the sanitized message text.
- Saved paths are wrapped in an `InboxGuard` (RAII) that guarantees cleanup on Drop — regardless of which early return path `handle_message` takes.

**Why This Exists:**
When users send images via Telegram or WhatsApp, the channel layer downloads them as in-memory byte arrays. The AI provider (Claude Code CLI) cannot access raw bytes from the message struct, but it can read files from disk using its built-in Read tool. By saving images to the inbox directory and referencing them by path in the message text, the provider gains the ability to view and reason about user-sent images.

**Error Handling:**
If writing an attachment to disk fails, the error is logged at warn level and the attachment is skipped. The message continues processing without the failed attachment.

### Stage 2b: Welcome Check (First-Time Users)

**What happens:** The gateway detects first-time users, sends them a welcome message, and then continues processing their message normally through the rest of the pipeline.

**Implementation:**
- Calls `memory.is_new_user()` to check if the sender has interacted before.
- If the user is new:
  - Detects the user's language from the message text.
  - Sends the appropriate welcome message from `WELCOME.toml` (privacy-focused messaging).
  - Stores a `welcomed` fact and the detected `preferred_language` fact.
  - Processing **continues** -- the user's first message falls through to the normal AI pipeline (context building, provider call, etc.), so new users get both a welcome greeting and an AI response to their first message.

**Why This Exists:**
New users should be greeted warmly and informed about the agent's privacy stance, but their first message should not be discarded. Sending the welcome and then processing the message normally ensures the user gets an immediate, useful response alongside the greeting.

### Stage 3: Command Dispatch

**What happens:** The gateway checks if the input is a bot command rather than a regular message.

**Implementation:**
- Calls `commands::Command::parse()` to extract command intent.
- Built-in commands include:
  - `/uptime` - How long Omega has been running.
  - `/help` - List available commands.
  - `/status` - System health information.
  - `/facts` - Retrieve stored facts about the user.
  - `/memory` - Retrieve conversation history.

**On Command Match:**
- The command is handled locally without calling the AI provider.
- A response is returned immediately.
- The message processing stops here (provider is never called).
- **Special case — `/forget`:** The gateway intercepts `/forget` before the normal command dispatch. It closes the conversation instantly and returns a localized confirmation (sub-second). Summarization and fact extraction run in the background via `summarize_and_extract()` — a single combined provider call that updates the closed conversation with a summary and stores extracted facts.

**Why This Exists:**
Commands are fast, deterministic, and don't require AI reasoning. They provide system introspection without API latency or cost.

### Stage 3b: Platform Formatting Hint

**What happens:** A platform-specific formatting hint is appended to the system prompt.

**Implementation:**
- For **WhatsApp**: "Avoid markdown tables and headers — use bold and bullet lists instead."
- For **Telegram**: "Markdown is supported (bold, italic, code blocks)."
- Other channels receive no hint.

**Why This Exists:**
Different platforms render text differently. WhatsApp does not support markdown tables or headers, while Telegram has full markdown support. Telling the AI about the platform prevents it from producing formatting that looks broken on the user's end.

### Stage 3c: Filesystem Protection (Always-On)

**What happens:** Filesystem protection is always active via `omega_sandbox`'s blocklist approach. There is no configuration and no modes to select.

**Implementation:**
- `omega_sandbox::protected_command()` wraps subprocess execution with OS-level protection (Seatbelt on macOS, Landlock on Linux), blocking writes to dangerous system directories and OMEGA's core database.
- `omega_sandbox::is_write_blocked()` checks paths at the tool level, denying writes to protected locations.
- The Claude Code CLI is started with `current_dir` set to `~/.omega/workspace/`.

**Why This Exists:**
The always-on blocklist approach provides OS-level write enforcement without requiring users to choose a security mode. Protection is automatic — dangerous system directories and the OMEGA database are blocked, while the workspace directory (`~/.omega/workspace/`) and `/tmp` remain writable.

### Stage 3d: Group Chat Rules

**What happens:** If the message is from a group chat, additional behavior rules are injected into the system prompt.

**Implementation:**
- When `incoming.is_group` is `true`, the gateway appends rules instructing the AI to:
  - Only respond when directly mentioned, asked a question, or when it can add genuine value.
  - Not leak personal facts from private conversations into the group.
  - Reply with exactly `SILENT` if the message does not warrant a response.

**Why This Exists:**
In group chats, an AI that responds to every message is noisy and annoying. The group rules create a "speak when spoken to" behavior model. The `SILENT` keyword provides a clean mechanism for the AI to signal "I have nothing useful to add" without sending an empty message.

### Stage 4: Typing Indicator

**What happens:** The gateway tells the channel that Omega is thinking.

**Implementation:**
- Gets the channel that received the message.
- Sends an initial typing action immediately.
- Spawns a background task that repeats the typing action every 5 seconds.
- The repeater runs concurrently while processing the message.

**Why This Exists:**
Users expect to see "typing" indicators on messaging platforms. Without them, it looks like Omega is broken or hung. The repeater keeps the indicator visible during long provider calls.

**Cleanup:**
- When the response is ready, the repeater task is aborted.
- If an error occurs during processing, the repeater is aborted early.

### Stage 5: Context Building

**What happens:** The gateway builds a rich context for the AI provider, including conversation history and user facts.

**Implementation:**
- The system prompt is composed from three separate sections: `format!("{}\n\n{}\n\n{}", prompts.identity, prompts.soul, prompts.system)`. Identity defines who the agent is, Soul defines personality and communication style, and System defines operational rules. Platform hints, sandbox rules, group chat rules, and project instructions are appended to this composed prompt.
- Calls `memory.build_context(&incoming, &system_prompt)`.
- The context includes:
  - The user's current message.
  - Recent conversation history (previous exchanges in the same thread).
  - Stored facts about the user (name, preferences, etc.).
  - A system prompt guiding the AI to be helpful and safe.

**Project Instructions:**
Projects are hot-reloaded from disk on every message. If the user has an active project (set via `/project <name>` or autonomously via `PROJECT_ACTIVATE:` marker), the project's ROLE.md instructions are appended to the system prompt (after identity/soul/system) with an `[Active project: <name>]` label before context building. This ensures OMEGA's core identity is established first, with project domain expertise layered on top as supplementary context. The AI can also autonomously create projects and activate/deactivate them using `PROJECT_ACTIVATE: <name>` and `PROJECT_DEACTIVATE` markers in its response — these are stripped before delivery to the user.

**Why This Exists:**
Raw AI models are stateless. They have no memory of previous conversations. The context gives the AI a chance to be conversational and personalized.

**Example:**
```
# Identity + Soul + System (composed)
You are OMEGA, a personal AI agent running on the owner's infrastructure...

# User Facts
- Name: Alice
- Timezone: America/Los_Angeles
- Preference: Brief, direct responses

# Recent History
User: What's the weather?
Assistant: I don't have real-time weather data, but you can check...
User: What about next week?
Assistant: You'd need to check a weather service like...

# Current Message
User: Thanks. What about my location?
```

**Error Handling:**
If context building fails (e.g., database error), an error message is sent immediately and the message is dropped. The provider is never called.

### Stage 5b: MCP Trigger Matching

**What happens:** The gateway checks if the user's message matches any skill-declared trigger keywords and, if so, attaches the corresponding MCP servers to the context.

**Implementation:**
1. Call `omega_skills::match_skill_triggers(&self.skills, &clean_incoming.text)`.
2. This performs case-insensitive substring matching against pipe-separated keywords in each skill's `trigger` field.
3. Only available skills (all required CLIs installed) are considered.
4. Matched MCP servers are deduplicated by server name.
5. The resulting `Vec<McpServer>` is set on `context.mcp_servers`.

**Why This Exists:**
MCP servers extend Claude Code with tools like browser automation (Playwright). Rather than loading all MCP servers on every invocation (which adds token overhead), triggers ensure servers are only activated when the user's message indicates they're needed. For example, "browse google.com" activates the Playwright MCP server, but "what's the weather?" does not.

**What happens downstream:**
The Claude Code provider reads `context.mcp_servers` and, if non-empty, writes a temporary `{workspace}/.claude/settings.local.json` with the MCP server configuration and adds `mcp__<name>__*` patterns to `--allowedTools`. The settings file is cleaned up after the CLI completes.

### Stage 5c: Autonomous Model Routing (Classify & Route)

**What happens:** Before calling the provider, the gateway always runs a fast complexity-aware classification call. The classifier receives the user's message along with lightweight context (active project, last 3 messages, available skills) and decides whether the message should be handled directly by the fast model (Sonnet) or decomposed into steps for the complex model (Opus). The key distinction is **task complexity, not task count** — multiple routine actions (reminders, scheduling, lookups) are routed to DIRECT, while genuinely complex work (multi-file code changes, deep research, building something new) gets decomposed into steps.

**Implementation:**
1. `classify_and_route()` always runs a fast Sonnet classification call — a lightweight provider call with a complexity-aware prompt enriched with ~90 tokens of context (active project name, last 3 conversation messages truncated to 80 chars, available skill names). No system prompt, no MCP servers, no tool access (`allowed_tools = Some(vec![])` passes `--allowedTools ""` to the CLI), and `max_turns = 25` (generous limit — classification is best-effort, falls through to DIRECT on failure). The prompt explicitly defines DIRECT as covering simple questions, conversations, and routine actions regardless of quantity. Step lists are reserved for genuinely complex work where decomposition adds value (sequential dependencies, multi-file changes, research synthesis). When in doubt, the prompt biases toward DIRECT for faster, cheaper execution.
2. The classification result is parsed:
   - **"DIRECT"** (case-insensitive) → Sonnet handles the response. The context's `model` field is set to the fast model, and the message falls through to the normal provider call (Stage 6).
   - **Single-step list** → Treated as DIRECT. No benefit to decomposition.
   - **Multi-step numbered list** (2+ steps) → Opus executes each step autonomously.
3. If steps are returned, `execute_steps()` runs them autonomously with the complex model:
   - Sets `context.model` to the complex model (Opus) for each step.
   - Announces the plan: "Breaking this into N steps. Starting now."
   - Executes each step in a fresh provider call with the full system prompt, MCP servers, and accumulated context from previous steps.
   - Reports progress after each step: "Step 1/N done: description".
   - Retries failed steps up to 3 times with a 2-second delay between attempts.
   - Sends a final summary when all steps complete.
   - Audits each step individually.
   - Cleans up inbox images and aborts the typing indicator.
   - **Returns immediately** — the normal provider call (Stage 6) is skipped entirely.

**Why This Exists:**
Not all messages need the most powerful (and expensive) model. Simple questions, greetings, routine actions (even batches of 12 reminders), and direct requests are handled quickly and cheaply by Sonnet. Only genuinely complex work — multi-file code changes, deep research requiring synthesis, building something new, or tasks with sequential dependencies — is routed to Opus. The classification considers **task complexity, not task count**, preventing over-escalation of simple multi-item requests. The classification call itself is fast and cheap — it uses Sonnet with ~90 tokens of context but no system prompt and no MCP overhead. The injected context (active project, recent messages, skills) prevents misclassification of vague messages that depend on conversational state.

**Design Characteristics:**
- The classification call always runs (the old `needs_planning()` word-count threshold is removed).
- The classification prompt biases toward DIRECT ("when in doubt, prefer DIRECT") — cheaper and faster is the default.
- The gateway stores two model identifiers: `model_fast` (default: `claude-sonnet-4-6`) and `model_complex` (default: `claude-opus-4-6`).
- Model routing is transparent to the provider — the gateway sets `context.model` before each `complete()` call, and the provider resolves the effective model via `context.model.as_deref().unwrap_or(&self.model)`.
- If the classification call fails for any reason, it falls back to direct execution with the fast model.
- Single-step plans are treated as direct — there is no benefit to wrapping one step in the autonomous execution machinery.

### Important: Markers Are Language-Invariant

All system markers (`SCHEDULE:`, `HEARTBEAT_ADD:`, `HEARTBEAT_INTERVAL:`, `SKILL_IMPROVE:`, `LANG_SWITCH:`, `PROJECT_ACTIVATE:`, `REWARD:`, `LESSON:`, etc.) must always use their exact English prefix, regardless of the conversation language. The gateway parses them as literal string prefixes — if the AI translates or paraphrases a marker (e.g., `INTERVALO_HEARTBEAT: 15` instead of `HEARTBEAT_INTERVAL: 15`), the gateway silently ignores it. The system prompt enforces this: "Speak to the user in their language; speak to the system in markers."

### Stage 6: Provider Call

**What happens:** The gateway sends the enriched context to the AI provider and gets a response, while keeping the user informed about progress.

**Implementation:**
1. **Background provider task** -- The `provider.complete(&context)` call is spawned as a background task. This allows the gateway to monitor progress concurrently.
2. **Delayed status updater** -- The user's `preferred_language` fact is resolved from memory (defaults to English), and localized messages are obtained via `status_messages()`. A separate background task is spawned with a two-phase approach: after 15 seconds of waiting, it sends a localized first nudge. Then, every 120 seconds thereafter, it sends a localized "Still working..." message. If the provider responds within 15 seconds (the common case), the updater is aborted and the user sees no extra messages — just the typing indicator followed by the answer. Supported languages: English, Spanish, Portuguese, French, German, Italian, Dutch, Russian.
3. **Await result** -- The gateway awaits the provider task. When it completes, the status updater is cancelled.

- The provider is typically the Claude Code CLI but can be swapped (OpenAI, Anthropic, Ollama, etc.).
- The provider returns a `Response` with:
  - `text`: The assistant's answer.
  - `metadata.provider_used`: Which provider generated this (for audit logging).
  - `metadata.model`: Which model was used (e.g., "claude-opus-4-6").
  - `metadata.processing_time_ms`: How long the request took.

**Why This Exists:**
This is where the actual AI reasoning happens. Everything else in the pipeline is infrastructure. The delayed status updates ensure the user experience remains clean for quick responses and responsive for long-running provider calls.

**Error Handling:**
If the provider fails, the error is mapped to a friendly user-facing message (no raw error details are leaked to the user). The full error is logged internally with details for debugging. The friendly message is sent to the user and the pipeline stops.

**Performance:**
Provider calls are the slowest part of the pipeline (typically 2-30 seconds, but can take up to 10 minutes for complex agentic tasks). The status updater keeps the user informed during long waits. Everything else is near-instant.

### Stage 5a: SILENT Response Suppression

**What happens:** In group chats, if the AI decides it has nothing useful to add, it responds with `SILENT`. The gateway detects this and drops the response entirely.

**Implementation:**
- After aborting the typing indicator, check if `incoming.is_group` is `true` AND `response.text.trim()` equals `"SILENT"`.
- If so, log the suppression and return immediately — no storage, no audit, no message sent.

**Why This Exists:**
This is the other half of group chat awareness. The group rules (Stage 3d) tell the AI to say `SILENT` when it should stay quiet. This stage enforces the suppression so the user never sees an empty or meaningless response.

### Stage 6b: Schedule Marker Extraction

**What happens:** After the provider responds, the gateway scans the response text for a `SCHEDULE:` marker. If found, a scheduled task is created and the marker line is stripped from the response before the user sees it.

**Implementation:**
- Calls `extract_schedule_marker(&response.text)` to find the first line starting with `SCHEDULE:`.
- If found, calls `parse_schedule_line()` to extract three pipe-separated fields: description, ISO 8601 datetime, and repeat type.
- Calls `store.create_task()` to persist the task in the `scheduled_tasks` table.
- Calls `strip_schedule_marker()` to remove all `SCHEDULE:` lines from the response text.

**Marker Format:**
```
SCHEDULE: Call John | 2026-02-17T15:00:00 | once
SCHEDULE: Stand-up meeting | 2026-02-18T09:00:00 | daily
```

**Repeat Types:** `once`, `daily`, `weekly`, `monthly`, `weekdays`

**Why This Exists:**
The provider is responsible for understanding the user's natural language ("remind me to call John at 3pm") and producing the structured marker. The gateway simply extracts it. This keeps scheduling logic in the AI and parsing logic in the gateway -- each does what it does best.

**Error Handling:**
If the marker is malformed (wrong number of fields, empty description), it is silently ignored. If the database insert fails, the error is logged but the response is still sent. The user always sees their response, even if scheduling fails.

### Stage 6c: Language Switch Extraction

**What happens:** After schedule extraction, the gateway scans the response for a `LANG_SWITCH:` marker. If found, it persists the user's new language preference and strips the marker from the response.

**Implementation:**
- Calls `extract_lang_switch(&response.text)` to find the first line starting with `LANG_SWITCH:`.
- If found, stores the language as a `preferred_language` fact via `store.store_fact()`.
- Calls `strip_lang_switch()` to remove the `LANG_SWITCH:` line from the response.

**Marker Format:**
```
LANG_SWITCH: French
```

**Why This Exists:**
When a user says "speak in French" in a regular message, the AI detects the intent and switches language. The `LANG_SWITCH:` marker tells the gateway to persist this preference so all future conversations use the new language.

**Error Handling:**
If the store fails to persist the language, the error is logged but the response is still sent.

### Stage 6c-2: Personality Marker Extraction

**What happens:** After language switch, the gateway scans for a `PERSONALITY:` marker — the conversational equivalent of `/personality`. If found, it persists or resets the personality preference and strips the marker.

**Implementation:**
- Calls `extract_personality(&response.text)` to find a `PERSONALITY:` line.
- If value is `"reset"` (case-insensitive), deletes the `personality` fact.
- Otherwise, stores the value as the `personality` fact.
- Calls `strip_personality()` to remove the marker from the response.

**Marker Format:**
```
PERSONALITY: casual and friendly
PERSONALITY: reset
```

### Stage 6c-3: Forget Conversation Marker

**What happens:** The gateway scans for a `FORGET_CONVERSATION` marker — the conversational equivalent of `/forget`. If found, it closes the current conversation.

**Implementation:**
- Calls `has_forget_marker(&response.text)`.
- If present, calls `memory.close_current_conversation(channel, sender_id)`.
- Calls `strip_forget_marker()` to remove the marker.

**Marker Format:**
```
FORGET_CONVERSATION
```

### Stage 6c-4: Cancel Task Marker

**What happens:** The gateway scans for ALL `CANCEL_TASK:` markers — the conversational equivalent of `/cancel`. Multiple tasks can be cancelled in a single response.

**Implementation:**
- Calls `extract_all_cancel_tasks(&response.text)` to find all `CANCEL_TASK:` lines.
- For each extracted ID prefix, calls `memory.cancel_task(&id_prefix, sender_id)`.
- Pushes `MarkerResult::TaskCancelled` or `MarkerResult::TaskCancelFailed` for each task into the results vector for gateway confirmation.
- Calls `strip_cancel_task()` to remove all markers.

**Marker Format:**
```
CANCEL_TASK: a1b2c3d4
CANCEL_TASK: e5f6g7h8
```

### Stage 6c-5: Update Task Marker

**What happens:** The gateway scans for ALL `UPDATE_TASK:` markers. Multiple tasks can be updated in a single response. Each marker updates the matching pending task's fields (description, due_at, repeat). Empty fields are left unchanged.

**Implementation:**
- Calls `extract_all_update_tasks(&response.text)` to find all `UPDATE_TASK:` lines.
- For each extracted line, calls `parse_update_task_line()` to extract (id, desc?, due_at?, repeat?).
- Calls `memory.update_task(&id_prefix, sender_id, desc, due_at, repeat)` for each.
- Pushes `MarkerResult::TaskUpdated` or `MarkerResult::TaskUpdateFailed` for each task into the results vector for gateway confirmation.
- Calls `strip_update_task()` to remove all markers.

**Marker Format:**
```
UPDATE_TASK: abc123 | New description | 2026-03-01T09:00:00 | daily
UPDATE_TASK: def456 | | | daily          (changes only recurrence)
```

### Stage 6c-6: Purge Facts Marker

**What happens:** The gateway scans for a `PURGE_FACTS` marker — the conversational equivalent of `/purge`. If found, it deletes all non-system facts (preserving `welcomed`, `preferred_language`, `active_project`, `personality`).

**Implementation:**
- Calls `has_purge_marker(&response.text)`.
- If present:
  - Reads all facts and saves system keys.
  - Calls `memory.delete_facts(sender_id, None)` to delete all facts.
  - Restores system facts.
- Calls `strip_purge_marker()` to remove the marker.

**Marker Format:**
```
PURGE_FACTS
```

**Why These Exist:**
Users shouldn't need to memorize slash commands. When a user says "be more casual", "forget this conversation", "cancel that reminder", "make that reminder daily", or "delete everything you know about me", OMEGA acts via these markers — providing a zero-friction conversational UX.

### Stage 6d: Heartbeat Marker Extraction

**What happens:** After language switch extraction, the gateway scans the response for `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, and `HEARTBEAT_INTERVAL:` markers. If found, the heartbeat checklist file is updated (add/remove), the runtime interval is changed (interval), and the markers are stripped from the response.

**Implementation:**
- Calls `extract_heartbeat_markers(&response.text)` to find all `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, and `HEARTBEAT_INTERVAL:` lines.
- If any markers are found:
  - Calls `apply_heartbeat_changes(&actions)` to update `~/.omega/prompts/HEARTBEAT.md`:
    - **Add**: Appends `- {item}` to the file. Prevents duplicate adds (case-insensitive).
    - **Remove**: Removes lines containing the keyword (case-insensitive partial match). Comment lines (`#`) are never removed.
  - For **SetInterval**: Updates the shared `Arc<AtomicU64>` with the new value and sends a confirmation notification to the owner via the heartbeat channel.
  - Logs each action at INFO level.
  - Calls `strip_heartbeat_markers()` to remove the marker lines from the response.

**Marker Format:**
```
HEARTBEAT_ADD: Check exercise habits
HEARTBEAT_REMOVE: exercise
HEARTBEAT_INTERVAL: 15
```

**Why This Exists:**
Without conversational management, users must manually edit `~/.omega/prompts/HEARTBEAT.md` or `config.toml` to change monitoring. This breaks the conversational flow and makes the heartbeat feature less discoverable. The marker pattern (proven by SCHEDULE and LANG_SWITCH) keeps management invisible to the user.

**Error Handling:**
File write errors are silently ignored. The response is always sent to the user. If `$HOME` is not set, the function returns without action. Invalid interval values (non-numeric, zero, or >1440) are silently ignored.

### Stage 6f: Reward Marker Extraction

**What happens:** The gateway scans the response for all `REWARD:` markers. Each one records a raw interaction outcome in the `outcomes` table, then the markers are stripped from the response.

**Implementation:**
- Calls `extract_all_rewards(&response.text)` to find ALL `REWARD:` lines.
- For each line, calls `parse_reward_line()` to extract `(score, domain, lesson)`.
- Calls `memory.store_outcome(sender_id, &domain, score, &lesson, "conversation")` to persist each outcome.
- Calls `strip_reward_markers()` to remove all `REWARD:` lines from the response.

**Marker Format:**
```
REWARD: +1|training|User completed morning workout on time
REWARD: 0|weather|User acknowledged weather update without action
REWARD: -1|trading|Redundant portfolio check — already reviewed today
```

Score must be `-1`, `0`, or `+1`. Lines with invalid scores or missing fields are silently ignored.

**Why This Exists:**
Outcomes give OMEGA a feedback loop. By recording whether each interaction was helpful, neutral, or annoying, OMEGA builds temporal awareness of what works. The last 15 outcomes are injected into every future conversation with relative timestamps ("3h ago"), so the AI can reason about patterns over time.

### Stage 6g: Lesson Marker Extraction

**What happens:** The gateway scans the response for all `LESSON:` markers. Each one upserts a permanent behavioral rule into the `lessons` table, then the markers are stripped from the response.

**Implementation:**
- Calls `extract_all_lessons(&response.text)` to find ALL `LESSON:` lines.
- For each line, calls `parse_lesson_line()` to extract `(domain, rule)`.
- Calls `memory.store_lesson(sender_id, &domain, &rule)` to upsert the lesson. If a lesson for the same `(sender_id, domain)` exists, the rule is replaced and the `occurrences` counter is incremented.
- Calls `strip_lesson_markers()` to remove all `LESSON:` lines from the response.

**Marker Format:**
```
LESSON: training|User trains Saturday mornings, no need to nag after 12:00
LESSON: trading|Always verify market hours before placing orders
```

Lines with empty domain or rule are silently ignored.

**Why This Exists:**
Lessons are distilled wisdom from repeated outcomes. While outcomes are working memory (recent, time-stamped), lessons are permanent behavioral rules that persist indefinitely. They prevent OMEGA from repeating mistakes it has already learned from. All lessons are injected into every conversation and heartbeat context.

### Stage 7: Memory Storage

**What happens:** The exchange (user input + AI response) is saved to the SQLite database.

**Implementation:**
- Calls `memory.store_exchange(&incoming, &response)`.
- This saves:
  - The user's text (sanitized).
  - The AI's response.
  - Metadata (channel, sender_id, timestamp).
  - Links the exchange to the conversation thread.

**Why This Exists:**
Without persistent memory, Omega forgets every message after it's processed. Storage enables continuity and allows the context builder to fetch history for future messages.

**Error Handling:**
If storage fails, the error is logged but does not block the response. The user gets their answer even if the database is temporarily unavailable. This is intentional: providing service is more important than logging.

### Stage 8: Audit Logging

**What happens:** The interaction is logged for security, compliance, and debugging.

**Implementation:**
- Calls `audit.log(&AuditEntry)` with:
  - Channel name, sender_id, sender_name.
  - Input text and output text (the actual exchange).
  - Provider name and model used.
  - Processing time.
  - Status (Ok, Denied, Error).

**Why This Exists:**
Audit logs answer critical questions:
- Who said what and when?
- Which provider answered which question?
- Were there any errors or denials?
- Is there a pattern of misuse?

**Privacy Note:**
Audit logs include the actual message text. Store them securely and comply with data retention laws.

### Stage 9: Send Response

**What happens:** The response is sent back to the user via the channel that received the message.

**Implementation:**
- Gets the channel by name.
- Calls `channel.send(response)`.
- If the send fails, the error is logged but processing is complete.

**Why This Exists:**
The message must be delivered to the user. If the channel fails (e.g., Telegram API is down), there's nothing to do but log it.

**Error Handling:**
Send errors are logged but do not cause a retry or escalation. The assumption is that the channel will handle retries internally if needed.

### Stage 9b: Workspace Image Diff

**What happens:** After sending the text response, the gateway checks if the provider created any new image files in the workspace and sends them to the user.

**Implementation:**
- Before the provider call, the gateway snapshots all top-level image files in `~/.omega/workspace/` (extensions: `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`).
- After sending the text response, it takes another snapshot and computes the difference (new files).
- For each new image:
  - Reads the file bytes.
  - Calls `channel.send_photo(target, &bytes, filename)` to deliver the image.
  - Deletes the file from the workspace after sending.
  - Logs success at INFO level, failures at WARN level.

**Why This Exists:**
When the provider uses MCP tools like Playwright to take screenshots, the image files are created in the workspace but never delivered to the user. The workspace diff bridges this gap: the user asks for a screenshot, the AI creates it, and the gateway automatically sends it as a photo message.

**Error Handling:**
- If reading the image file fails, the error is logged and the file is skipped.
- If sending the photo fails, the error is logged but the file is still cleaned up.
- A non-existent or unreadable workspace directory returns an empty snapshot (no error).

### Stage 9c: Cleanup Inbox Images

**What happens:** After the response is sent and workspace images are handled, the gateway removes the temporary inbox files that were saved during Stage 2a.

**Implementation:**
- Calls `cleanup_inbox_images(&inbox_images)` with the paths collected during Stage 2a.
- Each file is removed individually via `std::fs::remove_file()`.

**Why This Exists:**
Inbox images are only needed for the duration of a single message processing cycle. The provider reads them during its execution, and once the response has been sent, they serve no further purpose. Cleaning them up prevents the inbox directory from accumulating stale files over time.

**Error Handling:**
If removing a file fails (e.g., already deleted, permission issue), the error is logged at warn level but the cleanup continues for the remaining files.

## Full Pipeline Diagram

```
User sends message on Telegram
         ↓
    [Channel Listener] spawns task to forward to gateway
         ↓
    [MPSC Queue] receives message
         ↓
    [Main Event Loop] selects message from queue
         ↓
┌─────────────────────────────────────────┐
│        handle_message() executes         │
├─────────────────────────────────────────┤
│                                          │
│ Stage 1: check_auth()                   │
│  ✓ Allowed? → Continue                  │
│  ✗ Denied?  → Send deny, audit, return  │
│                                          │
│ Stage 2: sanitize()                     │
│  • Clean input                          │
│  • Replace text with sanitized version  │
│                                          │
│ Stage 2a: inbox image save              │
│  • Save image attachments to inbox/     │
│  • Prepend [Attached image: /path] text │
│                                          │
│ Stage 2b: welcome check (new users)     │
│  • Send welcome message                │
│  • Store language preference            │
│  • Continue processing (no return)      │
│                                          │
│ Stage 3: commands::parse()              │
│  ✓ Is command? → Handle locally, return │
│  ✗ Not command? → Continue              │
│                                          │
│ Stage 3b: Platform formatting hint      │
│  • WhatsApp: avoid tables/headers       │
│  • Telegram: markdown supported         │
│                                          │
│ Stage 3c: Sandbox mode prompt injection │
│  • sandbox: workspace only              │
│  • rx: read+exec host, write workspace  │
│  • rwx: full host access                │
│                                          │
│ Stage 3d: Group chat rules (if group)   │
│  • Only respond when mentioned/asked    │
│  • Don't leak private facts             │
│  • Say SILENT to stay quiet             │
│                                          │
│ Stage 4: send_typing()                  │
│  • Spawn repeater task (every 5s)       │
│                                          │
│ Stage 5: memory.build_context()         │
│  • Fetch history + facts                │
│  ✓ Success? → Continue                  │
│  ✗ Error? → Send error, audit, return   │
│                                          │
│ Stage 5c: Classify & route              │
│  • Context-enriched Sonnet classif.     │
│  ✓ Steps? → Opus executes, return       │
│  ✗ DIRECT? → Sonnet handles, continue   │
│                                          │
│ Stage 6: provider.complete()            │
│  • Spawn provider call as background    │
│  • Spawn delayed status updater         │
│    (first nudge at 15s, then 120s)      │
│  • Await result, cancel updater         │
│  ✓ Success? → Continue                  │
│  ✗ Error? → Friendly msg, audit, return │
│                                          │
│ Stage 5a: SILENT suppression (groups)   │
│  ✓ is_group && SILENT? → Drop, return  │
│  ✗ Otherwise? → Continue               │
│                                          │
│ Stage 6b: extract_schedule_marker()     │
│  • Scan response for SCHEDULE: line     │
│  ✓ Found? → Create task, strip marker   │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6c: extract_lang_switch()         │
│  • Scan response for LANG_SWITCH: line  │
│  ✓ Found? → Store pref, strip marker    │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6c-2: extract_personality()       │
│  • Scan for PERSONALITY: line           │
│  ✓ Found? → Set/reset pref, strip       │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6c-3: has_forget_marker()         │
│  • Scan for FORGET_CONVERSATION         │
│  ✓ Found? → Close conversation, strip   │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6c-4: extract_all_cancel_tasks()  │
│  • Scan for ALL CANCEL_TASK: lines      │
│  ✓ Found? → Cancel each, confirm, strip │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6c-4b: extract_all_update_tasks() │
│  • Scan for ALL UPDATE_TASK: lines      │
│  ✓ Found? → Update each, confirm, strip │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6c-5: has_purge_marker()          │
│  • Scan for PURGE_FACTS                 │
│  ✓ Found? → Purge facts, strip          │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6d: extract_heartbeat_markers()   │
│  • Scan for HEARTBEAT_ADD/REMOVE lines  │
│  ✓ Found? → Update file, strip markers  │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6e: extract_bug_report()          │
│  • Scan for BUG_REPORT: line            │
│  ✓ Found? → Append to BUG.md, strip     │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6f: extract_all_rewards()         │
│  • Scan for ALL REWARD: lines           │
│  ✓ Found? → Store outcomes, strip       │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 6g: extract_all_lessons()         │
│  • Scan for ALL LESSON: lines           │
│  ✓ Found? → Upsert lessons, strip       │
│  ✗ Not found? → Continue                │
│                                          │
│ Stage 7: memory.store_exchange()        │
│  • Save to SQLite (best-effort)         │
│                                          │
│ Stage 8: audit.log()                    │
│  • Log to SQLite                        │
│                                          │
│ Stage 9: channel.send()                 │
│  • Send response via Telegram/WhatsApp  │
│  • Abort typing repeater task           │
│                                          │
│ Stage 9b: Workspace image diff          │
│  • Snapshot images after provider call  │
│  • Send new images via send_photo()     │
│  • Delete sent images from workspace    │
│                                          │
│ Stage 9c: Cleanup inbox images          │
│  • Remove temp inbox files from 2a     │
│                                          │
└─────────────────────────────────────────┘
         ↓
    Message complete, ready for next message
```

## Conversation Lifecycle

Messages are grouped into conversations. A conversation is a thread of related exchanges between a user and Omega.

### Conversation Boundaries

Conversations are isolated by:
- **User** (sender_id).
- **Channel** (Telegram, WhatsApp, etc.).
- **Time** — After a period of inactivity (threshold TBD), a conversation is closed.

### Conversation Summarization

Every 60 seconds, the background summarizer runs:

1. **Find idle conversations** — Find all conversations inactive for N minutes.
2. **Summarize each** — Call the provider to generate a 1-2 sentence summary.
3. **Extract facts** — Call the provider to extract user facts (name, preferences, etc.).
4. **Validate & store facts** — Each extracted fact is validated by `is_valid_fact()` before storing. Rejects system-managed keys (`welcomed`, `preferred_language`, `active_project`, `personality`), numeric keys, price values, pipe-delimited rows, and oversized entries.
5. **Close conversation** — Mark the conversation as closed and store the summary.

**Why Summarization?**

- **Memory efficiency** — Summaries are short; full history is long.
- **Context window management** — Older conversations are summarized into facts, not kept in full.
- **User profiling** — Facts extracted from conversations are reused in future exchanges.

**Example:**

```
Conversation 1 (inactive, 30+ minutes):
User: What's your favorite food?
Assistant: As an AI, I don't eat, but I find it interesting that...
User: Do you think AI will replace humans?
Assistant: It's complex. AI augments human capability...

→ Summarization triggered
→ Summary: "User interested in AI ethics and food. Thoughtful questions."
→ Facts extracted:
   - interested_in: "AI ethics"
   - question_style: "philosophical"

Conversation 2 (current):
User: Any good book recommendations?
Assistant: [builds context with previous facts about philosophical interests]
```

## Scheduler Loop

The scheduler is a background task that delivers due tasks to users. It is spawned at gateway startup when `[scheduler].enabled` is `true` (the default).

### Poll-Deliver-Complete Cycle

Every `poll_interval_secs` seconds (default: 60), the scheduler:

1. **Poll** -- Calls `store.get_due_tasks()` to find all tasks where `status = 'pending'` and `due_at <= now`.
2. **Deliver** -- For each due task:
   - **Reminder tasks:** Sends a message via the task's channel: `"Reminder: {description}"`.
   - **Action tasks:** Executes the task via the AI provider with full tool/MCP access, processes response markers (SCHEDULE, SCHEDULE_ACTION, CANCEL_TASK, UPDATE_TASK, REWARD, LESSON), and delivers the response text to the user.
3. **Complete** -- Calls `store.complete_task()` which either:
   - Marks the task as `'delivered'` if it is a one-shot task (`repeat` is `NULL` or `"once"`).
   - Advances `due_at` to the next occurrence if the task is recurring (daily, weekly, monthly, weekdays).

```
┌──────────┐    ┌──────────┐    ┌───────────────┐
│  Sleep   │───>│  Query   │───>│  For each     │
│  60s     │    │  due     │    │  due task:     │
│          │    │  tasks   │    │  send + mark   │
└──────────┘    └──────────┘    └───────────────┘
     ^                                 │
     └─────────────────────────────────┘
```

### Recurring Tasks

When a recurring task is delivered, `complete_task()` does not mark it as `'delivered'`. Instead, it advances the `due_at` timestamp:

| Repeat | Advance |
|--------|---------|
| `daily` | +1 day |
| `weekly` | +7 days |
| `monthly` | +1 month |
| `weekdays` | +1 day, skipping Saturday and Sunday |

The task remains in `'pending'` status with the new `due_at`, so the next poll cycle will pick it up again at the right time.

### Error Handling

- If a channel is not found for a task, the task is skipped (not marked delivered) and a warning is logged.
- If delivery fails (channel send error), the task is skipped and will be retried on the next poll.
- If `complete_task()` fails, the error is logged. The task may be re-delivered on the next poll (at-least-once delivery).

## Heartbeat Loop

The heartbeat is a background task that performs periodic **active execution** of a checklist. It is spawned at gateway startup when `[heartbeat].enabled` is `true` (disabled by default). Unlike passive review, the heartbeat actively executes each checklist item: reminders/accountability items are sent to the user, system checks are performed, and results are reported. It uses the Opus model and processes response markers identically to the scheduler.

### Check-In Cycle

At each clock-aligned boundary (e.g. :00 and :30 for a 30-minute interval), the heartbeat:

1. **Active Hours Check** -- If `active_start` and `active_end` are configured, checks the current local time. Skips the check if outside the window.
2. **Read Checklist** -- Reads `~/.omega/prompts/HEARTBEAT.md` if it exists. If the file is missing or empty, the entire cycle is **skipped** (no API call). This prevents wasted provider calls when no checklist is configured.
3. **Context Enrichment** -- Before calling the provider, the heartbeat enriches the prompt with:
   - **User facts** from `memory.get_all_facts()` (excluding internal `welcomed` markers).
   - **Recent conversation summaries** from `memory.get_all_recent_summaries(3)`.
   - **Learned behavioral rules** from `memory.get_all_lessons()` (all distilled lessons across all users).
   - **Recent outcomes** from `memory.get_all_recent_outcomes(24, 20)` (last 24h, up to 20 entries).
   This gives the AI provider awareness of who the user is, what they've been working on, what behavioral rules have been learned, and how recent interactions went. **Enrichment is injected BEFORE the checklist template** in the prompt so learned behavioral rules (especially output format constraints) frame the AI's approach before it encounters detailed instructions. The heartbeat template includes an OUTPUT FORMAT override block that makes learned rules binding over default verbosity.
4. **System Prompt** -- Composes the full Identity/Soul/System prompt (plus sandbox constraints) and attaches it to the context, ensuring the AI has proper role boundaries during heartbeat calls.
5. **Model & MCP** -- Sets the Opus model (`model_complex`) for powerful active execution. Matches skill triggers on checklist content to inject relevant MCP servers.
6. **Provider Call** -- Sends the enriched prompt with the full system prompt to the AI provider for active execution.
7. **Process Markers** -- Response markers are processed identically to the scheduler:
   - `SCHEDULE` → creates reminder tasks
   - `SCHEDULE_ACTION` → creates action tasks
   - `HEARTBEAT_ADD/REMOVE/INTERVAL` → updates checklist/interval
   - `CANCEL_TASK` → cancels pending tasks
   - `UPDATE_TASK` → modifies existing tasks
   - `REWARD` → records interaction outcomes to outcomes table (source: "heartbeat")
   - `LESSON` → distills behavioral rules to lessons table
   - All markers are stripped from the response text.
8. **Suppress or Alert** (after marker stripping):
   - The response text is cleaned (markdown `*` and backtick characters are stripped) before checking for `HEARTBEAT_OK`.
   - If the cleaned response contains `HEARTBEAT_OK`, the result is logged at INFO level and no message is sent to the user.
   - If the response contains anything else, an audit entry is logged (`[HEARTBEAT]` prefix) and the response is delivered to the configured channel and reply target.

```
┌──────────┐    ┌─────────┐    ┌───────────┐    ┌───────────┐    ┌───────────┐    ┌──────────┐
│  Sleep   │───>│ Active  │───>│ Read      │───>│ Enrich    │───>│ Set Opus  │───>│ Provider │
│  to next │    │ hours?  │    │ HEARTBEAT │    │ with      │    │ model +   │    │ active   │
│  boundary│    │ Yes ──> │    │ .md       │    │ facts +   │    │ MCP +     │    │ execution│
└──────────┘    │ No: skip│    │ None: skip│    │ summaries │    │ sys prompt│    └──────────┘
     ^          └─────────┘    └───────────┘    └───────────┘    └───────────┘         │
     │                                                                    ┌────────────┴──────┐
     │                                                                    │ Process markers   │
     │                                                                    │ → HEARTBEAT_OK?   │
     │                                                                    │ Yes: log only     │
     │                                                                    │ No: audit + send  │
     │                                                                    └───────────────────┘
     └────────────────────────────────────────────────────────────────────────────────┘
```

### The HEARTBEAT.md File

This is an optional markdown file at `~/.omega/prompts/HEARTBEAT.md`. When present, its contents are included in the prompt sent to the provider. This allows you to define a custom checklist that the AI evaluates on each heartbeat.

**Example:**
```markdown
- Is the system load below 80%?
- Are all Docker containers running?
- Is disk usage below 90%?
- Any errors in /var/log/syslog in the last hour?
```

If the file does not exist or is empty, the heartbeat sends a generic health check prompt instead.

### Active Hours

The `active_start` and `active_end` fields define a time window (in 24-hour `HH:MM` format) during which heartbeats are allowed. Outside this window, the heartbeat sleeps without calling the provider.

- If both fields are empty, heartbeats run around the clock.
- Midnight wrapping is supported: `active_start = "22:00"`, `active_end = "06:00"` means heartbeats run from 10 PM to 6 AM.

### HEARTBEAT_OK Suppression

The suppression mechanism prevents notification fatigue. When everything is fine, you do not want a message every 30 minutes telling you so. The `HEARTBEAT_OK` keyword acts as a sentinel: the provider responds with it when there are no issues, and the gateway silently logs the result instead of forwarding it.

## Error Recovery & Resilience

The gateway is designed to be resilient:

### Non-Fatal Errors
- Database temporarily unavailable → Store fails, but response still sent.
- Audit logging fails → Logged and ignored, processing continues.
- Channel send fails → Logged and ignored, pipeline completes.
- Provider returns an error → Error message sent, audit logged, pipeline stops.

### Fatal Errors
- Channel startup fails → Gateway initialization fails, Omega exits.
- Auth denied → Message dropped, pipeline stops.

### Graceful Shutdown
When Omega receives Ctrl+C:
1. Main event loop breaks.
2. Background tasks are aborted (summarizer, scheduler loop, heartbeat loop).
3. All active conversations are summarized (preserving memory).
4. All channels are stopped cleanly.
5. Omega exits.

This ensures no in-flight conversations are lost.

## Concurrency Model

The gateway uses a **single-threaded, async architecture**:

- **One main thread** — Processes messages sequentially on the main event loop.
- **Multiple background tasks** — Channel listeners, typing repeaters, summarizer run in separate tokio tasks.
- **No locks** — All access is through `Arc` shared references. No Mutex or RwLock needed.

**Why this design?**

- **Simplicity** — No race conditions to reason about.
- **Efficiency** — Message processing is I/O-bound (network, database), not CPU-bound. Concurrency is achieved through async/await, not threads.
- **Scalability** — Can handle many concurrent channels and users without thread overhead.

## Configuration

The gateway accepts several config sources:

### AuthConfig
```toml
[auth]
enabled = true
deny_message = "Sorry, you're not authorized to use Omega."
```

Controls whether authentication is enforced globally.

### ChannelConfig
```toml
[telegram]
token = "YOUR_BOT_TOKEN"
allowed_users = [123456789, 987654321]  # Empty = allow all
```

Controls per-channel settings. For Telegram, the allowed_users list is a whitelist. An empty list allows anyone (useful for testing).

### Filesystem Protection

Filesystem protection is always-on via the `omega_sandbox` crate. There is no `[sandbox]` config section — protection is automatic. The `omega_sandbox::protected_command()` function wraps subprocess execution with OS-level blocklist enforcement (Seatbelt on macOS, Landlock on Linux), and `omega_sandbox::is_write_blocked()` provides tool-level write checking.

## Observability

### Logging
- **INFO** — Gateway startup, messages received, responses sent, summaries completed.
- **WARN** — Auth denials, input sanitization warnings, errors during background tasks.
- **ERROR** — Provider failures, database errors, channel failures.

### Audit Trail
Every interaction is logged to SQLite with full context. Query the audit table to see:
- Who said what and when.
- Which provider answered.
- How long it took.
- Whether there were any errors.

### Example Audit Query
```sql
SELECT channel, sender_id, input_text, output_text, model, processing_ms, status
FROM audit_log
WHERE sender_id = '123456789'
ORDER BY created_at DESC
LIMIT 10;
```

## Performance Characteristics

### Latency
- **Auth check** — <1ms (in-memory comparison).
- **Sanitization** — <1ms (regex scan).
- **Context building** — 10-50ms (database query, history fetch).
- **Provider call** — 2,000-30,000ms (API request).
- **Memory storage** — 10-100ms (database insert).
- **Audit logging** — <1ms (queued insert).
- **Response send** — 100-1000ms (network, channel API).

**Total:** Dominated by provider call (2-30 seconds).

### Throughput
- The main loop processes one message at a time (sequential).
- While one message is being processed, other incoming messages wait in the MPSC queue (capacity 256).
- If the queue fills (256 messages waiting), new messages are blocked until space opens.

**Recommended:** Keep the queue from filling by ensuring provider calls complete in <30 seconds.

### Memory
- Gateway struct stores references (Arc) to channels, provider, memory.
- No per-message allocations that aren't freed.
- MPSC queue holds up to 256 IncomingMessage objects in memory.

## Security Posture

1. **Auth Enforcement** — Messages from unauthorized users are rejected immediately.
2. **Input Sanitization** — Injection patterns are neutralized before provider call.
3. **Audit Logging** — All interactions are logged for intrusion detection.
4. **Error Suppression** — Detailed errors are logged internally but generic messages are sent to users (no info leaks).
5. **Graceful Degradation** — If components fail, the gateway degrades gracefully (e.g., storage failure doesn't block user response).

## Design Rationale

### Why MPSC Channel?
All incoming messages funnel through a single MPSC queue. This ensures:
- Messages are processed in order (no race conditions).
- The main loop can wait on a single receiver (tokio::select!).
- Backpressure is built-in (queue fills if processing is slow).

### Why Arc for Shared References?
Provider and channels are wrapped in Arc to:
- Allow cloning without deep copying (cheap clones for spawned tasks).
- Enable thread-safe access without locking (Arc is read-only).
- Avoid lifetime issues in async code (Arc lives as long as all clones exist).

### Why Background Summarization?
Summarization runs in a separate task to:
- Not block the main event loop.
- Preserve memory across conversation boundaries.
- Extract user facts for personalization.

### Why Graceful Shutdown?
On Ctrl+C, Omega summarizes all active conversations to:
- Avoid losing context from in-flight exchanges.
- Cleanly close all database connections.
- Stop all background tasks.

## Next Steps & Future Enhancements

### Phase 4 (Planned)
- Alternative providers — Direct integration with OpenAI, Anthropic APIs.
- Skills system — Plugins for custom functions (weather, calendar, etc.).
- Sandbox environment — Safe execution of user code.
- WhatsApp support — Full WhatsApp channel implementation.

### Possible Improvements
- Adaptive summarization — Summarize based on content, not just time.
- Conversation branching — Support multiple concurrent threads from the same user.
- Streaming responses — Send response text incrementally instead of waiting for completion.
- Retry logic — Exponential backoff for transient failures.
