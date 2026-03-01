# Developer Guide: omega-memory

## Path
`/Users/isudoajl/ownCloud/Projects/omega/backend/crates/omega-memory/src/lib.rs`

## What is omega-memory?

`omega-memory` is the persistence layer for the Omega agent. It stores conversation history, user facts, scheduled tasks, reward-based learning data, cross-channel aliases, CLI sessions, and a full audit trail of every interaction -- all backed by SQLite. Every message that flows through the gateway is recorded here, and every AI provider call is enriched with context retrieved from here.

Think of it as the agent's long-term memory: it remembers what was said, who said it, what it learned about each user, what tasks are scheduled, what behavioral lessons have been distilled, and a summary of every past conversation.

## What does it provide?

The crate is organized into two public modules and re-exports their primary types at the crate root:

| Module | What you get |
|--------|-------------|
| `store` | Conversation memory -- storing messages, building AI context, managing conversation lifecycles, tracking user facts, aliases, limitations, scheduled tasks, reward-based learning (outcomes + lessons), and CLI sessions. Split into 8 focused submodules. |
| `audit` | Audit log -- recording every interaction with status, timing, and provider details |

The primary types are re-exported for convenience:

```rust
// Preferred imports (via re-export):
use omega_memory::{Store, AuditLogger, DueTask, detect_language};

// Also valid (via module path):
use omega_memory::store::Store;
use omega_memory::store::format_user_profile;
use omega_memory::audit::{AuditLogger, AuditEntry, AuditStatus};
```

---

## The Store

The `Store` is the core of the memory system. It manages an SQLite database across 11 tables and 1 virtual table, implemented as a directory module with 8 submodules.

### Creating a store

```rust
use omega_memory::Store;
use omega_core::config::MemoryConfig;

let config = MemoryConfig {
    backend: "sqlite".to_string(),
    db_path: "~/.omega/data/memory.db".to_string(),
    max_context_messages: 50,
};

let store = Store::new(&config).await?;
```

On first call, `new()` will:
1. Expand `~` to the user's home directory.
2. Create parent directories if they do not exist.
3. Open (or create) the SQLite database with WAL journal mode.
4. Run all 13 pending migrations automatically.

The store is `Clone` -- it wraps a connection pool internally, so cloning is cheap and shares the same pool.

### Building context for the AI provider

The most important method. Given an incoming message and context needs, it assembles a full `Context` with history, facts, and a dynamic system prompt:

```rust
use omega_core::message::IncomingMessage;
use omega_core::context::ContextNeeds;

let needs = ContextNeeds {
    summaries: true,
    recall: true,
    pending_tasks: true,
    profile: true,
    outcomes: true,
};

let context = store.build_context(
    &incoming_message,
    &prompts.system,
    &needs,
    active_project.as_deref(),
).await?;
// context.system_prompt  -- enriched with facts, summaries, recall, tasks, outcomes, lessons
// context.history        -- recent messages from the current conversation
// context.current_message -- the user's new message
```

Under the hood, `build_context` does the following:
1. Finds (or creates) the active conversation for this channel + sender + project.
2. Runs 7 parallel database queries via `tokio::join!`:
   - Recent messages from the current conversation (up to `max_context_messages`)
   - All known facts about the sender
   - Recent closed conversation summaries (if `needs.summaries`)
   - FTS5 semantic recall from past conversations (if `needs.recall`)
   - Pending scheduled tasks (if `needs.pending_tasks`)
   - Recent outcomes (if `needs.outcomes`)
   - Learned behavioral lessons
3. Resolves the user's language (stored preference > auto-detect > English).
4. Computes progressive onboarding stage and injects hints on transitions.
5. Builds a dynamic system prompt that includes facts, summaries, recall, tasks, outcomes, lessons, language directive, and onboarding hints.

### Storing exchanges

After the AI provider responds, store both sides of the exchange:

```rust
store.store_exchange(&incoming_message, &outgoing_response, "project-name").await?;
```

This inserts two rows into the `messages` table -- one for the user's input (role `"user"`) and one for the assistant's response (role `"assistant"`). The assistant message also stores serialized metadata (provider, model, timing) in a `metadata_json` column. The conversation is project-scoped.

### Conversation lifecycle

Conversations have automatic boundary detection. A conversation is considered "idle" after 2 hours of inactivity. The gateway uses these methods during its periodic maintenance cycle:

```rust
// Find conversations that have been idle too long:
let idle = store.find_idle_conversations().await?;
// Returns Vec<(conversation_id, channel, sender_id, project)>

// Close a conversation with a summary:
store.close_conversation(&conv_id, "Discussed Rust error handling").await?;

// Find all active conversations (for graceful shutdown):
let active = store.find_all_active_conversations().await?;

// Close the current conversation immediately (for /forget command):
let was_closed = store.close_current_conversation("telegram", "12345", "project").await?;

// Get recent summaries across all users (for heartbeat):
let summaries = store.get_all_recent_summaries(5).await?;
```

