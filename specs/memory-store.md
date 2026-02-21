# Specification: omega-memory/src/store.rs

## File Path
`/Users/isudoajl/ownCloud/Projects/omega/crates/omega-memory/src/store.rs`

## Purpose
SQLite-backed persistent memory store for Omega. Manages conversation lifecycle (creation, activity tracking, idle detection, closure), message storage, user fact persistence, context building for AI providers, and memory statistics. This is the central data layer that enables Omega to maintain conversation continuity, user personalization, and long-term memory across sessions.

## Architecture Overview

### Core Responsibility
The store owns all read/write access to the SQLite database for conversation data, messages, and facts. It is consumed by the gateway (for the message pipeline) and by background tasks (for summarization and shutdown). The store does **not** manage the audit log -- that is handled by `AuditLogger` which shares the same database pool.

### Database Location
Default: `~/.omega/memory.db` (configurable via `MemoryConfig.db_path`). The `~` prefix is expanded at runtime via the `shellexpand()` helper.

## Constants

| Name | Type | Value | Description |
|------|------|-------|-------------|
| `CONVERSATION_TIMEOUT_MINUTES` | `i64` | `30` | Minutes of inactivity before a conversation is considered idle and eligible for summarization/closure. |

## Data Structures

### Store

```rust
#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    max_context_messages: usize,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `pool` | `SqlitePool` | SQLite connection pool (max 4 connections, WAL journal mode). |
| `max_context_messages` | `usize` | Maximum number of recent messages to include in context for the provider. Sourced from `MemoryConfig.max_context_messages` (default: 50). |

**Traits derived:** `Clone`.

**Thread safety:** `SqlitePool` is `Send + Sync`, so `Store` can be safely shared across tokio tasks via cloning.

## Database Schema

The store manages six tables and one virtual table created across eight migrations. A tracking table (`_migrations`) tracks migration state.

### Table: `_migrations`

Created directly in `run_migrations()`, not via a migration file.

```sql
CREATE TABLE IF NOT EXISTS _migrations (
    name       TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `name` | `TEXT` | `PRIMARY KEY` | Migration identifier (e.g., `"001_init"`). |
| `applied_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | ISO-8601 timestamp of when the migration was applied. |

### Table: `conversations`

Created by `001_init.sql`, extended by `003_memory_enhancement.sql`.

```sql
CREATE TABLE IF NOT EXISTS conversations (
    id            TEXT PRIMARY KEY,
    channel       TEXT NOT NULL,
    sender_id     TEXT NOT NULL,
    started_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    -- Added by 003_memory_enhancement:
    summary       TEXT,
    last_activity TEXT NOT NULL DEFAULT (datetime('now')),
    status        TEXT NOT NULL DEFAULT 'active'
);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` | UUID v4 string. |
| `channel` | `TEXT` | `NOT NULL` | Channel name (e.g., `"telegram"`, `"whatsapp"`). |
| `sender_id` | `TEXT` | `NOT NULL` | Platform-specific user identifier. |
| `started_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | When the conversation was created. |
| `updated_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | Last modification timestamp. |
| `summary` | `TEXT` | nullable | AI-generated 1-2 sentence summary, set when conversation is closed. |
| `last_activity` | `TEXT` | `NOT NULL`, default `datetime('now')` | Timestamp of most recent message exchange. Used for idle detection. |
| `status` | `TEXT` | `NOT NULL`, default `'active'` | Either `'active'` or `'closed'`. |

**Indexes:**
- `idx_conversations_channel_sender` on `(channel, sender_id)` -- for lookup by user.
- `idx_conversations_status` on `(status, last_activity)` -- for idle conversation queries.

### Table: `messages`

Created by `001_init.sql`.

```sql
CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    role            TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content         TEXT NOT NULL,
    timestamp       TEXT NOT NULL DEFAULT (datetime('now')),
    metadata_json   TEXT
);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` | UUID v4 string. |
| `conversation_id` | `TEXT` | `NOT NULL`, FK to `conversations(id)` | The conversation this message belongs to. |
| `role` | `TEXT` | `NOT NULL`, CHECK `IN ('user', 'assistant')` | Who sent this message. |
| `content` | `TEXT` | `NOT NULL` | The message text. |
| `timestamp` | `TEXT` | `NOT NULL`, default `datetime('now')` | When the message was stored. |
| `metadata_json` | `TEXT` | nullable | JSON-serialized `MessageMetadata` for assistant messages. |

**Index:**
- `idx_messages_conversation` on `(conversation_id, timestamp)` -- for conversation history queries.

### Table: `facts`

Created by `001_init.sql`, **dropped and recreated** by `003_memory_enhancement.sql` to add `sender_id` scoping.

```sql
CREATE TABLE facts (
    id                TEXT PRIMARY KEY,
    sender_id         TEXT NOT NULL,
    key               TEXT NOT NULL,
    value             TEXT NOT NULL,
    source_message_id TEXT REFERENCES messages(id),
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(sender_id, key)
);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` | UUID v4 string. |
| `sender_id` | `TEXT` | `NOT NULL` | The user this fact belongs to. |
| `key` | `TEXT` | `NOT NULL` | Fact key (e.g., `"name"`, `"timezone"`, `"preference"`). |
| `value` | `TEXT` | `NOT NULL` | Fact value (e.g., `"Alice"`, `"America/New_York"`). |
| `source_message_id` | `TEXT` | FK to `messages(id)`, nullable | The message that originated this fact (currently unused by store code). |
| `created_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | When the fact was first stored. |
| `updated_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | When the fact was last updated (via upsert). |

**Unique constraint:** `(sender_id, key)` -- each user can have at most one value per key.

### Virtual Table: `messages_fts`

Created by `004_fts5_recall.sql`. FTS5 full-text search index over user messages.

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    content='messages',
    content_rowid='rowid'
);
```

| Property | Value |
|----------|-------|
| Type | FTS5 virtual table (content-sync) |
| Content table | `messages` |
| Rowid mapping | `messages.rowid` |
| Indexed columns | `content` |

**Content-sync mode:** Stores only the inverted index, not message text. Queries join back to `messages` via rowid.

**Auto-sync triggers:**

| Trigger | Event | Condition |
|---------|-------|-----------|
| `messages_fts_insert` | `AFTER INSERT ON messages` | `NEW.role = 'user'` |
| `messages_fts_delete` | `AFTER DELETE ON messages` | `OLD.role = 'user'` |
| `messages_fts_update` | `AFTER UPDATE OF content ON messages` | `NEW.role = 'user'` |

Only user messages are indexed. Assistant messages are excluded.

### Table: `audit_log`

Created by `002_audit_log.sql`. Not accessed by `Store` directly (managed by `AuditLogger`), but shares the same database pool.

### Table: `scheduled_tasks`

Created by `005_scheduled_tasks.sql`.

```sql
CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id           TEXT PRIMARY KEY,
    channel      TEXT NOT NULL,
    sender_id    TEXT NOT NULL,
    reply_target TEXT NOT NULL,
    description  TEXT NOT NULL,
    due_at       TEXT NOT NULL,
    repeat       TEXT,
    status       TEXT NOT NULL DEFAULT 'pending',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    delivered_at TEXT,
    -- Added by 007_task_type:
    task_type    TEXT NOT NULL DEFAULT 'reminder'
);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` | UUID v4 string. |
| `channel` | `TEXT` | `NOT NULL` | Channel name for delivery (e.g., `"telegram"`). |
| `sender_id` | `TEXT` | `NOT NULL` | User who created the task. |
| `reply_target` | `TEXT` | `NOT NULL` | Platform-specific delivery target (e.g., chat ID). |
| `description` | `TEXT` | `NOT NULL` | Human-readable task description (e.g., `"Call John"`). |
| `due_at` | `TEXT` | `NOT NULL` | ISO 8601 datetime when the task is due (e.g., `"2026-02-17T15:00:00"`). |
| `repeat` | `TEXT` | nullable | Recurrence pattern: `NULL` (one-shot), `"daily"`, `"weekly"`, `"monthly"`, `"weekdays"`. |
| `status` | `TEXT` | `NOT NULL`, default `'pending'` | Task state: `'pending'`, `'delivered'`, or `'cancelled'`. |
| `created_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | When the task was created. |
| `delivered_at` | `TEXT` | nullable | When the task was delivered (set on completion for one-shot tasks). |
| `task_type` | `TEXT` | `NOT NULL`, default `'reminder'` | Task type: `'reminder'` (simple message delivery) or `'action'` (provider-backed autonomous execution). |

**Indexes:**
- `idx_scheduled_tasks_due` on `(status, due_at)` -- for efficient due-task queries.
- `idx_scheduled_tasks_sender` on `(sender_id, status)` -- for per-user task listing.

### Table: `limitations`

Created by `006_limitations.sql`.

```sql
CREATE TABLE IF NOT EXISTS limitations (
    id            TEXT PRIMARY KEY,
    title         TEXT NOT NULL,
    description   TEXT NOT NULL,
    proposed_plan TEXT NOT NULL DEFAULT '',
    status        TEXT NOT NULL DEFAULT 'open',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at   TEXT
);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` | UUID v4 string. |
| `title` | `TEXT` | `NOT NULL` | Short title of the limitation (e.g., `"No email"`). |
| `description` | `TEXT` | `NOT NULL` | What the agent cannot do and why. |
| `proposed_plan` | `TEXT` | `NOT NULL`, default `''` | The agent's proposed plan to fix the limitation. |
| `status` | `TEXT` | `NOT NULL`, default `'open'` | Either `'open'` or `'resolved'`. |
| `created_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | When the limitation was first detected. |
| `resolved_at` | `TEXT` | nullable | When the limitation was resolved. |

