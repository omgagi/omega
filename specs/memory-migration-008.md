# Specification: omega-memory/migrations/008_user_aliases.sql

## Path

`backend/crates/omega-memory/migrations/008_user_aliases.sql`

## Purpose

Creates the `user_aliases` table for cross-channel user identity linking. Before this migration, each messaging channel maintained a separate user identity -- a user interacting via Telegram (ID `842277204`) and WhatsApp (phone `5511999887766`) would be treated as two different people with separate facts, conversations, and preferences.

This migration enables Omega to recognize that the same person is interacting across multiple channels. When a new user appears on WhatsApp who is the same person as an existing Telegram user, Omega creates an alias mapping. All subsequent fact lookups and memory queries resolve the alias to the canonical (first-registered) sender ID.

## Prerequisites

- Migration `001_init.sql` must have been applied (creates the base schema with `facts` table).
- The `facts` table must have a `sender_id` column (established by migration 003).
- No dependency on other tables -- `user_aliases` is independent.

---

## Schema Changes

### CREATE TABLE: `user_aliases`

```sql
CREATE TABLE IF NOT EXISTS user_aliases (
    alias_sender_id     TEXT PRIMARY KEY,
    canonical_sender_id TEXT NOT NULL
);
```

### Column Descriptions

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `alias_sender_id` | `TEXT` | `PRIMARY KEY` | The secondary sender ID (e.g., WhatsApp phone number). Each alias maps to exactly one canonical ID. |
| `canonical_sender_id` | `TEXT` | `NOT NULL` | The primary sender ID that this alias resolves to (e.g., the Telegram user ID registered first). All memory lookups use this ID. |

---

## Design Decisions

### No Foreign Key to `facts`

The `canonical_sender_id` is not declared as a foreign key to any table. This is intentional -- sender IDs appear across multiple tables (`facts`, `conversations`, `messages`, `scheduled_tasks`), and the alias table is a lookup layer that sits above all of them. Referential integrity is enforced at the application level by `Store::resolve_sender_id()`.

### Simple 1:1 Mapping

The schema uses a simple alias -> canonical mapping rather than a user identity table. This keeps the migration minimal and avoids introducing a new "user" concept into a system that has always identified users by their platform sender ID. Multiple aliases can point to the same canonical ID (e.g., WhatsApp phone + future Discord ID -> Telegram ID).

### No Indexes Beyond Primary Key

The primary key on `alias_sender_id` provides the only index needed. The canonical direction (finding all aliases for a canonical ID) is not a hot path -- it would only be needed for administrative queries.

---

## Migration Tracking

This migration is registered with name `"008_user_aliases"` in the `_migrations` table. The migration runner in `Store::run_migrations()` checks for this name and skips execution if already applied.

**Migration definitions (compile-time embedded):**
```rust
("008_user_aliases", include_str!("../../migrations/008_user_aliases.sql"))
```

---

## Application-Level Usage

### `Store::resolve_sender_id()`

Resolves a sender_id to its canonical form. Returns the original ID if no alias exists.

```rust
pub async fn resolve_sender_id(&self, sender_id: &str) -> Result<String, OmegaError>
```

**Parameters:**
- `sender_id`: The sender ID to resolve (may be an alias or already canonical).

**Returns:** The canonical sender_id if an alias exists, otherwise the original sender_id.

**Called by:** Gateway message pipeline, before any memory operations (fact lookup, context building, etc.).

### `Store::create_alias()`

Creates a new alias mapping. Uses `INSERT OR IGNORE` for idempotency.

```rust
pub async fn create_alias(&self, alias_id: &str, canonical_id: &str) -> Result<(), OmegaError>
```

**Parameters:**
- `alias_id`: The secondary sender ID to register as an alias.
- `canonical_id`: The primary sender ID this alias resolves to.

**Called by:** Gateway when a new WhatsApp user is detected and matched to an existing Telegram user via `find_canonical_user()`.

### `Store::find_canonical_user()`

Finds an existing welcomed user different from the given sender_id.

```rust
pub async fn find_canonical_user(&self, exclude_sender_id: &str) -> Result<Option<String>, OmegaError>
```

**Parameters:**
- `exclude_sender_id`: The sender_id to exclude from the search (the new user being registered).

**Returns:** `Some(sender_id)` of an existing welcomed user, or `None` if no other users exist.

**Called by:** Gateway during new user registration, to automatically create cross-channel aliases.

---

## Relationship to Other Migrations

| Migration | Name | What It Creates |
|-----------|------|----------------|
| `001_init.sql` | `001_init` | `conversations`, `messages`, `facts` (original) |
| `002_audit_log.sql` | `002_audit_log` | `audit_log` |
| `003_memory_enhancement.sql` | `003_memory_enhancement` | ALTER `conversations` (+3 cols), DROP+CREATE `facts` with sender_id |
| `004_fts5_recall.sql` | `004_fts5_recall` | `messages_fts` virtual table, triggers |
| `005_scheduled_tasks.sql` | `005_scheduled_tasks` | `scheduled_tasks` table |
| `006_limitations.sql` | `006_limitations` | `limitations` table |
| `007_task_type.sql` | `007_task_type` | `task_type` column on `scheduled_tasks` |
| **`008_user_aliases.sql`** | **`008_user_aliases`** | **`user_aliases` table** |

---

## Idempotency

- `CREATE TABLE IF NOT EXISTS` is idempotent.
- The entire migration file is idempotent and can safely be re-run, though the migration tracker prevents re-execution.
