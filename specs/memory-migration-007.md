# Specification: omega-memory/migrations/007_task_type.sql

## Path

`crates/omega-memory/migrations/007_task_type.sql`

## Purpose

Adds a `task_type` column to the `scheduled_tasks` table to distinguish between simple reminder tasks and provider-backed action tasks. Before this migration, all scheduled tasks were reminders -- they simply delivered a text message when due. With this column, tasks can be classified as either `'reminder'` (default, backward-compatible) or `'action'` (invokes the AI provider with full tool access when due).

This migration was introduced alongside the action scheduler feature in Phase 4.

## Prerequisites

- Migration `005_scheduled_tasks.sql` must have been applied (creates the `scheduled_tasks` table).

---

## Schema Changes

### ALTER TABLE: `scheduled_tasks`

```sql
ALTER TABLE scheduled_tasks ADD COLUMN task_type TEXT NOT NULL DEFAULT 'reminder';
```

### Column Description

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `task_type` | `TEXT` | `NOT NULL`, default `'reminder'` | Classifies the task behavior when due. `'reminder'` delivers a text message to the user. `'action'` invokes the AI provider with the task description as a prompt, with full tool and MCP access. |

---

## Task Type Values

| Type | Meaning | Behavior When Due |
|------|---------|-------------------|
| `reminder` | Simple text reminder (default). This is the original behavior. | Sends `"Reminder: {description}"` via the channel. |
| `action` | Provider-backed autonomous execution. | Invokes the provider with full tool/MCP access. The response is scanned for SCHEDULE, SCHEDULE_ACTION, HEARTBEAT, and LIMITATION markers. Result is delivered via the channel. |

---

## Backward Compatibility

The `DEFAULT 'reminder'` clause ensures all existing tasks automatically receive the `reminder` type. No data migration is needed -- existing rows are unaffected. The scheduler loop handles both types: it checks `task_type` and branches to either message delivery (reminder) or provider invocation (action).

---

## Migration Tracking

This migration is registered with name `"007_task_type"` in the `_migrations` table. The migration runner in `Store::run_migrations()` checks for this name and skips execution if already applied.

**Migration definitions (compile-time embedded):**
```rust
("007_task_type", include_str!("../migrations/007_task_type.sql"))
```

---

## Application-Level Usage

### `Store::create_task()`

Now accepts a `task_type: &str` parameter (7th argument) that is inserted into the `task_type` column. Gateway passes `"reminder"` for `SCHEDULE:` markers and `"action"` for `SCHEDULE_ACTION:` markers.

### `Store::get_due_tasks()`

Returns a 6-tuple with `task_type` as the 6th element: `(id, channel, reply_target, description, repeat, task_type)`. The scheduler loop uses this to branch between reminder delivery and action execution.

### `Store::get_tasks_for_sender()`

Returns a 5-tuple with `task_type` as the 5th element: `(id, description, due_at, repeat, task_type)`. The `/tasks` command uses this to show an `[action]` badge for action tasks.

---

## Relationship to Other Migrations

| Migration | Name | What It Creates |
|-----------|------|----------------|
| `001_init.sql` | `001_init` | `conversations`, `messages`, `facts` (original) |
| `002_audit_log.sql` | `002_audit_log` | `audit_log` |
| `003_memory_enhancement.sql` | `003_memory_enhancement` | ALTER `conversations` (+3 cols, +1 idx), DROP+CREATE `facts` |
| `004_fts5_recall.sql` | `004_fts5_recall` | `messages_fts` virtual table, 3 sync triggers, backfill |
| `005_scheduled_tasks.sql` | `005_scheduled_tasks` | `scheduled_tasks` table, 2 indexes |
| `006_limitations.sql` | `006_limitations` | `limitations` table, 1 unique index |
| **`007_task_type.sql`** | **`007_task_type`** | **ALTER `scheduled_tasks` (+1 col: `task_type`)** |

---

## Idempotency

- `ALTER TABLE ... ADD COLUMN` is **not** idempotent in SQLite -- running it twice would produce an error ("duplicate column name"). However, the migration tracker prevents re-execution. The `_migrations` table records `007_task_type` after the first successful run, and subsequent calls to `run_migrations()` skip it.

---

## Performance Considerations

- **No new index:** The `task_type` column does not have its own index. The existing `idx_scheduled_tasks_due` index on `(status, due_at)` remains the primary query path for `get_due_tasks()`. Since `task_type` is only read after the row is fetched (to decide reminder vs action), a dedicated index is unnecessary.
- **Minimal storage overhead:** Adding a `TEXT` column with a short default value (`'reminder'`) adds negligible storage per row.