When a user sends a new message after their conversation has timed out, `build_context` automatically creates a fresh conversation.

### Facts

Facts are key-value pairs scoped to a sender. They persist across conversations and are used to enrich the system prompt:

```rust
// Store a fact:
store.store_fact("user123", "name", "Alice").await?;
store.store_fact("user123", "timezone", "UTC-3").await?;

// Retrieve a single fact:
let tz = store.get_fact("user123", "timezone").await?;

// Retrieve all facts:
let facts = store.get_facts("user123").await?;
// Returns Vec<(key, value)> ordered by key

// Delete a specific fact:
let deleted = store.delete_fact("user123", "timezone").await?;

// Delete all facts for a user:
let deleted = store.delete_facts("user123", None).await?;

// Check if a user is new (no 'welcomed' fact):
let is_new = store.is_new_user("user123").await?;

// Get all facts across all users (for heartbeat):
let all_facts = store.get_all_facts().await?;

// Get all users with a specific fact key:
let active_project_users = store.get_all_facts_by_key("active_project").await?;
```

Facts use an upsert pattern: storing a fact with an existing `(sender_id, key)` pair updates the value rather than creating a duplicate.

### Cross-channel aliases

Link users across channels so they share the same memory:

```rust
// Resolve a sender_id to its canonical form:
let canonical = store.resolve_sender_id("5511999887766").await?;

// Create an alias (WhatsApp phone -> Telegram ID):
store.create_alias("5511999887766", "842277204").await?;

// Find an existing user to link to:
let canonical = store.find_canonical_user("5511999887766").await?;
```

### Limitations

Track self-detected capability gaps:

```rust
// Store a limitation (deduplicates by title):
let is_new = store.store_limitation(
    "Cannot access Google Calendar",
    "No direct API integration",
    "Add Google Calendar MCP server"
).await?;

// Get all open limitations:
let open = store.get_open_limitations().await?;
// Returns Vec<(title, description, proposed_plan)>
```

### Scheduled tasks

Create, query, and manage scheduled tasks:

```rust
// Create a task with deduplication:
let id = store.create_task(
    "telegram", "user123", "chat_id", "Call John",
    "2026-03-01T15:00:00", Some("daily"), "reminder", ""
).await?;

// Get due tasks:
let due = store.get_due_tasks().await?;

// Complete a task:
store.complete_task(&id, Some("daily")).await?;

// Fail an action task (with retry):
let will_retry = store.fail_task(&id, "provider timeout", 3).await?;

// Get pending tasks for a user:
let tasks = store.get_tasks_for_sender("user123").await?;

// Cancel a task by ID prefix:
store.cancel_task("abc123", "user123").await?;

// Update task fields:
store.update_task("abc123", "user123", Some("New desc"), None, None).await?;

// Defer a task:
store.defer_task(&id, "2026-03-02T15:00:00").await?;
```

### Reward-based learning

Store outcomes (working memory) and distilled lessons (long-term memory):

```rust
// Store a raw outcome from a REWARD marker:
store.store_outcome("user123", "code_review", 1, "Caught a bug early", "conversation", "omega").await?;

// Get recent outcomes for a user (project-scoped):
let outcomes = store.get_recent_outcomes("user123", 15, Some("omega")).await?;
// Returns Vec<(score, domain, lesson, timestamp)>

// Get recent outcomes across all users (for heartbeat):
let all = store.get_all_recent_outcomes(24, 20, None).await?;

// Store a distilled lesson (content-based dedup, capped at 10 per domain):
store.store_lesson("user123", "code_review", "Always check error handling", "omega").await?;

// Get lessons for a user:
let lessons = store.get_lessons("user123", Some("omega")).await?;
// Returns Vec<(domain, rule, project)>

// Get all lessons (for heartbeat):
let all_lessons = store.get_all_lessons(None).await?;
```

### CLI sessions

Persist Claude Code CLI session IDs across restarts:

```rust
// Store/update a session:
store.store_session("telegram", "user123", "omega", "session-abc").await?;

// Look up a session:
let session = store.get_session("telegram", "user123", "omega").await?;

// Clear a specific project session:
store.clear_session("telegram", "user123", "omega").await?;

// Clear all sessions for a user:
store.clear_all_sessions_for_sender("user123").await?;
```

### Conversation history and statistics

For bot commands like `/history` and `/memory`:

