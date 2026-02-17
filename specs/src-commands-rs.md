# Specification: src/commands.rs

## Overview

**File Path:** `src/commands.rs`

**Purpose:** Implements built-in bot commands for Omega. Commands are instant-response operations that bypass the AI provider entirely. They directly query and manipulate the memory store, providing users with immediate access to system status, conversation history, facts, and memory management.

**Key Characteristic:** All commands execute asynchronously with no external provider invocation.

---

## Command Enum

### Variants

```rust
pub enum Command {
    Status,
    Memory,
    History,
    Facts,
    Forget,
    Tasks,
    Cancel,
    Language,
    Projects,
    Project,
    Help,
}
```

| Command | Purpose |
|---------|---------|
| `Status` | System uptime, active provider, and database size |
| `Memory` | User-specific stats: conversations, messages, facts count |
| `History` | Last 5 conversation summaries with timestamps |
| `Facts` | List of known facts about the user |
| `Forget` | Close and clear the current active conversation |
| `Tasks` | List pending scheduled tasks for the user |
| `Cancel` | Cancel a scheduled task by ID prefix |
| `Language` | Show or set the user's preferred response language |
| `Projects` | List available projects, marking the active one |
| `Project` | Show, activate, or deactivate a project |
| `Help` | Display all available commands |

---

## Command Parsing

### Function: `Command::parse(text: &str) -> Option<Self>`

**Location:** Lines 19–30

**Behavior:**
- Extracts the first whitespace-delimited token from message text
- Matches against known command prefixes (all start with `/`)
- Returns `None` for unknown `/` prefixes, allowing them to pass through to the provider as regular messages
- Case-sensitive matching

**Command Prefixes Recognized:**
- `/status` → `Command::Status`
- `/memory` → `Command::Memory`
- `/history` → `Command::History`
- `/facts` → `Command::Facts`
- `/forget` → `Command::Forget`
- `/tasks` → `Command::Tasks`
- `/cancel` → `Command::Cancel`
- `/language` or `/lang` → `Command::Language`
- `/projects` → `Command::Projects`
- `/project` → `Command::Project`
- `/help` → `Command::Help`

**Example Behavior:**
```
"/status" → Some(Status)
"/help foobar" → Some(Help)  // whitespace-delimited, so "foobar" ignored
"/unknown" → None  // unknown commands pass through
"hello" → None  // non-command text returns None
```

---

## CommandContext Struct

### `CommandContext<'a>`

Groups all execution context for command handling into a single struct, avoiding excessive positional arguments.

```rust
pub struct CommandContext<'a> {
    pub store: &'a Store,
    pub channel: &'a str,
    pub sender_id: &'a str,
    pub text: &'a str,
    pub uptime: &'a Instant,
    pub provider_name: &'a str,
    pub skills: &'a [omega_skills::Skill],
    pub projects: &'a [omega_skills::Project],
    pub sandbox_mode: &'a str,
}
```

| Field | Purpose |
|-------|---------|
| `store` | Reference to the memory store (SQLite-backed) |
| `channel` | Messaging channel identifier (e.g., "telegram", "whatsapp") |
| `sender_id` | User identifier within the channel |
| `text` | Full original message text (used by `/cancel`, `/language`, `/project` to extract arguments) |
| `uptime` | Process start time (for elapsed duration calculation) |
| `provider_name` | Active AI provider name (e.g., "Claude Code CLI") |
| `skills` | Slice of loaded skill definitions (for `/skills` command) |
| `projects` | Slice of loaded project definitions (for `/projects` and `/project` commands) |
| `sandbox_mode` | Display name of the active sandbox mode (e.g., `"sandbox"`, `"rx"`, `"rwx"`). Shown in `/status` output. |

---

## Command Handler

### Function: `handle(cmd, ctx) -> String`

**Signature:**
```rust
pub async fn handle(cmd: Command, ctx: &CommandContext<'_>) -> String
```

**Parameters:**
- `cmd`: The parsed command enum variant
- `ctx`: Grouped execution context (see `CommandContext` above)

**Return:** Formatted response text to send back to the user

**Dispatch:** Routes each command variant to its handler function, passing only the fields each handler needs from `ctx`

---

## Individual Command Handlers

### /status — `handle_status(store, uptime, provider_name, sandbox_mode)`

**Location:** Lines 52–70

**Behavior:**
- Calculates elapsed time since `uptime` in hours, minutes, seconds
- Queries `store.db_size()` for database file size
- Formats size using `format_bytes()`
- Includes the active sandbox mode in the output
- Returns multi-line response with four fields

**Response Format:**
```
*OMEGA Ω* Status
Uptime: 1h 23m 45s
Provider: Claude Code CLI
Sandbox: sandbox
Database: 2.3 MB
```

**Error Handling:** If `db_size()` fails, displays "unknown" instead of panicking

---

### /memory — `handle_memory(store, sender_id)`

**Location:** Lines 72–84

**Behavior:**
- Calls `store.get_memory_stats(sender_id)` (async)
- Retrieves tuple: `(convos: i64, msgs: i64, facts: i64)`
- Formats response with three counts

