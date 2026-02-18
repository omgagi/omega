# Task Type Column (Migration 007)

## Path

`crates/omega-memory/migrations/007_task_type.sql`

## What This Migration Does

Migration 007 adds a `task_type` column to the `scheduled_tasks` table. Before this migration, all scheduled tasks were reminders -- they delivered a text message when due. Now, tasks can also be **actions** -- when an action task comes due, Omega invokes the AI provider with full tool and MCP access to autonomously execute the task.

This is a single `ALTER TABLE` statement. Existing tasks are unaffected -- they automatically receive the default type `'reminder'`.

## Migration Sequence

| Order | File | What It Creates |
|-------|------|----------------|
| 1 | `001_init.sql` | Core tables: `conversations`, `messages`, `facts` |
| 2 | `002_audit_log.sql` | Audit trail: `audit_log` |
| 3 | `003_memory_enhancement.sql` | Conversation lifecycle + per-user facts |
| 4 | `004_fts5_recall.sql` | FTS5 search index + auto-sync triggers |
| 5 | `005_scheduled_tasks.sql` | Task queue: `scheduled_tasks` table + indexes |
| 6 | `006_limitations.sql` | Self-introspection: `limitations` table + unique index |
| **7** | **`007_task_type.sql`** | **Task type: `task_type` column on `scheduled_tasks`** |

Migrations run automatically when the memory store initializes. Each migration runs exactly once.

## The Change

```sql
ALTER TABLE scheduled_tasks ADD COLUMN task_type TEXT NOT NULL DEFAULT 'reminder';
```

### Column Explanation

| Column | What It Stores |
|--------|---------------|
| `task_type` | How the task behaves when due. `"reminder"` sends a text message (the original behavior). `"action"` invokes the AI provider with full tool access to autonomously execute the task description. |

## Task Types

| Type | What Happens When Due | Created By |
|------|----------------------|------------|
| `reminder` | Sends `"Reminder: {description}"` to you | `SCHEDULE:` marker |
| `action` | Invokes the AI provider with the description as a prompt, with full tool/MCP access | `SCHEDULE_ACTION:` marker |

### How Action Tasks Work

When the scheduler loop finds a due action task, it does not just send a message. Instead:

1. The task description is sent to the AI provider as a prompt.
2. The provider has full tool access and MCP server configuration.
3. The provider's response is processed for all markers (SCHEDULE, SCHEDULE_ACTION, HEARTBEAT, LIMITATION).
4. The final response (with markers stripped) is delivered to you through the channel.

This enables Omega to schedule autonomous follow-up work -- checking deployments, verifying services, running health checks -- without needing you to initiate the request.

## Backward Compatibility

All existing tasks automatically get `task_type = 'reminder'` via the `DEFAULT` clause. No data migration is needed. The scheduler continues to deliver reminders exactly as before. Action task behavior is only triggered for tasks explicitly created with `task_type = 'action'`.

## Schema Overview After All Migrations

After migration 007, the `scheduled_tasks` table has this schema:

| Column | Type | Created By |
|--------|------|------------|
| `id` | TEXT PRIMARY KEY | 005 |
| `channel` | TEXT NOT NULL | 005 |
| `sender_id` | TEXT NOT NULL | 005 |
| `reply_target` | TEXT NOT NULL | 005 |
| `description` | TEXT NOT NULL | 005 |
| `due_at` | TEXT NOT NULL | 005 |
| `repeat` | TEXT | 005 |
| `status` | TEXT NOT NULL DEFAULT 'pending' | 005 |
| `created_at` | TEXT NOT NULL DEFAULT datetime('now') | 005 |
| `delivered_at` | TEXT | 005 |
| `task_type` | TEXT NOT NULL DEFAULT 'reminder' | **007** |