```rust
// Get conversation summaries for display:
let history = store.get_history("telegram", "user123", 10).await?;
// Returns Vec<(summary, updated_at)> -- newest first

// Get memory statistics:
let (conversations, messages, facts) = store.get_memory_stats("user123").await?;

// Get database size:
let bytes = store.db_size().await?;
```

### Getting the connection pool

The `AuditLogger` needs a pool reference. Get it from the store:

```rust
let pool = store.pool().clone();
let logger = AuditLogger::new(pool);
```

---

## The Audit Logger

Every interaction through Omega -- successful, failed, or denied -- gets an audit log entry.

### Creating the logger

The audit logger shares the same SQLite pool as the store:

```rust
use omega_memory::AuditLogger;
use omega_memory::audit::{AuditEntry, AuditStatus};

let logger = AuditLogger::new(store.pool().clone());
```

### Logging an interaction

```rust
let entry = AuditEntry {
    channel: "telegram".to_string(),
    sender_id: "12345".to_string(),
    sender_name: Some("Alice".to_string()),
    input_text: "What's the weather?".to_string(),
    output_text: Some("It's sunny and 22C.".to_string()),
    provider_used: Some("claude-code".to_string()),
    model: Some("claude-sonnet-4-20250514".to_string()),
    processing_ms: Some(1500),
    status: AuditStatus::Ok,
    denial_reason: None,
};

logger.log(&entry).await?;
```

### Audit status values

| Status | When to use |
|--------|------------|
| `AuditStatus::Ok` | The interaction completed successfully |
| `AuditStatus::Error` | The provider or system encountered an error |
| `AuditStatus::Denied` | Auth rejected the request (set `denial_reason`) |

Each entry gets a UUID and timestamp automatically.

---

## Database schema

The memory system manages 11 tables, 1 virtual table, and a migration tracker:

| Table | Purpose |
|-------|---------|
| `conversations` | Conversation sessions with status, summary, activity tracking, and project scope |
| `messages` | Individual messages within conversations (user and assistant) |
| `messages_fts` | FTS5 virtual table for full-text search across messages |
| `facts` | Key-value facts about users, scoped by `sender_id` |
| `scheduled_tasks` | Task queue with reminders, actions, repeat schedules, retry logic, and project scope |
| `limitations` | Self-detected capability limitations |
| `user_aliases` | Cross-channel user identity linking (alias -> canonical sender_id) |
| `outcomes` | Raw interaction outcomes -- short-term working memory, scored per domain, project-scoped |
| `lessons` | Distilled behavioral rules -- long-term memory, multiple per domain, project-scoped |
| `project_sessions` | Persistent CLI session IDs scoped by (channel, sender_id, project) |
| `audit_log` | Complete record of every interaction |
| `_migrations` | Internal migration tracking (do not modify) |

### Migrations

Migrations run automatically on `Store::new()`. The system is idempotent -- it tracks which migrations have been applied and skips those already recorded. It also handles bootstrapping from databases created before migration tracking was added.

All 13 migrations:
1. **001_init** -- Base schema: conversations, messages, facts tables
2. **002_audit_log** -- Audit log table
3. **003_memory_enhancement** -- Conversation boundaries (status, summary, last_activity), facts scoped by sender_id
4. **004_fts5_recall** -- FTS5 full-text search index for cross-conversation recall
5. **005_scheduled_tasks** -- Task queue table with indexes
6. **006_limitations** -- Internal limitations tracking table
7. **007_task_type** -- Task type column for action scheduler
8. **008_user_aliases** -- Cross-channel user aliases table
9. **009_task_retry** -- Retry columns for action failure handling
10. **010_outcomes** -- Reward-based learning: outcomes (working memory) and lessons (long-term memory) tables
11. **011_project_learning** -- Project column on outcomes, lessons, and scheduled_tasks; recreates lessons with project in unique constraint
12. **012_project_sessions** -- Project-scoped CLI sessions table; project column on conversations
13. **013_multi_lessons** -- Removes unique constraint from lessons; allows multiple lessons per (sender, domain, project)

New migrations can be added by:
1. Creating a new SQL file in `backend/crates/omega-memory/migrations/` (e.g. `014_your_feature.sql`).
2. Adding the migration to the `migrations` array in `Store::run_migrations()`.

---

## How it fits into the gateway

The memory system sits at the center of the gateway event loop:

```
Incoming message arrives from a channel
    |
    v
Auth check (allowed_users)
    |
    v
store.resolve_sender_id()        <-- Cross-channel alias resolution
    |
    v
store.build_context(              <-- Loads history, facts, summaries,
    &incoming,                        recall, tasks, outcomes, lessons
    &prompts.system,                  Builds enriched system prompt
    &needs,
    active_project,
)
    |
    v
provider.complete(&context)      <-- AI generates response
    |
    v
store.store_exchange(             <-- Persists both sides (project-scoped)
    &incoming,
    &response,
    &project,
)
    |
    v
logger.log(&audit_entry)         <-- Records the interaction
    |
    v
channel.send(response)           <-- Delivers response to user
```

