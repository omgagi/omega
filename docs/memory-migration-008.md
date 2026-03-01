# User Aliases Table (Migration 008)

## Path

`backend/crates/omega-memory/migrations/008_user_aliases.sql`

## What This Migration Does

Migration 008 enables cross-channel user identity linking. Before this migration, a user chatting via Telegram (ID `842277204`) and the same person chatting via WhatsApp (phone `5511999887766`) would be treated as two separate users -- with separate facts, conversation history, and preferences.

Now, Omega can recognize that these are the same person by creating an alias mapping in the `user_aliases` table. All memory operations resolve aliases to the canonical (original) sender ID, so facts learned via Telegram are available when the same user chats via WhatsApp.

## Migration Sequence

| Order | File | What It Creates |
|-------|------|----------------|
| 1 | `001_init.sql` | Core tables: `conversations`, `messages`, `facts` |
| 2 | `002_audit_log.sql` | Audit trail: `audit_log` |
| 3 | `003_memory_enhancement.sql` | Conversation lifecycle + per-user facts |
| 4 | `004_fts5_recall.sql` | FTS5 search index + auto-sync triggers |
| 5 | `005_scheduled_tasks.sql` | Task queue: `scheduled_tasks` table + indexes |
| 6 | `006_limitations.sql` | Self-introspection: `limitations` table |
| 7 | `007_task_type.sql` | Task type: `task_type` column on `scheduled_tasks` |
| **8** | **`008_user_aliases.sql`** | **Cross-channel aliases: `user_aliases` table** |

Migrations run automatically when the memory store initializes. Each migration runs exactly once.

## The user_aliases Table

```sql
CREATE TABLE IF NOT EXISTS user_aliases (
    alias_sender_id     TEXT PRIMARY KEY,
    canonical_sender_id TEXT NOT NULL
);
```

### Column Explanations

| Column | What It Stores |
|--------|---------------|
| `alias_sender_id` | The secondary sender ID (e.g., WhatsApp phone number `5511999887766`). This is the primary key -- each alias can only point to one canonical user. |
| `canonical_sender_id` | The primary sender ID that this alias resolves to (e.g., Telegram user ID `842277204`). This is the ID used for all memory lookups. |

## How It Works

### Automatic Alias Creation

When a new user first interacts with Omega via a second channel (e.g., WhatsApp after already using Telegram), the gateway:

1. Calls `store.find_canonical_user(new_sender_id)` to check if any existing welcomed user exists.
2. If found, calls `store.create_alias(new_sender_id, canonical_sender_id)` to link the identities.
3. From this point on, all memory operations for the new sender ID are resolved to the canonical ID.

### Resolution in the Gateway Pipeline

Every incoming message goes through alias resolution early in the pipeline:

```rust
// Resolve alias before any memory operations
let canonical_id = store.resolve_sender_id(&incoming.sender_id).await?;
// Use canonical_id for all subsequent operations
```

If no alias exists, the original sender ID is returned unchanged. This makes the system transparent to existing single-channel users.

### Example

```
Telegram user 842277204 ("Alice") sends "remember my timezone is UTC-3"
  -> store.store_fact("842277204", "timezone", "UTC-3")

WhatsApp user 5511999887766 ("Alice" from her phone) sends "what's my timezone?"
  -> store.resolve_sender_id("5511999887766") => "842277204" (alias)
  -> store.get_facts("842277204") => [("timezone", "UTC-3")]
  -> Omega responds: "Your timezone is UTC-3"
```

## Store Methods

| Method | What It Does |
|--------|-------------|
| `resolve_sender_id(sender_id)` | Look up the canonical ID for an alias. Returns the original ID if no alias exists. |
| `create_alias(alias_id, canonical_id)` | Create a new alias mapping. Uses `INSERT OR IGNORE` for idempotency. |
| `find_canonical_user(exclude_sender_id)` | Find any existing welcomed user who is different from the given sender. Used to auto-create aliases for new channel users. |

## Backward Compatibility

This is an additive migration -- it only creates a new table. No existing tables are modified. Users who only interact via a single channel are completely unaffected. The `resolve_sender_id()` method returns the original ID when no alias exists, so the system is backward-compatible by default.