**Indexes:**
- `idx_limitations_title` on `(title COLLATE NOCASE)` — case-insensitive unique index for deduplication.

### Table: `user_aliases`

Created by `008_user_aliases.sql`. Maps alternative sender IDs to a canonical sender ID for cross-channel user identity.

```sql
CREATE TABLE IF NOT EXISTS user_aliases (
    alias_sender_id     TEXT PRIMARY KEY,
    canonical_sender_id TEXT NOT NULL
);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `alias_sender_id` | `TEXT` | `PRIMARY KEY` | The alternative sender ID (e.g., WhatsApp phone number). |
| `canonical_sender_id` | `TEXT` | `NOT NULL` | The canonical sender ID that all facts are stored under (e.g., Telegram numeric ID). |

**Design:** The first channel to connect creates the canonical ID (via the `welcomed` fact). When a new channel connects and finds an existing welcomed user, an alias is created mapping the new sender_id to the existing canonical sender_id. All fact operations then use the canonical ID, while conversations keep using the original channel-specific sender_id.

## Migrations

### Migration Tracking

Migrations are tracked via the `_migrations` table. The system handles three scenarios:

1. **Fresh database** -- No tables exist. All migrations run in order.
2. **Pre-tracking database** -- Tables exist from before migration tracking was added. The `run_migrations()` method detects this by checking if the `conversations` table has the `summary` column (added in migration 003). If so, all three migrations are marked as applied without re-running.
3. **Normal operation** -- Each migration is checked against `_migrations` and skipped if already applied.

### Migration Files

| Name | File | Purpose |
|------|------|---------|
| `001_init` | `migrations/001_init.sql` | Creates `conversations`, `messages`, `facts` tables with indexes. |
| `002_audit_log` | `migrations/002_audit_log.sql` | Creates `audit_log` table with indexes. |
| `003_memory_enhancement` | `migrations/003_memory_enhancement.sql` | Adds `summary`, `last_activity`, `status` to `conversations`. Recreates `facts` with `sender_id` scoping. |
| `004_fts5_recall` | `migrations/004_fts5_recall.sql` | Creates `messages_fts` FTS5 virtual table, backfills existing user messages, adds auto-sync triggers. |
| `005_scheduled_tasks` | `migrations/005_scheduled_tasks.sql` | Creates `scheduled_tasks` table with indexes for user-scheduled reminders and recurring tasks. |
| `006_limitations` | `migrations/006_limitations.sql` | Creates `limitations` table with case-insensitive unique index for autonomous self-introspection. |
| `007_task_type` | `migrations/007_task_type.sql` | Adds `task_type` column to `scheduled_tasks` for distinguishing reminder vs action tasks. |
| `008_user_aliases` | `migrations/008_user_aliases.sql` | Creates `user_aliases` table for cross-channel user identity resolution. |

### Bootstrap Detection Logic

```
IF _migrations table is empty:
    IF conversations table has "summary" column:
        → Mark 001_init, 002_audit_log, 003_memory_enhancement as applied
    ELSE:
        → All migrations will run normally