**Response Format (Success):**
```
Your Memory
Conversations: 5
Messages: 47
Facts: 3
```

**Response Format (Error):**
```
Error: [error description]
```

---

### /history — `handle_history(store, channel, sender_id)`

**Location:** Lines 86–98

**Behavior:**
- Calls `store.get_history(channel, sender_id, 5)` to fetch last 5 conversations
- Returns `Vec<(summary: String, timestamp: String)>`
- Iterates through entries, formatting each with timestamp and summary
- Handles empty history gracefully

**Response Format (With History):**
```
Recent Conversations

[2025-02-16 14:30:15]
Discussed project architecture and design patterns

[2025-02-16 13:15:22]
Reviewed Rust async/await best practices
```

**Response Format (Empty):**
```
No conversation history yet.
```

---

### /facts — `handle_facts(store, sender_id)`

**Location:** Lines 100–112

**Behavior:**
- Calls `store.get_facts(sender_id)` (async)
- Returns `Vec<(key: String, value: String)>` of fact key-value pairs
- Iterates and formats each fact as a bulleted list
- Handles empty facts gracefully

**Response Format (With Facts):**
```
Known Facts

- favorite_language: Rust
- location: San Francisco
- timezone: PST
```

**Response Format (Empty):**
```
No facts stored yet.
```

---

### /forget — `handle_forget(store, channel, sender_id)`

**Location:** Lines 114–120

**Behavior:**
- Calls `store.close_current_conversation(channel, sender_id)` (async)
- Closes and clears the active conversation for the user in the specified channel
- Returns boolean: `true` if a conversation was closed, `false` if none was active

**Response Format (Success - Conversation Cleared):**
```
Conversation cleared. Starting fresh.
```

**Response Format (Success - No Active Conversation):**
```
No active conversation to clear.
```

**Response Format (Error):**
```
Error: [error description]
```

---

### /tasks — `handle_tasks(store, sender_id)`

**Location:** Lines 129–145

**Behavior:**
- Calls `store.get_tasks_for_sender(sender_id)` (async)
- Returns `Vec<(String, String, String, Option<String>)>` as `(id, description, due_at, repeat)`
- Displays first 8 characters of task ID as short ID for user reference
- Shows repeat label (`once` if `None`, otherwise the repeat value)
- Handles empty task list gracefully

**Response Format (With Tasks):**
```
Scheduled Tasks

[a1b2c3d4] Call John
  Due: 2026-02-17T15:00:00 (once)

[e5f6g7h8] Daily standup
  Due: 2026-02-18T09:00:00 (daily)
```

**Response Format (Empty):**
```
No pending tasks.
```

**Response Format (Error):**
```
Error: [error description]
```

---

### /cancel — `handle_cancel(store, sender_id, text)`

**Location:** Lines 147–157

**Behavior:**
- Extracts the second whitespace-delimited token from `text` as the task ID prefix
- If no ID prefix is provided, returns usage instructions
- Calls `store.cancel_task(id_prefix, sender_id)` (async)
- Returns `bool`: `true` if a task was cancelled, `false` if no matching task found
- Task must belong to the sender (enforced by store query)

**Response Format (Success):**
```
Task cancelled.
```

**Response Format (No Match):**
```
No matching task found.
```

**Response Format (Missing Argument):**
```
Usage: /cancel <task-id>
```

**Response Format (Error):**
```
Error: [error description]
```

---

### /language — `handle_language(store, sender_id, text)`

**Behavior:**
- Collects all whitespace-delimited tokens after `/language` as the language argument.
- **No argument:** Looks up the `preferred_language` fact for the user via `store.get_facts()`. Displays the current language or "not set" if absent. Shows usage hint.
- **With argument:** Stores the language as a `preferred_language` fact via `store.store_fact()`. Confirms the change.

**Aliases:** `/language`, `/lang`

**Response Format (Show Current):**
```
Language: Spanish
Usage: /language <language>
```

**Response Format (Set New):**
```
Language set to: French
```

**Response Format (Error):**
```
Error: [error description]
```

---

### /projects — `handle_projects(store, sender_id, projects)`

**Behavior:**
- If no projects exist, returns instructions to create folders in `~/.omega/projects/`.
- Calls `store.get_fact(sender_id, "active_project")` to get the currently active project.
- Lists all projects with `(active)` marker next to the current one.
- Appends usage instructions.

**Response Format (With Projects):**
```
Projects

- real-estate (active)
- nutrition
- stocks

Use /project <name> to activate, /project off to deactivate.
```

**Response Format (No Projects):**
```
No projects found. Create folders in ~/.omega/projects/ with INSTRUCTIONS.md
```

---

### /project — `handle_project(store, channel, sender_id, text, projects)`

**Behavior:**
- **No argument** (`/project`): Shows the current active project or instructions.
- **`/project off`**: Deactivates the current project by deleting the `active_project` fact. Closes current conversation for clean context.
- **`/project <name>`**: Activates a project by name. Validates the name exists via `get_project_instructions()`. Stores the `active_project` fact. Closes current conversation for clean context.

**Response Format (Show Current):**
```
Active project: real-estate
Use /project off to deactivate.
```

