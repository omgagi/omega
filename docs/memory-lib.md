# Developer Guide: omega-memory

## Path
`/Users/isudoajl/ownCloud/Projects/omega/crates/omega-memory/src/lib.rs`

## What is omega-memory?

`omega-memory` is the persistence layer for the Omega agent. It stores conversation history, user facts, and a full audit trail of every interaction -- all backed by SQLite. Every message that flows through the gateway is recorded here, and every AI provider call is enriched with context retrieved from here.

Think of it as the agent's long-term memory: it remembers what was said, who said it, what it learned about each user, and a summary of every past conversation.

## What does it provide?

The crate is organized into two public modules and re-exports their primary types at the crate root:

| Module | What you get |
|--------|-------------|
| `store` | Conversation memory -- storing messages, building AI context, managing conversation lifecycles, tracking user facts |
| `audit` | Audit log -- recording every interaction with status, timing, and provider details |

The two primary types are re-exported for convenience:

```rust
// Preferred imports (via re-export):
use omega_memory::{Store, AuditLogger};

// Also valid (via module path):
use omega_memory::store::Store;
use omega_memory::audit::{AuditLogger, AuditEntry, AuditStatus};
```

---

## The Store

The `Store` is the core of the memory system. It manages an SQLite database that holds conversations, messages, and user facts.

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
4. Run all pending migrations automatically.

The store is `Clone` -- it wraps a connection pool internally, so cloning is cheap and shares the same pool.

### Building context for the AI provider

The most important method. Given an incoming message, it assembles a full `Context` with history, facts, and a dynamic system prompt:

```rust
use omega_core::message::IncomingMessage;

let context = store.build_context(&incoming_message, &prompts.system).await?;
// context.system_prompt  -- enriched with user facts and conversation summaries
// context.history        -- recent messages from the current conversation
// context.current_message -- the user's new message
```

Under the hood, `build_context` does the following:
1. Finds (or creates) the active conversation for this channel + sender.
2. Loads the most recent messages from that conversation (up to `max_context_messages`).
3. Fetches all known facts about the sender.
4. Fetches the 3 most recent closed conversation summaries.
5. Builds a dynamic system prompt that includes facts, summaries, and language detection.

### Storing exchanges

After the AI provider responds, store both sides of the exchange:

```rust
store.store_exchange(&incoming_message, &outgoing_response).await?;
```

This inserts two rows into the `messages` table -- one for the user's input (role `"user"`) and one for the assistant's response (role `"assistant"`). The assistant message also stores serialized metadata (provider, model, timing) in a `metadata_json` column.

### Conversation lifecycle

Conversations have automatic boundary detection. A conversation is considered "idle" after 2 hours of inactivity. The gateway uses these methods during its periodic maintenance cycle:

```rust
// Find conversations that have been idle too long:
let idle = store.find_idle_conversations().await?;
// Returns Vec<(conversation_id, channel, sender_id)>

// Close a conversation with a summary:
store.close_conversation(&conv_id, "Discussed Rust error handling").await?;

// Find all active conversations (for graceful shutdown):
let active = store.find_all_active_conversations().await?;

// Close the current conversation immediately (for /forget command):
let was_closed = store.close_current_conversation("telegram", "12345").await?;
```

When a user sends a new message after their conversation has timed out, `build_context` automatically creates a fresh conversation.

### Facts

Facts are key-value pairs scoped to a sender. They persist across conversations and are used to enrich the system prompt:

```rust
// Store a fact:
store.store_fact("user123", "name", "Alice").await?;
store.store_fact("user123", "language", "Spanish").await?;

// Retrieve all facts:
let facts = store.get_facts("user123").await?;
// Returns Vec<(key, value)> ordered by key

// Delete a specific fact:
let deleted = store.delete_facts("user123", Some("language")).await?;

// Delete all facts for a user:
let deleted = store.delete_facts("user123", None).await?;
```

Facts use an upsert pattern: storing a fact with an existing `(sender_id, key)` pair updates the value rather than creating a duplicate.

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

The memory system manages six tables (plus a migration tracker):

| Table | Purpose |
|-------|---------|
| `conversations` | Conversation sessions with status, summary, and activity tracking |
| `messages` | Individual messages within conversations (user and assistant) |
| `facts` | Key-value facts about users, scoped by `sender_id` |
| `outcomes` | Raw interaction outcomes -- short-term working memory (24-48h), scored +1/0/-1 per domain |
| `lessons` | Distilled behavioral rules -- permanent long-term memory, upserted by (sender_id, domain) |
| `audit_log` | Complete record of every interaction |
| `_migrations` | Internal migration tracking (do not modify) |

