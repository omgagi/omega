# Memory Enhancement Migration (003)

## Path

`crates/omega-memory/migrations/003_memory_enhancement.sql`

## What This Migration Does

Migration 003 upgrades Omega's memory system to support two features introduced in Phase 3:

1. **Conversation boundaries** -- conversations now have a lifecycle (`active` / `closed`), a `last_activity` timestamp for idle detection, and an optional `summary` that captures what was discussed after the conversation ends.
2. **Per-user facts** -- the `facts` table is rebuilt so that each user has their own set of facts, rather than sharing a single global namespace.

These changes enable the gateway to automatically detect idle conversations, summarize them with an AI call, store the summary for future context, and build personalized system prompts that reference what the user has discussed in past sessions.

## Migration Sequence

This is the third of three migrations that define Omega's database schema:

| Order | File | What It Creates |
|-------|------|----------------|
| 1 | `001_init.sql` | Core tables: `conversations`, `messages`, `facts` |
| 2 | `002_audit_log.sql` | Audit trail: `audit_log` |
| **3** | **`003_memory_enhancement.sql`** | **Conversation lifecycle + per-user facts** |

Migrations run automatically when the memory store initializes. Each migration runs exactly once -- the system tracks which migrations have been applied in a `_migrations` table and skips those that have already run.

## What Changed in `conversations`

Three new columns were added to the `conversations` table:

| Column | What It Stores | Example Value |
|--------|---------------|---------------|
| `summary` | An AI-generated summary of the conversation, written when the conversation closes. | `"User asked about Rust async patterns. Omega provided tokio examples."` |
| `last_activity` | When the last message was sent or received in this conversation. | `"2025-06-15 14:30:00"` |
| `status` | Whether the conversation is still going (`active`) or has been closed (`closed`). | `"active"` |

A new index on `(status, last_activity)` speeds up the queries that find active and idle conversations.

### How conversations work after this migration

Before migration 003, conversations had no concept of "ending." Every conversation existed indefinitely, and there was no mechanism to detect idle sessions or summarize past interactions.

After migration 003, conversations follow this lifecycle:

```
[New message arrives]
       |
       v
  Is there an active conversation for this user
  that had activity in the last 2 hours?
       |               |
      YES              NO
       |               |
  Continue it     Create a new one
       |
       v
  Update last_activity
       |
       ... (user keeps chatting) ...
       |
  2 hours of silence
       |
       v
  Gateway detects idle conversation
       |
       v
  AI generates a summary of the conversation
       |
       v
  Conversation is closed with the summary stored
       |
       v
  Next message from this user starts a fresh conversation
```

Closed conversations and their summaries are available to enrich future interactions. When building context for a new message, the memory store includes recent summaries in the system prompt so the AI can naturally reference past discussions.

### Where summaries appear

Summaries show up in two places:

1. **System prompt enrichment** -- The three most recent closed conversation summaries are included in the system prompt sent to the AI provider. This lets the AI say things like "Earlier you asked about Rust async patterns" without having the full conversation history.

2. **The `/history` command** -- Users can run `/history` in Telegram to see a list of their past conversations with timestamps and summaries.

## What Changed in `facts`

The `facts` table was dropped and recreated with a new structure. The migration notes that the table was empty at the time, so no data was lost.

### Before (migration 001)

Facts were globally unique by key:

```
facts
  key = "name"  ->  value = "Alice"    (only one "name" fact in the whole database)
```

This meant that if two users both had a fact called `"name"`, they would conflict.

### After (migration 003)

Facts are scoped to each user via `sender_id`:

```
facts
  sender_id = "user_123", key = "name"  ->  value = "Alice"
  sender_id = "user_456", key = "name"  ->  value = "Bob"
```

Each user now has their own independent set of facts. The uniqueness constraint is on `(sender_id, key)` instead of just `(key)`.

### How facts are used

Facts are key-value pairs that Omega learns about a user from conversations. They are extracted automatically by the gateway when the AI mentions user-specific information (name, timezone, preferences, etc.).

Facts appear in the system prompt so the AI can personalize its responses:

```
Known facts about this user:
- name: Alice
- timezone: America/New_York
- preferred_language: Spanish
```

Users can manage their facts with bot commands:
- `/facts` -- view all stored facts
- `/forget name` -- delete a specific fact
- `/forget all` -- delete all facts and close the current conversation

## Schema Overview After All Migrations

After all three migrations have run, the database has the following tables:

### `conversations`

Tracks conversation sessions between users and Omega.

| Column | Purpose |
|--------|---------|
| `id` | UUID primary key |
| `channel` | Where the conversation happened (e.g., `"telegram"`) |
| `sender_id` | The user's identifier on that channel |
| `started_at` | When the conversation began |
| `updated_at` | When the row was last modified |
| `summary` | AI-generated summary (null while active, populated on close) |
| `last_activity` | When the last message was sent/received |
| `status` | `"active"` or `"closed"` |

### `messages`

Individual messages within conversations.

| Column | Purpose |
|--------|---------|
| `id` | UUID primary key |
| `conversation_id` | Links to `conversations.id` |
| `role` | `"user"` or `"assistant"` |
| `content` | The message text |
| `timestamp` | When the message was sent |
| `metadata_json` | Optional JSON metadata (provider, model, etc.) |

### `facts`

Per-user key-value facts that Omega has learned.

| Column | Purpose |
|--------|---------|
| `id` | UUID primary key |
| `sender_id` | The user this fact belongs to |
| `key` | Fact name (e.g., `"name"`, `"timezone"`) |
| `value` | Fact value (e.g., `"Alice"`, `"America/New_York"`) |
| `source_message_id` | Optional link to the message this fact was extracted from |
| `created_at` | When the fact was first stored |
| `updated_at` | When the fact was last updated |

### `audit_log`

Record of every interaction through Omega (created in migration 002, not modified by this migration).

### `_migrations`

Internal tracking table that records which migrations have been applied. Not created by any migration file -- it is created by the Rust migration runner in `store.rs`.

## Indexes

After all migrations, the following indexes exist:

| Index | Table | Columns | Created By |
|-------|-------|---------|------------|
| `idx_conversations_channel_sender` | `conversations` | `channel`, `sender_id` | `001_init` |
| `idx_conversations_status` | `conversations` | `status`, `last_activity` | `003_memory_enhancement` |
| `idx_messages_conversation` | `messages` | `conversation_id`, `timestamp` | `001_init` |
| `idx_audit_log_timestamp` | `audit_log` | `timestamp` | `002_audit_log` |
| `idx_audit_log_sender` | `audit_log` | `channel`, `sender_id` | `002_audit_log` |
| (automatic) | `facts` | `sender_id`, `key` | `003_memory_enhancement` (from `UNIQUE` constraint) |

## Important Notes for Developers

### The migration is destructive for `facts`

The `DROP TABLE IF EXISTS facts` statement destroys any existing data in the facts table. This was safe at the time of the migration because the table was unused, but future migrations should use `ALTER TABLE` or data-preserving approaches if the table contains production data.

### Conversation timeout is application-level

The 2-hour idle conversation timeout is not defined in the schema. It is a constant (`CONVERSATION_TIMEOUT_MINUTES = 120`) in `store.rs`. Changing it requires a code change, not a migration.

### No CHECK constraint on `status`

Unlike the `messages.role` column (which has `CHECK (role IN ('user', 'assistant'))`), the `conversations.status` column has no schema-level check constraint. The values `"active"` and `"closed"` are enforced purely by application code. If you add new status values in the future, no migration is needed, but be aware that queries filtering on specific status strings may need updating.

### SQLite type flexibility

All columns use `TEXT` type. SQLite does not enforce strict typing, so timestamps are stored as ISO 8601 strings (e.g., `"2025-06-15 14:30:00"`) and compared using SQLite's `datetime()` function. UUIDs are stored as their string representation.

### Foreign keys are advisory

The `source_message_id REFERENCES messages(id)` foreign key on the `facts` table is declared but not enforced by default in SQLite. Enforcement would require `PRAGMA foreign_keys = ON`, which is not set in the Omega connection options. This means a fact could reference a non-existent message without causing a database error.