**Response Format (Activate):**
```
Project 'real-estate' activated. Conversation cleared.
```

**Response Format (Deactivate):**
```
Project deactivated. Conversation cleared.
```

**Response Format (Not Found):**
```
Project 'xyz' not found. Use /projects to see available projects.
```

---

### /help — `handle_help()`

**Location:** Lines 159–171

**Behavior:**
- No async operations or external calls
- Returns hardcoded help text with all nine commands and brief descriptions
- Single-threaded, pure function

**Response Format:**
```
*OMEGA Ω* Commands

/status   — Uptime, provider, database info
/memory   — Your conversation and facts stats
/history  — Last 5 conversation summaries
/facts    — List known facts about you
/forget   — Clear current conversation
/tasks    — List your scheduled tasks
/cancel   — Cancel a task by ID
/language — Show or set your language
/projects — List available projects
/project  — Show, activate, or deactivate a project
/help     — This message
```

---

## Helper Functions

### `format_bytes(bytes: u64) -> String`

**Location:** Lines 134–143

**Purpose:** Convert byte counts to human-readable format

**Logic:**
- `< 1024 B` → Display as bytes: `"512 B"`
- `< 1 MB` → Display as KB (1 decimal): `"2.5 KB"`
- `≥ 1 MB` → Display as MB (1 decimal): `"3.2 MB"`

**Examples:**
- `512` → `"512 B"`
- `2560` → `"2.5 KB"`
- `5242880` → `"5.0 MB"`

---

## Memory Store Integration

All command handlers interact with the `omega_memory::Store` trait/type:

| Handler | Method | Return Type | Purpose |
|---------|--------|-------------|---------|
| `handle_status()` | `store.db_size()` | `Result<u64>` | Get database file size |
| `handle_memory()` | `store.get_memory_stats(sender_id)` | `Result<(i64, i64, i64)>` | Count conversations, messages, facts |
| `handle_history()` | `store.get_history(channel, sender_id, 5)` | `Result<Vec<(String, String)>>` | Fetch last 5 conversation summaries |
| `handle_facts()` | `store.get_facts(sender_id)` | `Result<Vec<(String, String)>>` | Fetch all facts for user |
| `handle_forget()` | `store.close_current_conversation(channel, sender_id)` | `Result<bool>` | Close active conversation |
| `handle_tasks()` | `store.get_tasks_for_sender(sender_id)` | `Result<Vec<(String, String, String, Option<String>)>>` | Fetch pending tasks for user |
| `handle_cancel()` | `store.cancel_task(id_prefix, sender_id)` | `Result<bool>` | Cancel a task by ID prefix |
| `handle_language()` | `store.get_facts(sender_id)` | `Result<Vec<(String, String)>>` | Look up current preferred_language fact |
| `handle_language()` | `store.store_fact(sender_id, key, value)` | `Result<()>` | Set preferred_language fact |
| `handle_projects()` | `store.get_fact(sender_id, "active_project")` | `Result<Option<String>>` | Get current active project |
| `handle_project()` | `store.get_fact(sender_id, "active_project")` | `Result<Option<String>>` | Get current active project |
| `handle_project()` | `store.store_fact(sender_id, "active_project", name)` | `Result<()>` | Set active project |
| `handle_project()` | `store.delete_fact(sender_id, "active_project")` | `Result<bool>` | Deactivate project |
| `handle_project()` | `store.close_current_conversation(channel, sender_id)` | `Result<bool>` | Clear conversation on project switch |

All store operations are async and return `Result` types with proper error handling.

---

## Design Patterns

### Pattern 1: Async Without External I/O

All handlers are `async` even though most only interact with local SQLite. This allows for:
- Consistent async interface with rest of Omega
- Future extensibility (e.g., remote status checks)
- Non-blocking database queries

### Pattern 2: Error Handling

- `Result` types from store methods are unwrapped with `match` expressions
- No `.unwrap()` or `.expect()` calls
- Errors formatted as-is in responses: `"Error: {e}"`
- Graceful degradation (e.g., `"unknown"` for db_size failures)

### Pattern 3: Separation of Concerns

- Parsing (`Command::parse()`) → Dispatch (`handle()`) → Execution (individual handlers)
- Each handler has single responsibility
- No business logic in enum definition

---

## Integration Points

**Called From:** `src/gateway.rs` (event loop)

**Flow:**
1. Message arrives (Telegram/WhatsApp)
2. Text is checked via `Command::parse(text)`
3. If `Some(cmd)`, invoke `Command::handle()` with parsed command
4. Send response immediately without calling AI provider
5. If `None`, pass message to provider for reasoning

---

## Notes

- Command names are hardcoded; no dynamic command registration system
- All commands are synchronous from user perspective (no long-running operations)
- Commands are scoped to user + channel (e.g., `/forget` clears only the user's conversation in that channel)
- `/language` has an alias `/lang`; all other commands have no aliases
- Fact keys and values are opaque strings managed by the memory system
- `/cancel` uses ID prefix matching (first 8+ characters of UUID) for user convenience
- `/tasks` only shows tasks with status `'pending'`; delivered and cancelled tasks are hidden

