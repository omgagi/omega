# Limitations Table (Migration 006)

## Path

`backend/crates/omega-memory/migrations/006_limitations.sql`

## What This Migration Does

Migration 006 gives Omega self-awareness about its own gaps. It creates a `limitations` table where the agent can record capability limitations it discovers during operation -- things it cannot do, constraints it encounters, and proposed solutions. This is part of Omega's autonomous self-monitoring system.

When Omega encounters something it cannot handle (e.g., "I can't access your calendar directly"), it can store this as a limitation via the `LIMITATION:` response marker. The heartbeat loop can also query open limitations during self-audit checks.

## Migration Sequence

| Order | File | What It Creates |
|-------|------|----------------|
| 1 | `001_init.sql` | Core tables: `conversations`, `messages`, `facts` |
| 2 | `002_audit_log.sql` | Audit trail: `audit_log` |
| 3 | `003_memory_enhancement.sql` | Conversation lifecycle + per-user facts |
| 4 | `004_fts5_recall.sql` | FTS5 search index + auto-sync triggers |
| 5 | `005_scheduled_tasks.sql` | Task queue: `scheduled_tasks` table + indexes |
| **6** | **`006_limitations.sql`** | **Self-introspection: `limitations` table + unique title index** |

Migrations run automatically when the memory store initializes. Each migration runs exactly once.

## The limitations Table

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
CREATE UNIQUE INDEX IF NOT EXISTS idx_limitations_title ON limitations(title COLLATE NOCASE);
```

### Column Explanations

| Column | What It Stores |
|--------|---------------|
| `id` | UUID v4 generated at insertion time. |
| `title` | Brief, unique title for the limitation (e.g., "Cannot access Google Calendar"). Used for deduplication -- case-insensitive via the unique index. |
| `description` | Detailed explanation of the limitation: what is blocked, why, and the user impact. |
| `proposed_plan` | Optional solution or workaround plan. Empty string if no plan has been proposed yet. |
| `status` | Current state: `'open'` (active) or `'resolved'` (addressed). |
| `created_at` | When the limitation was first identified. Auto-populated by SQLite. |
| `resolved_at` | When the limitation was resolved. NULL until status changes to `'resolved'`. |

### Unique Index

The `idx_limitations_title` index enforces case-insensitive title uniqueness. This means "Cannot Execute Code" and "cannot execute code" are treated as the same limitation, preventing duplicates from different phrasings.

## How Omega Uses This

### Storing a Limitation

When the AI provider includes a `LIMITATION:` marker in its response, the gateway extracts it and calls:

```rust
// Returns true if new, false if already existed (dedup by title)
let is_new = store.store_limitation(
    "Cannot access Google Calendar",
    "No direct Google Calendar API integration. User must relay events manually.",
    "Integrate Google Calendar API via MCP server"
).await?;
```

The `INSERT OR IGNORE` pattern means the same limitation can be reported multiple times without creating duplicates.

### Querying Open Limitations

During heartbeat self-audits or on-demand reporting:

```rust
let limitations = store.get_open_limitations().await?;
// Returns Vec<(title, description, proposed_plan)> for all open limitations
```

## Backward Compatibility

This is an additive migration -- it only creates a new table and index. No existing tables are modified.
