# Specification: omega-memory/migrations/003_memory_enhancement.sql

## Path

`crates/omega-memory/migrations/003_memory_enhancement.sql`

## Purpose

Enhances the memory schema (originally created in `001_init.sql`) with conversation lifecycle management and per-user fact scoping. This migration adds three columns to the `conversations` table for tracking status, summaries, and activity timestamps, creates an index to support status-based queries, and recreates the `facts` table with `sender_id` scoping so that facts are isolated per user rather than being globally unique by key alone.

This migration was introduced in Phase 3 alongside conversation boundary detection, automatic summarization of idle conversations, fact extraction from AI responses, and enriched context building.

## Prerequisites

- Migration `001_init.sql` must have been applied (creates `conversations`, `messages`, and `facts` tables).
- Migration `002_audit_log.sql` must have been applied (creates `audit_log` table).
- The `facts` table must be empty or expendable -- this migration drops and recreates it.

---

## Schema Changes

### ALTER TABLE: `conversations`

Three columns are added to the existing `conversations` table created in `001_init.sql`.

#### Original schema (from `001_init.sql`)

```sql
CREATE TABLE IF NOT EXISTS conversations (
    id          TEXT PRIMARY KEY,
    channel     TEXT NOT NULL,
    sender_id   TEXT NOT NULL,
    started_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

#### Added columns

| Column | Type | Constraints | Default | Purpose |
|--------|------|-------------|---------|---------|
| `summary` | `TEXT` | None (nullable) | `NULL` | Stores an AI-generated summary of the conversation after it is closed. Populated by `close_conversation()` in `store.rs`. |
| `last_activity` | `TEXT` | `NOT NULL` | `datetime('now')` | Tracks the most recent message activity in the conversation. Updated on every `get_or_create_conversation()` call. Used for idle conversation detection. |
| `status` | `TEXT` | `NOT NULL` | `'active'` | Conversation lifecycle state. Values: `'active'` (in progress) or `'closed'` (summarized and ended). |

#### SQL statements

```sql
ALTER TABLE conversations ADD COLUMN summary TEXT;
ALTER TABLE conversations ADD COLUMN last_activity TEXT NOT NULL DEFAULT (datetime('now'));
ALTER TABLE conversations ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
```

**Notes:**
- `summary` is deliberately nullable. Active conversations have no summary; it is only populated when the conversation is closed.
- `last_activity` defaults to `datetime('now')` for existing rows at migration time, meaning all pre-existing conversations are treated as recently active.
- `status` defaults to `'active'` for existing rows. There is no `CHECK` constraint on `status` values at the schema level; the values `'active'` and `'closed'` are enforced by application logic in `store.rs`.
- The existing `updated_at` column remains and continues to be updated alongside `last_activity`. The two columns serve different purposes: `updated_at` tracks any modification to the row, while `last_activity` specifically tracks user/assistant message activity for timeout calculations.

#### Resulting full schema

After this migration, the `conversations` table has the following columns:

| Column | Type | Constraints | Default | Source |
|--------|------|-------------|---------|--------|
| `id` | `TEXT` | `PRIMARY KEY` | -- | `001_init` |
| `channel` | `TEXT` | `NOT NULL` | -- | `001_init` |
| `sender_id` | `TEXT` | `NOT NULL` | -- | `001_init` |
| `started_at` | `TEXT` | `NOT NULL` | `datetime('now')` | `001_init` |
| `updated_at` | `TEXT` | `NOT NULL` | `datetime('now')` | `001_init` |
| `summary` | `TEXT` | -- (nullable) | `NULL` | `003_memory_enhancement` |
| `last_activity` | `TEXT` | `NOT NULL` | `datetime('now')` | `003_memory_enhancement` |
| `status` | `TEXT` | `NOT NULL` | `'active'` | `003_memory_enhancement` |

---

### CREATE INDEX: `idx_conversations_status`

```sql
CREATE INDEX IF NOT EXISTS idx_conversations_status ON conversations(status, last_activity);
```

| Property | Value |
|----------|-------|
| Name | `idx_conversations_status` |
| Table | `conversations` |
| Columns | `status`, `last_activity` (composite) |
| Unique | No |
| Condition | `IF NOT EXISTS` (idempotent) |

**Purpose:** Accelerates the two primary query patterns introduced in Phase 3:

1. **Active conversation lookup** (`get_or_create_conversation`):
   ```sql
   WHERE status = 'active' AND datetime(last_activity) > datetime('now', '-120 minutes')
   ```

2. **Idle conversation detection** (`find_idle_conversations`):
   ```sql
   WHERE status = 'active' AND datetime(last_activity) <= datetime('now', '-120 minutes')
   ```

Both queries filter on `status` first (high selectivity -- most conversations are closed) then on `last_activity`, making the composite index `(status, last_activity)` optimal.

**Existing indexes on `conversations`:**

| Index | Columns | Source |
|-------|---------|--------|
| `idx_conversations_channel_sender` | `channel`, `sender_id` | `001_init` |
| `idx_conversations_status` | `status`, `last_activity` | `003_memory_enhancement` |

---

### DROP TABLE + CREATE TABLE: `facts`

The original `facts` table from `001_init.sql` is dropped and recreated with per-user scoping.

#### Original schema (from `001_init.sql`)

```sql
CREATE TABLE IF NOT EXISTS facts (
    id                TEXT PRIMARY KEY,
    key               TEXT NOT NULL UNIQUE,
    value             TEXT NOT NULL,
    source_message_id TEXT REFERENCES messages(id),
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Key difference: `key` was globally unique (`UNIQUE` constraint on `key` alone). This meant all users shared a single namespace for fact keys -- a fact with key `"name"` could only exist once across all users.

#### Drop statement

```sql
DROP TABLE IF EXISTS facts;
```

The migration comment notes the table was "currently unused/empty" at the time of this migration, making a destructive drop safe.

#### New schema

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

#### Column definitions

| Column | Type | Constraints | Default | Purpose |
|--------|------|-------------|---------|---------|
| `id` | `TEXT` | `PRIMARY KEY` | -- | UUID v4 identifier for the fact row. Generated by `store_fact()`. |
| `sender_id` | `TEXT` | `NOT NULL` | -- | Identifies the user this fact belongs to. **New column** -- not present in `001_init`. |
| `key` | `TEXT` | `NOT NULL` | -- | The fact name (e.g., `"name"`, `"timezone"`, `"location"`). |
| `value` | `TEXT` | `NOT NULL` | -- | The fact value (e.g., `"Alice"`, `"America/New_York"`). |
| `source_message_id` | `TEXT` | `REFERENCES messages(id)` | `NULL` (nullable) | Foreign key to the message from which this fact was extracted. Optional. |
| `created_at` | `TEXT` | `NOT NULL` | `datetime('now')` | Timestamp of fact creation. |
| `updated_at` | `TEXT` | `NOT NULL` | `datetime('now')` | Timestamp of last update. Updated on upsert via `ON CONFLICT ... DO UPDATE`. |

#### Constraints

| Constraint | Type | Columns | Purpose |
|------------|------|---------|---------|
| `PRIMARY KEY` | Primary key | `id` | Row identity. |
| `UNIQUE(sender_id, key)` | Composite unique | `sender_id`, `key` | Ensures each user has at most one fact per key. Enables upsert via `ON CONFLICT(sender_id, key) DO UPDATE`. |
| `REFERENCES messages(id)` | Foreign key | `source_message_id` -> `messages(id)` | Links facts back to the message they were extracted from. Not enforced by default in SQLite (requires `PRAGMA foreign_keys = ON`). |

#### Key behavioral change

The uniqueness constraint changed from `UNIQUE(key)` to `UNIQUE(sender_id, key)`. This means:

- **Before (001):** Only one fact with key `"name"` could exist in the entire database.
- **After (003):** Each `sender_id` can have their own fact with key `"name"`. User A's `name = "Alice"` and User B's `name = "Bob"` coexist.

The upsert pattern in `store.rs` relies on this composite unique constraint:

```sql
INSERT INTO facts (id, sender_id, key, value) VALUES (?, ?, ?, ?)
ON CONFLICT(sender_id, key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
```

#### No explicit index on `facts`

The new `facts` table has no explicit `CREATE INDEX` statement. SQLite automatically creates an index for the `PRIMARY KEY` and for the `UNIQUE(sender_id, key)` constraint. The unique constraint index serves the primary query pattern:

```sql
SELECT key, value FROM facts WHERE sender_id = ? ORDER BY key
```

---

## Migration Tracking

This migration is tracked via the `_migrations` table (created in `store.rs`, not in any migration file):

```sql
CREATE TABLE IF NOT EXISTS _migrations (
    name TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

The migration is registered with name `"003_memory_enhancement"`. On subsequent runs, the migration runner checks `_migrations` and skips already-applied migrations.

A bootstrap mechanism in `Store::run_migrations()` handles databases created before migration tracking was added: if `_migrations` is empty but the `conversations` table already contains a `summary` column (indicating this migration was previously applied), all three migrations are marked as applied retroactively.

---

## Application-Level Usage

### `conversations.status` and `conversations.last_activity`

Used by the following `Store` methods:

| Method | Query Pattern | Purpose |
|--------|--------------|---------|
| `get_or_create_conversation()` | `WHERE status = 'active' AND datetime(last_activity) > datetime('now', '-120 minutes')` | Find the current active conversation for a user, or create a new one if the last one has timed out. |
| `find_idle_conversations()` | `WHERE status = 'active' AND datetime(last_activity) <= datetime('now', '-120 minutes')` | Find conversations that should be summarized and closed. Called periodically by the gateway's background task. |
| `find_all_active_conversations()` | `WHERE status = 'active'` | Find all active conversations for graceful shutdown summarization. |
| `close_conversation()` | `UPDATE SET status = 'closed', summary = ?` | Close a conversation with its AI-generated summary. |
| `close_current_conversation()` | `UPDATE SET status = 'closed' WHERE status = 'active'` | Close without a summary (used by `/forget` command). |

### `conversations.summary`

Used by the following `Store` methods:

| Method | Query Pattern | Purpose |
|--------|--------------|---------|
| `close_conversation()` | `UPDATE SET summary = ?` | Store the AI-generated summary when closing a conversation. |
| `get_recent_summaries()` | `SELECT summary, updated_at WHERE status = 'closed' AND summary IS NOT NULL` | Retrieve recent summaries to include in the enriched system prompt for context building. |
| `get_history()` | `SELECT COALESCE(summary, '(no summary)')` | Retrieve conversation history for the `/history` bot command. |

### `facts` (with `sender_id`)

Used by the following `Store` methods:

| Method | Query Pattern | Purpose |
|--------|--------------|---------|
| `store_fact()` | `INSERT ... ON CONFLICT(sender_id, key) DO UPDATE` | Upsert a fact for a specific user. |
| `get_facts()` | `SELECT key, value WHERE sender_id = ?` | Retrieve all facts for a user to include in the enriched system prompt. |
| `get_memory_stats()` | `SELECT COUNT(*) WHERE sender_id = ?` | Count facts for the `/memory` bot command. |
| `delete_facts()` | `DELETE WHERE sender_id = ?` or `DELETE WHERE sender_id = ? AND key = ?` | Remove facts for the `/forget` bot command. |

---

## Relationship to Other Migrations

| Migration | Name | What It Creates |
|-----------|------|----------------|
| `001_init.sql` | `001_init` | `conversations`, `messages`, `facts` (original) |
| `002_audit_log.sql` | `002_audit_log` | `audit_log` |
| **`003_memory_enhancement.sql`** | **`003_memory_enhancement`** | **ALTER `conversations` (+3 cols, +1 idx), DROP+CREATE `facts`** |

---

## Idempotency

- The `ALTER TABLE` statements are **not** idempotent -- running them twice on the same database would produce a "duplicate column" error. Idempotency is handled by the migration tracker (`_migrations` table), which prevents re-execution.
- `CREATE INDEX IF NOT EXISTS` is idempotent by itself.
- `DROP TABLE IF EXISTS` is idempotent by itself.
- `CREATE TABLE` (without `IF NOT EXISTS`) is **not** idempotent, but the preceding `DROP TABLE IF EXISTS` ensures the table does not exist before creation.

---

## Data Types

All columns use SQLite's flexible type system. The effective storage classes are:

| Declared Type | SQLite Storage Class | Format |
|---------------|---------------------|--------|
| `TEXT` | `TEXT` | ISO 8601 datetime strings for timestamps (e.g., `"2025-01-15 14:30:00"`), UUIDs for IDs, free-form text for content. |

SQLite does not enforce declared types strictly. The `datetime('now')` function returns an ISO 8601 string, and all timestamp comparisons in the application use SQLite's `datetime()` function for correct lexicographic ordering.