### Migrations

Migrations run automatically on `Store::new()`. The system is idempotent -- it tracks which migrations have been applied and skips those already recorded. It also handles bootstrapping from databases created before migration tracking was added.

Migrations exist:
1. **001_init** -- Base schema: conversations, messages, facts tables
2. **002_audit_log** -- Audit log table
3. **003_memory_enhancement** -- Conversation boundaries (status, summary, last_activity), facts scoped by sender_id
4. **004_fts5_recall** -- FTS5 full-text search index for cross-conversation recall
5. **005_scheduled_tasks** -- Task queue table with indexes
6. **006_limitations** -- Internal limitations tracking
7. **007_task_type** -- Task type column for action scheduler
8. **008_user_aliases** -- Cross-channel user aliases table
9. **009_task_retry** -- Retry columns for action failure handling
10. **010_outcomes** -- Reward-based learning: outcomes (working memory) and lessons (long-term memory) tables

New migrations can be added by:
1. Creating a new SQL file in `crates/omega-memory/migrations/` (e.g. `004_your_feature.sql`).
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
store.build_context(&incoming, &prompts.system)  <-- Loads history, facts, summaries
    |                                Builds enriched system prompt
    v
provider.complete(&context)     <-- AI generates response
    |
    v
store.store_exchange(&incoming, &response)  <-- Persists both sides
    |
    v
logger.log(&audit_entry)       <-- Records the interaction
    |
    v
channel.send(response)         <-- Delivers response to user
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

Add a public async method to the `Store` impl block in `store.rs`:

```rust
/// Get the total number of audit log entries.
pub async fn audit_count(&self) -> Result<i64, OmegaError> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;
    Ok(count)
}
```

Follow the existing pattern: use `sqlx::query_as` for typed results, map errors to `OmegaError::Memory`, and keep the method async.

### Adding a new table

1. Create a migration file (e.g. `migrations/004_schedules.sql`):
   ```sql
   CREATE TABLE IF NOT EXISTS schedules (
       id TEXT PRIMARY KEY,
       sender_id TEXT NOT NULL,
       cron_expr TEXT NOT NULL,
       action TEXT NOT NULL,
       created_at TEXT NOT NULL DEFAULT (datetime('now'))
   );
   ```

2. Register it in `run_migrations()`:
   ```rust
   let migrations: &[(&str, &str)] = &[
       ("001_init", include_str!("../migrations/001_init.sql")),
       ("002_audit_log", include_str!("../migrations/002_audit_log.sql")),
       ("003_memory_enhancement", include_str!("../migrations/003_memory_enhancement.sql")),
       ("004_schedules", include_str!("../migrations/004_schedules.sql")),
   ];
   ```

3. Add query methods to `Store` as needed.

### Adding a new module

If you need a new subsystem (e.g. a scheduler store), add it as a sibling module:

1. Create `crates/omega-memory/src/scheduler.rs`.
2. Declare it in `lib.rs`:
   ```rust
   pub mod scheduler;
   pub use scheduler::Scheduler;
   ```
3. Have it accept a `SqlitePool` (from `Store::pool()`) just like `AuditLogger` does.

---

## Quick reference

| You want to... | Use this |
|----------------|----------|
| Initialize memory | `Store::new(&config.memory).await?` |
| Build context for a provider | `store.build_context(&incoming, &prompts.system).await?` |
| Store a message exchange | `store.store_exchange(&incoming, &response).await?` |
| Store a user fact | `store.store_fact("sender", "key", "value").await?` |
| Get user facts | `store.get_facts("sender").await?` |
| Delete user facts | `store.delete_facts("sender", Some("key")).await?` |
| Close current conversation | `store.close_current_conversation("channel", "sender").await?` |
| Get conversation history | `store.get_history("channel", "sender", 10).await?` |
| Get memory statistics | `store.get_memory_stats("sender").await?` |
| Get database size | `store.db_size().await?` |
| Create audit logger | `AuditLogger::new(store.pool().clone())` |
| Log an interaction | `logger.log(&audit_entry).await?` |
| Store a reward outcome | `store.store_outcome("sender", "domain", 1, "lesson", "conversation").await?` |
| Get recent outcomes (per user) | `store.get_recent_outcomes("sender", 15).await?` |
| Get recent outcomes (all users) | `store.get_all_recent_outcomes(24, 20).await?` |
| Store a distilled lesson | `store.store_lesson("sender", "domain", "rule").await?` |
| Get lessons (per user) | `store.get_lessons("sender").await?` |
| Get lessons (all users) | `store.get_all_lessons().await?` |
| Get connection pool | `store.pool()` |
