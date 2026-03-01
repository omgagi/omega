# Project Sessions Table (Migration 012)

## Path

`backend/crates/omega-memory/migrations/012_project_sessions.sql`

## What This Migration Does

Migration 012 moves CLI session management from an in-memory `HashMap` to SQLite, and scopes both sessions and conversations to projects. Before this migration:

- CLI sessions (the Claude Code `--resume` session IDs) were stored in a `HashMap<String, String>` in the gateway. Sessions were lost on every restart.
- Conversations had no project awareness -- all messages for a user went into the same conversation regardless of which project was active.

Now:
- **Sessions survive restarts** -- they persist in the `project_sessions` SQLite table.
- **Sessions are project-scoped** -- switching projects preserves the previous project's CLI session. Returning to a project resumes where you left off.
- **Conversations are project-scoped** -- messages in project "omega-trader" stay separate from general conversations.

## Migration Sequence

| Order | File | What It Creates |
|-------|------|----------------|
| 1-10 | (previous) | Core schema, audit, tasks, outcomes, etc. |
| 11 | `011_project_learning.sql` | Project columns on outcomes, lessons, scheduled_tasks |
| **12** | **`012_project_sessions.sql`** | **`project_sessions` table + project column on conversations** |

Migrations run automatically when the memory store initializes. Each migration runs exactly once.

## The project_sessions Table

```sql
CREATE TABLE IF NOT EXISTS project_sessions (
    id              TEXT PRIMARY KEY,
    channel         TEXT NOT NULL,
    sender_id       TEXT NOT NULL,
    project         TEXT NOT NULL DEFAULT '',
    session_id      TEXT NOT NULL,
    parent_project  TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(channel, sender_id, project)
);
```

### Column Explanations

| Column | What It Stores |
|--------|---------------|
| `id` | UUID v4 generated at insertion time. |
| `channel` | The messaging channel (e.g., `"telegram"`, `"whatsapp"`). |
| `sender_id` | The user this session belongs to. |
| `project` | Project scope. Empty string (`""`) means general (no project). |
| `session_id` | The Claude Code CLI session ID used for `--resume`. |
| `parent_project` | Reserved for future use (project hierarchy). Currently unused. |
| `created_at` | When the session was first created. |
| `updated_at` | When the session was last updated (refreshed on each upsert). |

### Unique Constraint

`UNIQUE(channel, sender_id, project)` ensures one session per channel+user+project combination. The `store_session()` method uses `ON CONFLICT DO UPDATE` to refresh the session ID on subsequent calls.

## Conversation Project Column

```sql
ALTER TABLE conversations ADD COLUMN project TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS idx_conversations_project
    ON conversations(channel, sender_id, project, status, last_activity);
```

All existing conversations receive `project = ''` (general scope). The new composite index optimizes the project-scoped conversation queries used by `get_or_create_conversation()`.

## Store Methods

| Method | What It Does |
|--------|-------------|
| `store_session(channel, sender_id, project, session_id)` | Upsert a CLI session. If one exists for the same key, updates the session_id. |
| `get_session(channel, sender_id, project) -> Option<String>` | Look up the CLI session_id for a specific project context. |
| `clear_session(channel, sender_id, project)` | Delete the session for a specific project. |
| `clear_all_sessions_for_sender(sender_id)` | Delete all sessions for a user (used by `/forget`). |

### Updated Method Signatures

Several existing methods gained a `project` parameter with this migration:

| Method | Change |
|--------|--------|
| `get_or_create_conversation` | Added `project: &str` param |
| `close_current_conversation` | Added `project: &str` param |
| `find_idle_conversations` | Now returns `(id, channel, sender_id, project)` tuples |
| `find_all_active_conversations` | Now returns `(id, channel, sender_id, project)` tuples |
| `store_exchange` | Added `project: &str` param |

## Project Scope Convention

- **Empty string (`""`)** = general OMEGA scope (no active project). This is the default.
- **Non-empty string** = project-scoped (e.g., `"omega-trader"`).
- The convention is consistent across all project-scoped tables: `outcomes`, `lessons`, `scheduled_tasks`, `conversations`, and `project_sessions`.

## Backward Compatibility

- `project_sessions` is a new table (`CREATE TABLE IF NOT EXISTS`), no existing data affected.
- `ALTER TABLE conversations ADD COLUMN ... DEFAULT ''` preserves all existing conversations with general scope.
- Methods that previously had no `project` parameter now accept one, but callers pass `""` for general scope -- identical behavior to pre-migration.