The store and logger are initialized once at startup and shared across the gateway via `Clone`:

```rust
let store = Store::new(&config.memory).await?;
let logger = AuditLogger::new(store.pool().clone());

// Both are Clone -- pass them into the gateway event loop
```

---

## How to extend it

### Adding a new query

Add a public async method to the appropriate submodule in `store/`:

```rust
// In store/conversations.rs, store/facts.rs, etc.
impl Store {
    /// Get the total number of audit log entries.
    pub async fn audit_count(&self) -> Result<i64, OmegaError> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;
        Ok(count)
    }
}
```

Follow the existing pattern: use `sqlx::query_as` for typed results, map errors to `OmegaError::Memory`, and keep the method async. Place the method in the submodule that matches its domain (conversations, facts, tasks, etc.).

### Adding a new table

1. Create a migration file (e.g. `migrations/014_schedules.sql`):
   ```sql
   CREATE TABLE IF NOT EXISTS schedules (
       id TEXT PRIMARY KEY,
       sender_id TEXT NOT NULL,
       cron_expr TEXT NOT NULL,
       action TEXT NOT NULL,
       created_at TEXT NOT NULL DEFAULT (datetime('now'))
   );
   ```

2. Register it in `run_migrations()` in `store/mod.rs`:
   ```rust
   let migrations: &[(&str, &str)] = &[
       // ... existing migrations ...
       ("014_schedules", include_str!("../../migrations/014_schedules.sql")),
   ];
   ```

3. Add query methods to the appropriate submodule (or create a new one).

### Adding a new submodule

If the domain warrants a separate file:

1. Create `backend/crates/omega-memory/src/store/your_module.rs`.
2. Declare it in `store/mod.rs`:
   ```rust
   mod your_module;
   ```
3. Implement methods on `Store` in the new file.

---

## Quick reference

| You want to... | Use this |
|----------------|----------|
| Initialize memory | `Store::new(&config.memory).await?` |
| Build context for a provider | `store.build_context(&incoming, &base_prompt, &needs, project).await?` |
| Store a message exchange | `store.store_exchange(&incoming, &response, "project").await?` |
| Store a user fact | `store.store_fact("sender", "key", "value").await?` |
| Get a single fact | `store.get_fact("sender", "key").await?` |
| Get user facts | `store.get_facts("sender").await?` |
| Delete a single fact | `store.delete_fact("sender", "key").await?` |
| Delete user facts | `store.delete_facts("sender", Some("key")).await?` |
| Check if new user | `store.is_new_user("sender").await?` |
| Resolve cross-channel alias | `store.resolve_sender_id("sender").await?` |
| Create cross-channel alias | `store.create_alias("alias", "canonical").await?` |
| Close current conversation | `store.close_current_conversation("channel", "sender", "project").await?` |
| Get conversation history | `store.get_history("channel", "sender", 10).await?` |
| Get memory statistics | `store.get_memory_stats("sender").await?` |
| Get database size | `store.db_size().await?` |
| Create a scheduled task | `store.create_task(channel, sender, target, desc, due_at, repeat, type, project).await?` |
| Get due tasks | `store.get_due_tasks().await?` |
| Complete a task | `store.complete_task(&id, repeat).await?` |
| Fail a task (with retry) | `store.fail_task(&id, "error", 3).await?` |
| Cancel a task | `store.cancel_task("prefix", "sender").await?` |
| Store a reward outcome | `store.store_outcome("sender", "domain", 1, "lesson", "source", "project").await?` |
| Get recent outcomes (per user) | `store.get_recent_outcomes("sender", 15, Some("project")).await?` |
| Get recent outcomes (all users) | `store.get_all_recent_outcomes(24, 20, None).await?` |
| Store a distilled lesson | `store.store_lesson("sender", "domain", "rule", "project").await?` |
| Get lessons (per user) | `store.get_lessons("sender", Some("project")).await?` |
| Get lessons (all users) | `store.get_all_lessons(None).await?` |
| Store a CLI session | `store.store_session("channel", "sender", "project", "session_id").await?` |
| Get a CLI session | `store.get_session("channel", "sender", "project").await?` |
| Clear a CLI session | `store.clear_session("channel", "sender", "project").await?` |
| Create audit logger | `AuditLogger::new(store.pool().clone())` |
| Log an interaction | `logger.log(&audit_entry).await?` |
| Get connection pool | `store.pool()` |