```

**SQL used for detection:**
```sql
SELECT COUNT(*) FROM _migrations
SELECT sql FROM sqlite_master WHERE type='table' AND name='conversations'
```

The `sql` column from `sqlite_master` contains the CREATE TABLE statement. The code checks if it contains the substring `"summary"`.

## Functions

### Public Methods

#### `async fn new(config: &MemoryConfig) -> Result<Self, OmegaError>`

**Purpose:** Create a new store instance, initializing the database and running migrations.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `config` | `&MemoryConfig` | Memory configuration with `db_path` and `max_context_messages`. |

**Returns:** `Result<Self, OmegaError>`.

**Logic:**
1. Expand `~` in `config.db_path` via `shellexpand()`.
2. Create the parent directory if it does not exist.
3. Build `SqliteConnectOptions` with:
   - `create_if_missing(true)` -- create the database file if absent.
   - `journal_mode(Wal)` -- WAL mode for concurrent reads.
4. Create a connection pool with `max_connections(4)`.
5. Run all migrations via `run_migrations()`.
6. Log initialization with `info!`.
7. Return the new `Store`.

**Error conditions:**
- Parent directory creation fails.
- Invalid database path.
- SQLite connection failure.
- Migration failure.

---

#### `fn pool(&self) -> &SqlitePool`

**Purpose:** Get a reference to the underlying connection pool for direct SQL access.

**Parameters:** None.

**Returns:** `&SqlitePool`.

**Usage:** Called by `AuditLogger` construction and by `gateway.rs` for direct queries (e.g., fetching `sender_id` from conversations during summarization).

---

#### `async fn find_idle_conversations(&self) -> Result<Vec<(String, String, String)>, OmegaError>`

**Purpose:** Find active conversations that have been idle beyond the timeout threshold.

**Parameters:** None.

**Returns:** `Result<Vec<(String, String, String)>, OmegaError>` where each tuple is `(conversation_id, channel, sender_id)`.

**SQL:**
```sql
SELECT id, channel, sender_id FROM conversations
WHERE status = 'active'
AND datetime(last_activity) <= datetime('now', ? || ' minutes')
```

**Bind parameters:**
- `?` = `-CONVERSATION_TIMEOUT_MINUTES` (i.e., `-30`), which SQLite interprets as "30 minutes ago".

**Called by:** `gateway.rs::background_summarizer()` every 60 seconds.

---

#### `async fn find_all_active_conversations(&self) -> Result<Vec<(String, String, String)>, OmegaError>`

**Purpose:** Find all currently active conversations, regardless of idle time.

**Parameters:** None.

**Returns:** `Result<Vec<(String, String, String)>, OmegaError>` where each tuple is `(conversation_id, channel, sender_id)`.

**SQL:**
```sql
SELECT id, channel, sender_id FROM conversations WHERE status = 'active'
```

**Called by:** `gateway.rs::shutdown()` to summarize all conversations before exit.

---

#### `async fn get_conversation_messages(&self, conversation_id: &str) -> Result<Vec<(String, String)>, OmegaError>`

**Purpose:** Get all messages for a conversation, ordered chronologically.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `conversation_id` | `&str` | The conversation UUID. |

**Returns:** `Result<Vec<(String, String)>, OmegaError>` where each tuple is `(role, content)`.

**SQL:**
```sql
SELECT role, content FROM messages
WHERE conversation_id = ? ORDER BY timestamp ASC
```

**Called by:** `gateway.rs::summarize_conversation()` to build a transcript for the AI summarizer.

---

#### `async fn close_conversation(&self, conversation_id: &str, summary: &str) -> Result<(), OmegaError>`

**Purpose:** Mark a conversation as closed and store its summary.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `conversation_id` | `&str` | The conversation UUID. |
| `summary` | `&str` | AI-generated summary text. |

**Returns:** `Result<(), OmegaError>`.

**SQL:**
```sql
UPDATE conversations SET status = 'closed', summary = ?, updated_at = datetime('now') WHERE id = ?
```

**Called by:** `gateway.rs::summarize_conversation()` after summarization and fact extraction.

---

#### `async fn store_fact(&self, sender_id: &str, key: &str, value: &str) -> Result<(), OmegaError>`

**Purpose:** Store a user fact, upserting on `(sender_id, key)` conflict.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The user this fact belongs to. |
| `key` | `&str` | Fact key (e.g., `"name"`, `"timezone"`). |
| `value` | `&str` | Fact value (e.g., `"Alice"`, `"America/New_York"`). |

**Returns:** `Result<(), OmegaError>`.

**SQL:**
```sql
INSERT INTO facts (id, sender_id, key, value) VALUES (?, ?, ?, ?)
ON CONFLICT(sender_id, key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
```

**Behavior:** If a fact with the same `(sender_id, key)` already exists, the value is updated and `updated_at` is refreshed. Otherwise, a new row is inserted with a fresh UUID.

**Called by:** `gateway.rs::summarize_conversation()` for each extracted fact.

---

#### `async fn get_facts(&self, sender_id: &str) -> Result<Vec<(String, String)>, OmegaError>`

**Purpose:** Get all stored facts for a user.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The user whose facts to retrieve. |

**Returns:** `Result<Vec<(String, String)>, OmegaError>` where each tuple is `(key, value)`, ordered alphabetically by key.

**SQL:**
```sql
SELECT key, value FROM facts WHERE sender_id = ? ORDER BY key
```

**Called by:** `build_context()` for enriching the system prompt, and `commands.rs` for the `/facts` command.

---

#### `async fn get_all_facts(&self) -> Result<Vec<(String, String)>, OmegaError>`

**Purpose:** Get all facts across all users — for heartbeat context enrichment.

**Parameters:** None.

**Returns:** `Result<Vec<(String, String)>, OmegaError>` where each tuple is `(key, value)`, ordered alphabetically by key.

**SQL:**
```sql
SELECT key, value FROM facts WHERE key != 'welcomed' ORDER BY key
```

**Note:** Excludes the `welcomed` fact (an internal marker) from results.

**Called by:** `gateway.rs::heartbeat_loop()` to inject user facts into the heartbeat provider prompt.

---

#### `async fn get_all_recent_summaries(&self, limit: i64) -> Result<Vec<(String, String)>, OmegaError>`

**Purpose:** Get recent conversation summaries across all users — for heartbeat context enrichment.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `limit` | `i64` | Maximum number of summaries to return. |

**Returns:** `Result<Vec<(String, String)>, OmegaError>` where each tuple is `(summary, updated_at)`, ordered newest first.

**SQL:**
```sql
SELECT summary, updated_at FROM conversations
WHERE status = 'closed' AND summary IS NOT NULL
ORDER BY updated_at DESC LIMIT ?
```

**Called by:** `gateway.rs::heartbeat_loop()` with `limit = 3` to inject recent conversation context into the heartbeat provider prompt.

---

#### `async fn get_fact(&self, sender_id: &str, key: &str) -> Result<Option<String>, OmegaError>`

**Purpose:** Get a single fact value by sender and key.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The user whose fact to retrieve. |
| `key` | `&str` | The fact key to look up. |

**Returns:** `Result<Option<String>, OmegaError>` -- `Some(value)` if the fact exists, `None` otherwise.

**SQL:**
```sql
SELECT value FROM facts WHERE sender_id = ? AND key = ?
```

**Called by:** `gateway.rs` for active project lookup, `commands.rs` for `/projects` and `/project` commands.

---

#### `async fn delete_fact(&self, sender_id: &str, key: &str) -> Result<bool, OmegaError>`

**Purpose:** Delete a single fact by sender and key.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The user whose fact to delete. |
| `key` | `&str` | The fact key to delete. |

**Returns:** `Result<bool, OmegaError>` -- `true` if a row was deleted, `false` if the fact did not exist.

**SQL:**
```sql
DELETE FROM facts WHERE sender_id = ? AND key = ?
```

**Called by:** `commands.rs` for `/project off` (deactivate project).

---

#### `async fn get_recent_summaries(&self, channel: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String)>, OmegaError>`

**Purpose:** Get recent closed conversation summaries for a user on a specific channel.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `channel` | `&str` | Channel name (e.g., `"telegram"`). |
| `sender_id` | `&str` | The user whose summaries to retrieve. |
| `limit` | `i64` | Maximum number of summaries to return. |

**Returns:** `Result<Vec<(String, String)>, OmegaError>` where each tuple is `(summary, updated_at)`, ordered newest first.

**SQL:**
```sql
SELECT summary, updated_at FROM conversations
WHERE channel = ? AND sender_id = ? AND status = 'closed' AND summary IS NOT NULL
ORDER BY updated_at DESC LIMIT ?
```

**Called by:** `build_context()` with `limit = 3` to include the 3 most recent conversation summaries in the system prompt.

---

#### `async fn get_memory_stats(&self, sender_id: &str) -> Result<(i64, i64, i64), OmegaError>`

**Purpose:** Get aggregate memory statistics for a user.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The user to get stats for. |

**Returns:** `Result<(i64, i64, i64), OmegaError>` where the tuple is `(conversation_count, message_count, fact_count)`.

**SQL (three queries):**

```sql
-- Conversation count
SELECT COUNT(*) FROM conversations WHERE sender_id = ?

-- Message count (via join)
SELECT COUNT(*) FROM messages m
JOIN conversations c ON m.conversation_id = c.id
WHERE c.sender_id = ?

-- Fact count
SELECT COUNT(*) FROM facts WHERE sender_id = ?
```

**Called by:** `commands.rs` for the `/status` command.

---

#### `async fn get_history(&self, channel: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String)>, OmegaError>`

**Purpose:** Get conversation history (summaries with timestamps) for a user.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `channel` | `&str` | Channel name. |
| `sender_id` | `&str` | The user whose history to retrieve. |
| `limit` | `i64` | Maximum number of history entries. |

**Returns:** `Result<Vec<(String, String)>, OmegaError>` where each tuple is `(summary_or_fallback, updated_at)`, ordered newest first.

**SQL:**
```sql
SELECT COALESCE(summary, '(no summary)'), updated_at FROM conversations
WHERE channel = ? AND sender_id = ? AND status = 'closed'
ORDER BY updated_at DESC LIMIT ?
```

**Note:** Uses `COALESCE` to handle conversations that were closed without a summary (returns `"(no summary)"` as fallback).

**Called by:** `commands.rs` for the `/memory` or `/history` command.

---

#### `async fn delete_facts(&self, sender_id: &str, key: Option<&str>) -> Result<u64, OmegaError>`

**Purpose:** Delete facts for a user -- either all facts or a specific fact by key.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The user whose facts to delete. |
| `key` | `Option<&str>` | If `Some(k)`, delete only the fact with that key. If `None`, delete all facts for the user. |

**Returns:** `Result<u64, OmegaError>` -- the number of rows deleted.

**SQL (conditional):**
```sql
-- When key is Some(k):
DELETE FROM facts WHERE sender_id = ? AND key = ?

-- When key is None:
DELETE FROM facts WHERE sender_id = ?
```

**Called by:** `commands.rs` for the `/forget` command (fact deletion variant).

---

#### `async fn close_current_conversation(&self, channel: &str, sender_id: &str) -> Result<bool, OmegaError>`

**Purpose:** Close the active conversation for a user without a summary (for the `/forget` command).

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `channel` | `&str` | Channel name. |
| `sender_id` | `&str` | The user whose conversation to close. |

**Returns:** `Result<bool, OmegaError>` -- `true` if a conversation was closed, `false` if none was active.

**SQL:**
```sql
UPDATE conversations SET status = 'closed', updated_at = datetime('now')
WHERE channel = ? AND sender_id = ? AND status = 'active'
```

**Note:** Does not set a summary. The conversation is simply marked as closed.

**Called by:** `commands.rs` for the `/forget` command (conversation reset variant).

---

#### `async fn db_size(&self) -> Result<u64, OmegaError>`

**Purpose:** Get the database file size in bytes.

**Parameters:** None.

**Returns:** `Result<u64, OmegaError>`.

**SQL (two PRAGMA queries):**
```sql
PRAGMA page_count
PRAGMA page_size
```

**Calculation:** `page_count * page_size` cast to `u64`.

**Called by:** `commands.rs` for the `/status` command.

---

#### `async fn search_messages(&self, query: &str, exclude_conversation_id: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String, String)>, OmegaError>`

**Purpose:** Search past messages across all conversations using FTS5 full-text search for cross-conversation recall.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | `&str` | The search query (typically the user's current message text). |
| `exclude_conversation_id` | `&str` | Conversation ID to exclude from results (the current conversation). |
| `sender_id` | `&str` | User identifier — only messages from this user's conversations are searched. |
| `limit` | `i64` | Maximum number of results to return. |

**Returns:** `Result<Vec<(String, String, String)>, OmegaError>` where each tuple is `(role, content, timestamp)`, ordered by FTS5 BM25 relevance rank.

**Short query guard:** Queries shorter than 3 characters return an empty vec immediately (short queries produce noisy results).

**SQL:**
```sql
SELECT m.role, m.content, m.timestamp
FROM messages_fts fts
JOIN messages m ON m.rowid = fts.rowid
JOIN conversations c ON c.id = m.conversation_id
WHERE messages_fts MATCH ?
AND m.conversation_id != ?
AND c.sender_id = ?
ORDER BY rank
LIMIT ?
```

**Security:** The `sender_id` filter ensures users can only recall their own messages.

**Called by:** `build_context()` with the incoming message text as query, `limit = 5`.

---

#### `async fn build_context(&self, incoming: &IncomingMessage, base_system_prompt: &str) -> Result<Context, OmegaError>`

**Purpose:** Build a complete conversation context for the AI provider from the current conversation state, user facts, recent summaries, and recalled past messages.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `incoming` | `&IncomingMessage` | The incoming message (provides `channel`, `sender_id`, `text`). |
| `base_system_prompt` | `&str` | The base system prompt (identity + soul + rules), composed by the gateway from `Prompts.identity`, `Prompts.soul`, and `Prompts.system`. |

**Returns:** `Result<Context, OmegaError>`.

**Logic:**
1. Call `get_or_create_conversation(channel, sender_id)` to get the active conversation ID.
2. Fetch recent messages from the conversation (newest first, then reversed to chronological order).
3. Fetch all facts for the sender (errors suppressed, default to empty).
4. Fetch the 3 most recent closed conversation summaries (errors suppressed, default to empty).
5. Search past messages via FTS5 for cross-conversation recall (errors suppressed, default to empty).
6. Resolve language preference:
   - If a `preferred_language` fact exists in the fetched facts, use it.
   - Otherwise, call `detect_language(&incoming.text)` and store the result as a `preferred_language` fact.
7. Compute progressive onboarding stage:
   - Read `onboarding_stage` from facts (default 0).
   - Call `compute_onboarding_stage()` with current stage, real fact count, and task presence.
   - If stage advanced: store new stage via `store_fact()`, pass `Some(new_stage)` to `build_system_prompt()`.
   - If first contact (no stage fact, 0 real facts): pass `Some(0)` (show intro).
   - If pre-existing user with no stage fact: silently bootstrap their stage, pass `None` (no hint).
   - Otherwise: pass `None` (no hint).
8. Build a dynamic system prompt via `build_system_prompt()`, passing the `base_system_prompt`, resolved language, and onboarding hint.
9. Return a `Context` with the system prompt, history, and current message.

**SQL (step 2):**
```sql
SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY timestamp DESC LIMIT ?
```

**Bind parameters:** `conversation_id`, `max_context_messages`.

**Context construction flow:**
```
get_or_create_conversation(channel, sender_id)
          |
          v
    conversation_id
          |
    ┌─────┴──────────────────────────────────┐
    v                                         v
SELECT messages                          get_facts(sender_id)
(newest N, reversed)                          |
    |                                         v
    |                                   get_recent_summaries(channel, sender_id, 3)
    |                                         |
    |                                         v
    |                                   search_messages(text, conv_id, sender_id, 5)
    |                                         |
    |                                         v
    |                                   resolve language:
    |                                     preferred_language fact? → use it
    |                                     else → detect_language(text) + store fact
    |                                         |
    v                                         v
history: Vec<ContextEntry>     build_system_prompt(base_system_prompt, facts, summaries, recall, tasks, language)
    |                                         |
    v                                         v
    └──────────────┬─────────────────────────┘
                   v
            Context {
              system_prompt,
              history,
              current_message: incoming.text
            }
```

**Error handling:**
- `get_or_create_conversation` failure propagates as `OmegaError`.
- Message query failure propagates as `OmegaError`.
- `get_facts` failure silently defaults to empty vec (`unwrap_or_default()`).
- `get_recent_summaries` failure silently defaults to empty vec (`unwrap_or_default()`).
- `search_messages` failure silently defaults to empty vec (`unwrap_or_default()`).

---

#### `async fn store_exchange(&self, incoming: &IncomingMessage, response: &OutgoingMessage) -> Result<(), OmegaError>`

**Purpose:** Store a user message and the corresponding assistant response in the database.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `incoming` | `&IncomingMessage` | The user's message. |
| `response` | `&OutgoingMessage` | The assistant's response. |

**Returns:** `Result<(), OmegaError>`.

**Logic:**
1. Call `get_or_create_conversation(channel, sender_id)` to get the conversation ID.
2. Insert the user message with `role = 'user'`.
3. Serialize `response.metadata` to JSON.
4. Insert the assistant message with `role = 'assistant'` and `metadata_json`.

**SQL (two inserts):**
```sql
-- User message
INSERT INTO messages (id, conversation_id, role, content) VALUES (?, ?, 'user', ?)

-- Assistant message
INSERT INTO messages (id, conversation_id, role, content, metadata_json) VALUES (?, ?, 'assistant', ?, ?)
```

**Called by:** `gateway.rs::handle_message()` after a successful provider call.

---

#### `async fn create_task(&self, channel: &str, sender_id: &str, reply_target: &str, description: &str, due_at: &str, repeat: Option<&str>, task_type: &str) -> Result<String, OmegaError>`

**Purpose:** Create a new scheduled task with two-level deduplication.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `channel` | `&str` | Channel name for delivery (e.g., `"telegram"`). |
| `sender_id` | `&str` | User who created the task. |
| `reply_target` | `&str` | Platform-specific delivery target (e.g., chat ID). |
| `description` | `&str` | Human-readable task description. |
| `due_at` | `&str` | ISO 8601 datetime when the task is due (normalized before storage). |
| `repeat` | `Option<&str>` | Recurrence pattern (`None` for one-shot, `Some("daily")`, etc.). |
| `task_type` | `&str` | Task type: `"reminder"` (simple delivery) or `"action"` (provider-backed execution). |

**Returns:** `Result<String, OmegaError>` -- the UUID of the created (or reused) task.

**Deduplication (two levels):**
1. **Exact match:** Same sender + description + normalized `due_at` → returns existing task ID.
2. **Fuzzy match:** Same sender + similar description (word overlap ≥ 50%, min 3 significant words) + `due_at` within 30 minutes → returns existing task ID.

**Datetime normalization:** `due_at` is normalized before comparison and storage — trailing `Z` stripped, `T` separator replaced with space (e.g., `2026-02-22T07:00:00Z` → `2026-02-22 07:00:00`).

**SQL:**
```sql
INSERT INTO scheduled_tasks (id, channel, sender_id, reply_target, description, due_at, repeat, task_type)
VALUES (?, ?, ?, ?, ?, ?, ?, ?)
```

**Called by:** `gateway.rs::handle_message()` Stage 5b (SCHEDULE and SCHEDULE_ACTION marker extraction), `gateway.rs::scheduler_loop()` for nested tasks from action responses.

---

#### `async fn get_due_tasks(&self) -> Result<Vec<(String, String, String, String, String, Option<String>, String)>, OmegaError>`

**Purpose:** Get tasks that are due for delivery (status is pending and due_at is in the past or now).

**Parameters:** None.

**Returns:** `Result<Vec<(String, String, String, String, String, Option<String>, String)>, OmegaError>` where each tuple is `(id, channel, sender_id, reply_target, description, repeat, task_type)`.

**SQL:**
```sql
SELECT id, channel, sender_id, reply_target, description, repeat, task_type
FROM scheduled_tasks
WHERE status = 'pending' AND datetime(due_at) <= datetime('now')
```

**Called by:** `gateway.rs::scheduler_loop()` every `poll_interval_secs` seconds.

---

#### `async fn complete_task(&self, id: &str, repeat: Option<&str>) -> Result<(), OmegaError>`

**Purpose:** Complete a task after delivery. One-shot tasks are marked as delivered. Recurring tasks have their `due_at` advanced by the repeat interval.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `&str` | The task UUID. |
| `repeat` | `Option<&str>` | Recurrence pattern (`None` or `Some("once")` for one-shot, `Some("daily")`, etc.). |

**Returns:** `Result<(), OmegaError>`.

**Logic:**
1. If `repeat` is `None` or `"once"`:
   - Set `status = 'delivered'` and `delivered_at = datetime('now')`.
2. If `repeat` is a recurring pattern:
   - Advance `due_at` by the interval:
     - `"daily"` or `"weekdays"` → `+1 day`
     - `"weekly"` → `+7 days`
     - `"monthly"` → `+1 month`
     - Unknown → `+1 day` (fallback)
   - For `"weekdays"`: if the new `due_at` falls on Saturday (strftime `%w` = 6), advance by 2 more days to Monday. If it falls on Sunday (`%w` = 0), advance by 1 more day to Monday.

**SQL (one-shot):**
```sql
UPDATE scheduled_tasks SET status = 'delivered', delivered_at = datetime('now') WHERE id = ?
```

**SQL (recurring advance):**
```sql
UPDATE scheduled_tasks SET due_at = datetime(due_at, '<offset>') WHERE id = ?
```

**SQL (weekday skip Saturday):**
```sql
UPDATE scheduled_tasks SET due_at = datetime(due_at, '+2 days')
WHERE id = ? AND CAST(strftime('%w', due_at) AS INTEGER) = 6
```

**SQL (weekday skip Sunday):**
```sql
UPDATE scheduled_tasks SET due_at = datetime(due_at, '+1 day')
WHERE id = ? AND CAST(strftime('%w', due_at) AS INTEGER) = 0
```

**Called by:** `gateway.rs::scheduler_loop()` after successful delivery.

---

#### `async fn get_tasks_for_sender(&self, sender_id: &str) -> Result<Vec<(String, String, String, Option<String>, String)>, OmegaError>`

**Purpose:** Get all pending tasks for a specific user (for the `/tasks` command).

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The user whose tasks to retrieve. |

**Returns:** `Result<Vec<(String, String, String, Option<String>, String)>, OmegaError>` where each tuple is `(id, description, due_at, repeat, task_type)`, ordered by `due_at` ascending.

**SQL:**
```sql
SELECT id, description, due_at, repeat, task_type
FROM scheduled_tasks
WHERE sender_id = ? AND status = 'pending'
ORDER BY due_at ASC
```

**Called by:** `commands.rs` for the `/tasks` command.

---

#### `async fn cancel_task(&self, id_prefix: &str, sender_id: &str) -> Result<bool, OmegaError>`

**Purpose:** Cancel a task by ID prefix. Idempotent — returns `true` if the task was cancelled or was already cancelled.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id_prefix` | `&str` | The beginning of the task UUID (e.g., first 8 characters). |
| `sender_id` | `&str` | The user requesting cancellation (ownership check). |

**Returns:** `Result<bool, OmegaError>` -- `true` if a task was cancelled or already cancelled, `false` if no matching task found.

**Logic:**
1. Try to update pending tasks to cancelled:
```sql
UPDATE scheduled_tasks SET status = 'cancelled'
WHERE id LIKE ? AND sender_id = ? AND status = 'pending'
```
2. If `rows_affected > 0`, return `true`.
3. Otherwise, check if already cancelled (idempotent):
```sql
SELECT COUNT(*) FROM scheduled_tasks
WHERE id LIKE ? AND sender_id = ? AND status = 'cancelled'
```
4. If count > 0, return `true` (intent already fulfilled). Otherwise `false`.

**Bind parameters:** `id_prefix` is bound as `"{id_prefix}%"` for prefix matching via `LIKE`.

**Security:** The `sender_id` filter ensures users can only cancel their own tasks.

**Called by:** `commands.rs` for the `/cancel` command.

---

#### `async fn store_limitation(&self, title: &str, description: &str, proposed_plan: &str) -> Result<bool, OmegaError>`

**Purpose:** Store a self-detected limitation with case-insensitive title deduplication.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `title` | `&str` | Short limitation title (e.g., `"No email"`). |
| `description` | `&str` | What the agent cannot do and why. |
| `proposed_plan` | `&str` | The agent's proposed plan to fix it. |

**Returns:** `Result<bool, OmegaError>` -- `true` if the limitation is new, `false` if it already existed (duplicate title).

**SQL:**
```sql
INSERT OR IGNORE INTO limitations (id, title, description, proposed_plan) VALUES (?, ?, ?, ?)
```

**Note:** Uses `INSERT OR IGNORE` — if the title already exists (case-insensitive match via the unique index), the insert is silently ignored and `rows_affected()` returns 0.

**Called by:** `gateway.rs::handle_message()` Stage 5h (SKILL_IMPROVE marker extraction) and `gateway.rs::heartbeat_loop()`.

---

#### `async fn resolve_sender_id(&self, sender_id: &str) -> Result<String, OmegaError>`

**Purpose:** Resolve a sender_id to its canonical form via the `user_aliases` table.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `sender_id` | `&str` | The sender ID to resolve. |

**Returns:** `Result<String, OmegaError>` -- the canonical sender_id if an alias exists, otherwise the original sender_id.

**SQL:**
```sql
SELECT canonical_sender_id FROM user_aliases WHERE alias_sender_id = ?
```

**Called by:** `gateway.rs::handle_message()` at the top of the pipeline, before any fact operations.

---

#### `async fn create_alias(&self, alias_id: &str, canonical_id: &str) -> Result<(), OmegaError>`

**Purpose:** Create a mapping from an alias sender_id to a canonical sender_id.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `alias_id` | `&str` | The alternative sender ID (e.g., WhatsApp phone number). |
| `canonical_id` | `&str` | The canonical sender ID (e.g., Telegram numeric ID). |

**Returns:** `Result<(), OmegaError>`.

**SQL:**
```sql
INSERT OR IGNORE INTO user_aliases (alias_sender_id, canonical_sender_id) VALUES (?, ?)
```

**Note:** Uses `INSERT OR IGNORE` — if the alias already exists, the insert is silently ignored (idempotent).

**Called by:** `gateway.rs::handle_message()` when a new user is detected on a second channel and an existing welcomed user is found.

---

#### `async fn find_canonical_user(&self, exclude_sender_id: &str) -> Result<Option<String>, OmegaError>`

**Purpose:** Find an existing welcomed user's sender_id, excluding the given sender_id. Used to create cross-channel aliases.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `exclude_sender_id` | `&str` | The sender ID to exclude from the search. |

**Returns:** `Result<Option<String>, OmegaError>` -- `Some(canonical_sender_id)` if found, `None` if no other welcomed user exists.

**SQL:**
```sql
SELECT sender_id FROM facts WHERE key = 'welcomed' AND sender_id != ? LIMIT 1
```

**Called by:** `gateway.rs::handle_message()` during first-time user detection to check for cross-channel identity.

---

#### `async fn get_open_limitations(&self) -> Result<Vec<(String, String, String)>, OmegaError>`

**Purpose:** Get all open (unresolved) limitations for heartbeat context enrichment.

**Parameters:** None.

**Returns:** `Result<Vec<(String, String, String)>, OmegaError>` where each tuple is `(title, description, proposed_plan)`, ordered by creation time ascending.

**SQL:**
```sql
SELECT title, description, proposed_plan FROM limitations WHERE status = 'open' ORDER BY created_at ASC
```

**Called by:** `gateway.rs::heartbeat_loop()` to inject known limitations into the heartbeat prompt.

---

### Private Methods

#### `async fn run_migrations(pool: &SqlitePool) -> Result<(), OmegaError>`

**Purpose:** Run SQL migrations with tracking to avoid re-execution.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `pool` | `&SqlitePool` | The database connection pool. |

**Returns:** `Result<(), OmegaError>`.

**Logic:**
1. Create `_migrations` table if it does not exist.
2. Check if any migrations have been recorded.
3. If no migrations recorded, check for pre-tracking schema and bootstrap if needed.
4. Iterate through all migration definitions.
5. For each migration, check if it has been applied. If not, execute the SQL and record it.

**Migration definitions (compile-time embedded):**
```rust
let migrations: &[(&str, &str)] = &[
    ("001_init", include_str!("../migrations/001_init.sql")),
    ("002_audit_log", include_str!("../migrations/002_audit_log.sql")),
    ("003_memory_enhancement", include_str!("../migrations/003_memory_enhancement.sql")),
    ("004_fts5_recall", include_str!("../migrations/004_fts5_recall.sql")),
    ("005_scheduled_tasks", include_str!("../migrations/005_scheduled_tasks.sql")),
    ("006_limitations", include_str!("../migrations/006_limitations.sql")),
    ("007_task_type", include_str!("../migrations/007_task_type.sql")),
    ("008_user_aliases", include_str!("../migrations/008_user_aliases.sql")),
];
```

---

#### `async fn get_or_create_conversation(&self, channel: &str, sender_id: &str) -> Result<String, OmegaError>`

**Purpose:** Get the active conversation for a user/channel pair, or create a new one if none exists or the existing one has timed out.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `channel` | `&str` | Channel name. |
| `sender_id` | `&str` | User identifier. |

**Returns:** `Result<String, OmegaError>` -- the conversation UUID.

**Logic:**
1. Query for an active conversation within the timeout window.
2. If found, update `last_activity` and `updated_at` to now, return the ID.
3. If not found, generate a new UUID, insert a new conversation, return the new ID.

**SQL (lookup):**
```sql
SELECT id FROM conversations
WHERE channel = ? AND sender_id = ? AND status = 'active'
AND datetime(last_activity) > datetime('now', ? || ' minutes')
ORDER BY last_activity DESC LIMIT 1
```

**SQL (touch):**
```sql
UPDATE conversations SET last_activity = datetime('now'), updated_at = datetime('now') WHERE id = ?
```

**SQL (create):**
```sql
INSERT INTO conversations (id, channel, sender_id, status, last_activity)
VALUES (?, ?, ?, 'active', datetime('now'))
```

**Conversation boundary logic:**

```
User sends message
       |
       v
Query: active + within 30min?
       |
  ┌────┴────┐
  |         |
 Yes       No
  |         |
  v         v
Touch    Create new
activity conversation
  |         |
  v         v
Return   Return
same ID  new ID
```

## Private Free Functions

### `shellexpand` (imported from `omega_core::shellexpand`)

The `shellexpand()` utility is now a public function in `omega_core::config`, re-exported as `omega_core::shellexpand`. It expands `~/` prefix to `$HOME/`. This store imports it rather than defining its own copy.

---

### `fn compute_onboarding_stage(current_stage: u8, real_fact_count: usize, has_tasks: bool) -> u8`

**Purpose:** Compute the next onboarding stage based on current state. Stages are sequential and cannot be skipped.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `current_stage` | `u8` | The user's current onboarding stage (0-5). |
| `real_fact_count` | `usize` | Number of non-system facts stored for this user. |
| `has_tasks` | `bool` | Whether the user has any pending scheduled tasks. |

**Returns:** `u8` -- the new onboarding stage.

**Stage transitions:**

| Current | Condition | Next |
|---------|-----------|------|
| 0 | `real_fact_count >= 1` | 1 |
| 1 | `real_fact_count >= 3` | 2 |
| 2 | `has_tasks` | 3 |
| 3 | `real_fact_count >= 5` | 4 |
| 4 | Always | 5 |
| 5+ | Never | Same |

---

### `fn onboarding_hint_text(stage: u8, language: &str) -> Option<String>`

**Purpose:** Return the prompt hint text for a given onboarding stage.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `stage` | `u8` | The onboarding stage to get the hint for. |
| `language` | `&str` | The user's preferred language (used in stage 0 intro). |

**Returns:** `Option<String>` -- `Some(hint)` for stages 0-4, `None` for stage 5+.

**Hints by stage:**

| Stage | What OMEGA teaches | Key content |
|-------|-------------------|-------------|
| 0 | Who OMEGA is | First-contact introduction. Uses "an appropriate greeting in {language}" (no hardcoded greeting). |
| 1 | /help exists | "ask me anything or type /help". Appends "Respond in {language}." |
| 2 | Personality | "tell me how to behave, or /personality". Appends "Respond in {language}." |
| 3 | Task management | "say 'show my tasks' or /tasks". Appends "Respond in {language}." |
| 4 | Projects | "organize work into projects — /projects". Appends "Respond in {language}." |
| 5+ | Done | `None` (no more hints) |

**Language awareness:** Stage 0 uses a dynamic greeting instruction instead of a hardcoded '¡Hola!' to respect the user's detected language. Stages 1-4 append "Respond in {language}." so the AI delivers the onboarding hint in the correct language.

---

### `fn build_system_prompt(base_rules: &str, facts: &[(String, String)], summaries: &[(String, String)], recall: &[(String, String, String)], pending_tasks: &[(String, String, String, Option<String>, String)], language: &str, onboarding_hint: Option<u8>) -> String`

**Purpose:** Build a dynamic system prompt enriched with user facts, conversation summaries, recalled past messages, and explicit language instruction.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `base_rules` | `&str` | Base prompt text (identity + soul + rules), composed by the gateway from `Prompts.identity`, `Prompts.soul`, and `Prompts.system`. |
| `facts` | `&[(String, String)]` | User facts as `(key, value)` pairs. |
| `summaries` | `&[(String, String)]` | Recent conversation summaries as `(summary, timestamp)` pairs. |
| `recall` | `&[(String, String, String)]` | Recalled past messages as `(role, content, timestamp)` tuples. |
| `pending_tasks` | `&[(String, String, String, Option<String>, String)]` | Pending scheduled tasks as `(id, description, due_at, repeat, task_type)`. |
| `language` | `&str` | The user's preferred language (e.g., `"English"`, `"Spanish"`). |
| `onboarding_hint` | `Option<u8>` | If `Some(stage)`, inject the hint for that stage. If `None`, no onboarding hint. |

**Returns:** `String` -- the complete system prompt.

**Output structure:**
```
<base_rules content (identity + soul + system, composed by gateway)>

User profile:                         ← (only if real facts exist, via format_user_profile())
- preferred_name: Alice               ← identity keys first
- pronouns: she/her
- timezone: America/New_York          ← context keys second
- favorite_language: Rust             ← remaining keys last

(onboarding hint)                     ← (only when <3 real facts, after language line)

Recent conversation history:          ← (only if summaries is non-empty)
- [2024-01-15 14:30:00] User asked about Rust async patterns.
- [2024-01-14 09:15:00] User discussed project architecture.

Related past context:                 ← (only if recall is non-empty)
- [2024-01-10 16:00:00] User: How do I use tokio::spawn for background tasks...
- [2024-01-08 11:30:00] User: I need to set up nginx reverse proxy for port 8080...

IMPORTANT: Always respond in Spanish.  ← (always present, uses language parameter)

If the user explicitly asks you to change language (e.g. 'speak in French'),
respond in the requested language. Include LANG_SWITCH: <language> on its own line
at the END of your response.

To schedule a task, include this marker on its own line at the END of your response:
SCHEDULE: <description> | <ISO 8601 datetime> | <once|daily|weekly|monthly|weekdays>
Example: SCHEDULE: Call John | 2026-02-17T15:00:00 | once
Use this when the user asks for a reminder AND proactively after any action you take
that warrants follow-up. After every action, ask yourself: does this need a check later?
If yes, schedule it. An autonomous agent closes its own loops.

To schedule an autonomous action (you will be invoked with full tool access when due):
SCHEDULE_ACTION: <description> | <ISO 8601 datetime> | <once|daily|weekly|monthly|weekdays>
Example: SCHEDULE_ACTION: Check deployment status | 2026-02-17T16:00:00 | once
Use this when the follow-up requires you to actually DO something (run commands, check
services, analyze data) rather than just remind the user. When the action fires, you
will be invoked as if the user sent the description as a message, with full tool access.

To add something to your periodic monitoring checklist, include this marker on its
own line at the END of your response:
HEARTBEAT_ADD: <description>
To remove something from monitoring:
HEARTBEAT_REMOVE: <description>
Use this when the user asks AND proactively when any action you take needs ongoing
monitoring. If something you did will evolve over time and could need attention,
add it to your watchlist. Don't wait to be told to keep an eye on your own actions.
To change the heartbeat check interval, include this marker on its own line:
HEARTBEAT_INTERVAL: <minutes>
Value must be between 1 and 1440 (24 hours). Use when the user asks to change how
often you check in (e.g., "check every 15 minutes").

```

**Conditional sections:**
- User profile section: appended only if real (non-system) facts exist, via `format_user_profile()`. Header is "User profile:" instead of "Known facts about this user:".
- Progressive onboarding hint: injected only when a stage transition fires (`onboarding_hint` is `Some(stage)`). Each hint teaches ONE feature and fires exactly once. Stage 0 = intro, 1 = /help, 2 = /personality, 3 = /tasks, 4 = /projects, 5+ = done (no hint). See `onboarding_hint_text()` for details.
- Summaries section: appended only if `summaries` is non-empty.
- Recall section: appended only if `recall` is non-empty. Each message is truncated to 200 characters.
- Language directive: always appended (unconditional). Uses the `language` parameter.
- LANG_SWITCH instruction: always appended (unconditional). Tells the provider to include a `LANG_SWITCH:` marker when the user explicitly asks to change language.
- SCHEDULE marker instructions: always appended (unconditional). Tells the provider to include a `SCHEDULE:` marker line when the user requests a reminder or scheduled task, AND proactively when the agent takes an action that needs follow-up.
- SCHEDULE_ACTION marker instructions: always appended (unconditional). Tells the provider to include a `SCHEDULE_ACTION:` marker line when the follow-up requires autonomous execution with full tool access rather than a simple reminder.
- HEARTBEAT_ADD/REMOVE/INTERVAL marker instructions: always appended (unconditional). Tells the provider to include `HEARTBEAT_ADD:` or `HEARTBEAT_REMOVE:` markers when the user requests monitoring changes, AND proactively when the agent takes an action that needs ongoing monitoring. Also includes `HEARTBEAT_INTERVAL:` instruction for dynamic interval changes (1–1440 minutes).
- SKILL_IMPROVE marker instructions: always appended (unconditional). Tells the provider to include a `SKILL_IMPROVE:` marker when it identifies a skill that could be improved.

---

### `fn format_user_profile(facts: &[(String, String)]) -> Option<String>`

**Purpose:** Format user facts into a structured "User profile:" block with intelligent grouping and system-key filtering.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `facts` | `&[(String, String)]` | All user facts as `(key, value)` pairs. |

**Returns:** `Option<String>` -- `Some(formatted_block)` if there are any real (non-system) facts, `None` otherwise.

**Logic:**
1. Filter out system keys: `welcomed`, `preferred_language`, `active_project`.
2. If no facts remain after filtering, return `None`.
3. Group remaining facts into three tiers:
   - **Identity keys** (first): e.g., `preferred_name`, `pronouns`, `location`, `occupation`.
   - **Context keys** (second): e.g., `timezone`, `primary_language`, `tech_stack`.
   - **Remaining keys** (last): everything else, alphabetically.
4. Format as "User profile:" header followed by `- key: value` lines.

**Called by:** `build_system_prompt()` to replace the previous flat "Known facts about this user:" dump.

---

### `fn detect_language(text: &str) -> &'static str`

**Purpose:** Detect the most likely language of a text using stop-word heuristics.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `text` | `&str` | The text to analyze. |

**Returns:** `&'static str` -- language name (e.g., `"English"`, `"Spanish"`, `"French"`).

**Logic:**
1. Convert text to lowercase.
2. For each supported language, count how many stop-words appear in the text.
3. The language with the highest count wins.
4. Require at least 2 matches to override the English default.
5. Return `"English"` if no language scores >= 2.

**Supported languages:**
- Spanish (15 stop-words)
- Portuguese (14 stop-words)
- French (14 stop-words)
- German (14 stop-words)
- Italian (14 stop-words)
- Dutch (14 stop-words)
- Russian (14 stop-words)

**Note:** This is a simple heuristic, not a language detection library. Some stop-words overlap between Romance languages. The user can always override with `/language <lang>`.

## Context Building Flow Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                          build_context()                              │
│                                                                       │
│  IncomingMessage { channel, sender_id, text }                        │
│         |                                                             │
│         v                                                             │
│  get_or_create_conversation(channel, sender_id)                      │
│         |                                                             │
│         v                                                             │
│  ┌──────────────────────────────┐                                    │
│  │ Active conversation exists?  │                                    │
│  │ (within 30min timeout)       │                                    │
│  └──────┬───────────┬──────────┘                                    │
│         |           |                                                 │
│        Yes         No                                                 │
│         |           |                                                 │
│         v           v                                                 │
│    Touch activity  Create new                                         │
│         |           |                                                 │
│         └─────┬─────┘                                                │
│               v                                                       │
│        conversation_id                                                │
│               |                                                       │
│    ┌──────────┼──────────────────────────────────┐                   │
│    v          v                  v                v                    │
│  SELECT     get_facts()    get_recent_      search_messages()        │
│  messages   (sender_id)    summaries()      (text, conv_id,          │
│  (DESC,     └──┬───┘      (channel,         sender_id, 5)           │
│   LIMIT N)     |           sender_id, 3)    └──────┬─────┘           │
│    |           v           └──┬──────┘             v                  │
│    v       facts[]            v               recall[]                │
│  Reverse                 summaries[]               |                  │
│  to ASC                       |                    |                  │
│    |                          v                    v                  │
│    |      build_system_prompt(facts, summaries, recall, text)        │
│    v                          |                                       │
│  history[]                    v                                       │
│    |                    system_prompt                                  │
│    |                          |                                       │
│    └───────────┬──────────────┘                                      │
│                v                                                      │
│          Context {                                                    │
│            system_prompt,                                             │
│            history,                                                   │
│            current_message: text                                      │
│          }                                                            │
└──────────────────────────────────────────────────────────────────────┘
```

## Conversation Lifecycle Diagram

```
┌───────────────────────────────────────────────────────┐
│              CONVERSATION LIFECYCLE                     │
│                                                         │
│  User sends first message                              │
│         |                                               │
│         v                                               │
│  ┌─────────────┐                                       │
│  │   CREATED    │  (status='active', new UUID)         │
│  └──────┬──────┘                                       │
│         |                                               │
│         v                                               │
│  ┌─────────────┐  User sends message  ┌────────────┐  │
│  │   ACTIVE     │◄────────────────────│ Touch      │  │
│  │              │     (< 30min)       │ activity   │  │
│  └──────┬──────┘                      └────────────┘  │
│         |                                               │
│         | (30+ minutes idle)                           │
│         v                                               │
│  ┌─────────────────────────────┐                       │
│  │ background_summarizer finds │                       │
│  │ idle conversation           │                       │
│  └──────┬──────────────────────┘                       │
│         |                                               │
│         v                                               │
│  ┌─────────────┐                                       │
│  │ SUMMARIZE   │  AI generates summary                 │
│  │ + EXTRACT   │  AI extracts facts                    │
│  └──────┬──────┘                                       │
│         |                                               │
│         v                                               │
│  ┌─────────────┐                                       │
│  │   CLOSED     │  (status='closed', summary stored)   │
│  └─────────────┘                                       │
│                                                         │
│  ── Alternative closure paths ──                       │
│                                                         │
│  /forget command → close_current_conversation()        │
│    (no summary, just status='closed')                  │
│                                                         │
│  Shutdown → summarize_conversation() for all active    │
│    (summary stored, then status='closed')              │
│                                                         │
│  User sends message after 30min → new conversation     │
│    (old one stays 'active' until summarizer finds it)  │
└───────────────────────────────────────────────────────┘
```

## Error Handling Strategy

### Error Types
All errors are wrapped in `OmegaError::Memory(String)` with descriptive messages.

### Error Propagation

| Method | Error Behavior |
|--------|---------------|
| `new()` | Propagates all errors (fatal -- store cannot function without database). |
| `run_migrations()` | Propagates all errors (fatal -- schema must be correct). |
| `get_or_create_conversation()` | Propagates all errors (called internally). |
| `build_context()` | Propagates conversation/message errors. Suppresses fact/summary/recall errors (defaults to empty). |
| `search_messages()` | Propagates errors. |
| `store_exchange()` | Propagates all errors (caller in gateway logs but continues). |
| `find_idle_conversations()` | Propagates errors (caller in background task logs and continues). |
| `close_conversation()` | Propagates errors. |
| `store_fact()` | Propagates errors. |
| `get_facts()` | Propagates errors. |
| `delete_facts()` | Propagates errors. |
| `close_current_conversation()` | Propagates errors. |
| `get_memory_stats()` | Propagates errors. |
| `get_history()` | Propagates errors. |
| `get_recent_summaries()` | Propagates errors. |
| `db_size()` | Propagates errors. |
| `create_task()` | Propagates errors. |
| `get_due_tasks()` | Propagates errors (caller in scheduler loop logs and continues). |
| `complete_task()` | Propagates errors (caller in scheduler loop logs and continues). |
| `get_tasks_for_sender()` | Propagates errors. |
| `cancel_task()` | Propagates errors. |
| `get_fact()` | Propagates errors. |
| `get_all_facts()` | Propagates errors. |
| `get_all_recent_summaries()` | Propagates errors. |
| `delete_fact()` | Propagates errors. |
| `store_limitation()` | Propagates errors (caller in gateway/heartbeat logs and continues). |
| `get_open_limitations()` | Propagates errors (caller in heartbeat suppresses with default empty). |

### Resilience in build_context()
Facts, summaries, and recalled messages are fetched with `unwrap_or_default()`. This ensures that a failure in retrieving personalization data does not prevent the provider from receiving a valid context. The conversation will still work; it will just lack facts, summaries, or recalled context.

## Dependencies

### External Crates
- `sqlx` -- SQLite driver, connection pool, query execution.
- `uuid` -- UUID v4 generation for all primary keys.
- `tracing` -- Structured logging (`info!` macro).
- `serde_json` -- Serialization of `MessageMetadata` to JSON.

### Internal Dependencies
- `omega_core::config::MemoryConfig` -- Configuration struct.
- `omega_core::context::{Context, ContextEntry}` -- Context types returned by `build_context()`.
- `omega_core::error::OmegaError` -- Error type used for all results.
- `omega_core::message::{IncomingMessage, OutgoingMessage}` -- Message types used by `build_context()` and `store_exchange()`.
- `omega_core::shellexpand` -- Home directory expansion utility (replaces local copy).

## Tests

### Scheduled Task Tests

All tests use an in-memory SQLite store (`sqlite::memory:`) with migrations applied.

| Test | Purpose |
|------|---------|
| `test_create_and_get_tasks` | Creates a one-shot task, verifies it appears in `get_tasks_for_sender()` with correct description, due_at, and `None` repeat. |
| `test_get_due_tasks` | Creates a past-due task and a future task, verifies only the past-due task appears in `get_due_tasks()`. |
| `test_complete_one_shot` | Creates and completes a one-shot task, verifies it no longer appears in pending or due queries. |
| `test_complete_recurring` | Creates a daily recurring task with past due_at, completes it, verifies the task still exists with `due_at` advanced by 1 day. |
| `test_cancel_task` | Creates a task, cancels it by ID prefix, verifies it no longer appears in `get_tasks_for_sender()`. |
| `test_cancel_task_wrong_sender` | Creates a task for user1, attempts cancellation by user2, verifies it fails (returns `false`) and the task still exists for user1. |
| `test_get_fact` | Stores a fact, verifies `get_fact()` returns `Some(value)`. Also checks missing fact returns `None`. |
| `test_delete_fact` | Verifies `delete_fact()` returns `false` for non-existent fact, `true` after storing, and fact is gone after deletion. |
| `test_get_all_facts` | Stores facts including a `welcomed` fact, verifies `get_all_facts()` returns all facts except `welcomed`. |
| `test_get_all_recent_summaries` | Creates and closes conversations with summaries, verifies `get_all_recent_summaries()` returns them ordered newest-first and respects the limit parameter. |
| `test_store_limitation_new` | Stores a limitation, verifies `store_limitation()` returns `true` (new). |
| `test_store_limitation_duplicate` | Stores same title twice, verifies second call returns `false` (duplicate). |
| `test_store_limitation_case_insensitive` | Stores same title with different case, verifies dedup works case-insensitively. |
| `test_get_open_limitations` | Stores multiple limitations, verifies `get_open_limitations()` returns all with correct order. |
| `test_user_profile_filters_system_facts` | Verifies that `format_user_profile()` filters out system keys (`welcomed`, `preferred_language`, `active_project`) from the output. |
| `test_user_profile_groups_identity_first` | Verifies that `format_user_profile()` places identity keys (e.g., `preferred_name`, `pronouns`) before context keys and other facts. |
| `test_user_profile_empty_for_system_only` | Verifies that `format_user_profile()` returns `None` when all facts are system keys. |
| `test_onboarding_stage0_first_conversation` | Verifies stage 0 hint includes first-conversation intro. |
| `test_onboarding_stage1_help_hint` | Verifies stage 1 hint mentions `/help`. |
| `test_onboarding_no_hint_when_none` | Verifies no onboarding hint is injected when `onboarding_hint` is `None`. |
| `test_compute_onboarding_stage_sequential` | Verifies sequential stage advancement through all transitions (0→1→2→3→4→5). |
| `test_compute_onboarding_stage_no_skip` | Verifies stages cannot be skipped even when conditions for later stages are met. |
| `test_onboarding_hint_text_contains_commands` | Verifies each stage's hint text contains the right command (/help, /personality, /tasks, /projects). |
| `test_build_context_advances_onboarding_stage` | Integration test: verifies `build_context()` triggers stage transitions and stores the `onboarding_stage` fact. |
| `test_create_task_with_action_type` | Creates a task with `task_type = "action"`, verifies it appears in `get_tasks_for_sender()` with the correct task_type. |
| `test_get_due_tasks_returns_task_type` | Creates reminder and action tasks, verifies `get_due_tasks()` returns `task_type` as the 6th tuple element. |
| `test_build_system_prompt_shows_action_badge` | Verifies that `build_system_prompt()` includes an `[action]` badge for tasks with `task_type = "action"` in the pending tasks section. |
| `test_resolve_sender_id_no_alias` | Verifies `resolve_sender_id()` returns original ID when no alias exists. |
| `test_create_and_resolve_alias` | Creates an alias, verifies `resolve_sender_id()` returns canonical ID. |
| `test_create_alias_idempotent` | Creates same alias twice, verifies idempotent behavior (INSERT OR IGNORE). |
| `test_find_canonical_user` | Verifies `find_canonical_user()` returns None when empty, returns existing welcomed user, excludes self. |
| `test_alias_shares_facts` | Creates alias, verifies facts stored under canonical ID are accessible via resolved alias. |

## Invariants

1. Every conversation has a UUID v4 as its primary key.
2. Every message has a UUID v4 as its primary key.
3. Every fact has a UUID v4 as its primary key.
4. A conversation is either `'active'` or `'closed'` -- no other statuses exist.
5. Message roles are constrained to `'user'` or `'assistant'` by a CHECK constraint.
6. Facts are unique per `(sender_id, key)` -- duplicate inserts update the existing value.
7. `build_context()` always returns history in chronological order (oldest first).
8. `build_context()` never fails due to fact/summary/recall retrieval errors.
9. The conversation timeout is 30 minutes -- this is a compile-time constant, not configurable.
10. Migrations are idempotent -- running them multiple times has no effect.
11. The database is created with WAL journal mode for concurrent read access.
12. Connection pool is limited to 4 connections maximum.
13. Scheduled tasks have UUID v4 as their primary key.
14. Task status is one of `'pending'`, `'delivered'`, or `'cancelled'`.
15. Recurring tasks stay in `'pending'` status with an advanced `due_at`; one-shot tasks transition to `'delivered'`.
16. Task cancellation requires sender_id ownership check (users can only cancel their own tasks).
17. The SCHEDULE marker instruction is always included in the system prompt.
18. The language directive is always included in the system prompt (uses resolved `preferred_language` fact or auto-detected language).
19. The LANG_SWITCH marker instruction is always included in the system prompt.
20. The HEARTBEAT_ADD/REMOVE marker instructions are always included in the system prompt.
21. The SKILL_IMPROVE marker instruction is always included in the system prompt.
22. Skill improvement suggestions are deduplicated by title (case-insensitive) — duplicate inserts are silently ignored.
23. Limitation status is one of `'open'` or `'resolved'` (table retained for SKILL_IMPROVE storage).
24. The SCHEDULE_ACTION marker instruction is always included in the system prompt.
25. Task type is one of `'reminder'` or `'action'` — default is `'reminder'`.
