# Technical Specification: `omega-memory/src/lib.rs`

## File

| Field | Value |
|-------|-------|
| Path | `crates/omega-memory/src/lib.rs` |
| Crate | `omega-memory` |
| Role | Crate root -- declares modules and re-exports their primary types |

## Purpose

`lib.rs` is the entry point for the `omega-memory` crate. It serves three purposes:

1. Declare the two submodules that compose the persistent memory system.
2. Re-export the primary types (`Store` and `AuditLogger`) at crate root for ergonomic imports.
3. Carry the crate-level doc comment describing what `omega-memory` does.

The file itself contains no types, traits, functions, or implementations. Its entire job is module wiring, re-exports, and documentation.

## Module Doc Comment

```rust
//! # omega-memory
//!
//! Persistent memory system for Omega (SQLite-backed).
```

## Module Declarations

| Module | Visibility | Status | Description |
|--------|-----------|--------|-------------|
| `audit` | `pub mod` (public) | Implemented | Audit log subsystem. Records every interaction through Omega to an `audit_log` SQLite table. |
| `store` | `pub mod` (public) | Implemented | Persistent conversation memory. Manages conversations, messages, facts, and context building via SQLite. |

Both modules are fully implemented and publicly accessible.

## Re-exports

```rust
pub use audit::AuditLogger;
pub use store::Store;
```

Two `pub use` re-exports bring the primary types to the crate root. This allows downstream crates to import either way:

```rust
// Via re-export (preferred):
use omega_memory::{Store, AuditLogger};

// Via module path (also valid):
use omega_memory::store::Store;
use omega_memory::audit::AuditLogger;
```

---

## Module Details

### `store` Module

**File:** `crates/omega-memory/src/store.rs`

**Module Doc Comment:**
```rust
//! SQLite-backed persistent memory store.
```

**Constants:**

| Constant | Type | Value | Purpose |
|----------|------|-------|---------|
| `CONVERSATION_TIMEOUT_MINUTES` | `i64` | `120` | Idle threshold (minutes) before a conversation is considered expired |

**Public Struct: `Store`**

| Derives | Fields (private) |
|---------|-----------------|
| `Clone` | `pool: SqlitePool`, `max_context_messages: usize` |

**Public Methods on `Store`:**

| Method | Signature | Purpose |
|--------|-----------|---------|
| `new` | `pub async fn new(config: &MemoryConfig) -> Result<Self, OmegaError>` | Create a store: expand `~` in db_path, create parent dirs, open SQLite with WAL journal mode, run migrations |
| `pool` | `pub fn pool(&self) -> &SqlitePool` | Return a reference to the underlying connection pool (used by `AuditLogger`) |
| `find_idle_conversations` | `pub async fn find_idle_conversations(&self) -> Result<Vec<(String, String, String)>, OmegaError>` | Find active conversations idle beyond the timeout; returns `(id, channel, sender_id)` tuples |
| `find_all_active_conversations` | `pub async fn find_all_active_conversations(&self) -> Result<Vec<(String, String, String)>, OmegaError>` | Find all active conversations regardless of idle time (used for shutdown); returns `(id, channel, sender_id)` tuples |
| `get_conversation_messages` | `pub async fn get_conversation_messages(&self, conversation_id: &str) -> Result<Vec<(String, String)>, OmegaError>` | Get all messages for a conversation in chronological order; returns `(role, content)` tuples |
| `close_conversation` | `pub async fn close_conversation(&self, conversation_id: &str, summary: &str) -> Result<(), OmegaError>` | Set conversation status to `closed` and store a summary |
| `store_fact` | `pub async fn store_fact(&self, sender_id: &str, key: &str, value: &str) -> Result<(), OmegaError>` | Upsert a fact by `(sender_id, key)` -- inserts or updates value |
| `get_facts` | `pub async fn get_facts(&self, sender_id: &str) -> Result<Vec<(String, String)>, OmegaError>` | Get all facts for a sender; returns `(key, value)` tuples ordered by key |
| `get_recent_summaries` | `pub async fn get_recent_summaries(&self, channel: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String)>, OmegaError>` | Get recent closed conversation summaries; returns `(summary, updated_at)` tuples ordered newest-first |
| `get_memory_stats` | `pub async fn get_memory_stats(&self, sender_id: &str) -> Result<(i64, i64, i64), OmegaError>` | Count conversations, messages, and facts for a sender; returns `(conv_count, msg_count, fact_count)` |
| `get_history` | `pub async fn get_history(&self, channel: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String)>, OmegaError>` | Get conversation summaries with timestamps for a sender; returns `(summary_or_fallback, updated_at)` tuples |
| `delete_facts` | `pub async fn delete_facts(&self, sender_id: &str, key: Option<&str>) -> Result<u64, OmegaError>` | Delete facts: all facts for a sender if `key` is `None`, or a specific fact if `key` is `Some`; returns rows deleted |
| `close_current_conversation` | `pub async fn close_current_conversation(&self, channel: &str, sender_id: &str) -> Result<bool, OmegaError>` | Close the active conversation for a sender on a channel (for `/forget` command); returns whether a conversation was closed |
| `db_size` | `pub async fn db_size(&self) -> Result<u64, OmegaError>` | Get database file size in bytes via `PRAGMA page_count * page_size` |
| `build_context` | `pub async fn build_context(&self, incoming: &IncomingMessage, base_system_prompt: &str) -> Result<Context, OmegaError>` | Build a full `Context` for the AI provider: get/create conversation, load history, fetch facts and summaries, generate enriched system prompt using the provided base prompt |
| `store_exchange` | `pub async fn store_exchange(&self, incoming: &IncomingMessage, response: &OutgoingMessage) -> Result<(), OmegaError>` | Store both the user message and assistant response in the conversation |

**Private Methods on `Store`:**

| Method | Signature | Purpose |
|--------|-----------|---------|
| `run_migrations` | `async fn run_migrations(pool: &SqlitePool) -> Result<(), OmegaError>` | Run SQL migrations with idempotent tracking via `_migrations` table; handles bootstrap from pre-tracking databases |
| `get_or_create_conversation` | `async fn get_or_create_conversation(&self, channel: &str, sender_id: &str) -> Result<String, OmegaError>` | Find an active, non-idle conversation for a channel+sender or create a new one; returns conversation ID |

**Private Functions (module-level):**

| Function | Signature | Purpose |
|----------|-----------|---------|
| `build_system_prompt` | `fn build_system_prompt(base_rules: &str, facts: &[(String, String)], summaries: &[(String, String)], recall: &[(String, String, String)], pending_tasks: &[(String, String, String, Option<String>)], language: &str) -> String` | Generate a dynamic system prompt by combining an externalized base prompt with user facts, conversation summaries, recall, pending tasks, and language directive |

**Migration System:**

The `run_migrations` method implements a custom migration tracker:

1. Creates a `_migrations` table with columns `name TEXT PRIMARY KEY` and `applied_at TEXT`.
2. Bootstrap logic: if `_migrations` is empty but the schema already has Phase 3 columns (e.g. `summary` on `conversations`), marks migrations `001_init`, `002_audit_log`, and `003_memory_enhancement` as already applied.
3. For each migration in the ordered list, checks `_migrations` for prior application; if not found, executes the SQL and records it.

**Migrations:**

| Migration | File | Tables/Columns Affected |
|-----------|------|------------------------|
| `001_init` | `migrations/001_init.sql` | Creates `conversations` (id, channel, sender_id, started_at, updated_at), `messages` (id, conversation_id, role, content, timestamp, metadata_json), `facts` (id, key, value, source_message_id, created_at, updated_at); creates indexes |
| `002_audit_log` | `migrations/002_audit_log.sql` | Creates `audit_log` (id, timestamp, channel, sender_id, sender_name, input_text, output_text, provider_used, model, processing_ms, status, denial_reason); creates indexes |
| `003_memory_enhancement` | `migrations/003_memory_enhancement.sql` | Adds `summary`, `last_activity`, `status` columns to `conversations`; recreates `facts` table with `sender_id` scoping and `UNIQUE(sender_id, key)` constraint; creates status index |

**SQLite Configuration:**
- Journal mode: WAL (Write-Ahead Logging)
- Max pool connections: 4
- `create_if_missing: true`

---

### `audit` Module

**File:** `crates/omega-memory/src/audit.rs`

**Module Doc Comment:**
```rust
//! Audit log — records every interaction through Omega.
```

**Public Struct: `AuditEntry`**

| Field | Type | Purpose |
|-------|------|---------|
| `channel` | `String` | Source channel (e.g. `"telegram"`) |
| `sender_id` | `String` | Platform-specific user ID |
| `sender_name` | `Option<String>` | Human-readable sender name |
| `input_text` | `String` | The user's message text |
| `output_text` | `Option<String>` | The assistant's response (None if error/denied) |
| `provider_used` | `Option<String>` | Which provider generated the response |
| `model` | `Option<String>` | Model identifier |
| `processing_ms` | `Option<i64>` | Wall-clock processing time in milliseconds |
| `status` | `AuditStatus` | Outcome of the interaction |
| `denial_reason` | `Option<String>` | Reason for denial (if status is `Denied`) |

**Public Enum: `AuditStatus`**

| Variant | String Representation | Meaning |
|---------|----------------------|---------|
| `Ok` | `"ok"` | Interaction completed successfully |
| `Error` | `"error"` | Provider or system error occurred |
| `Denied` | `"denied"` | Auth rejected the request |

**`AuditStatus` Methods (private):**

| Method | Signature | Purpose |
|--------|-----------|---------|
| `as_str` | `fn as_str(&self) -> &'static str` | Convert variant to the string stored in SQLite |

**Public Struct: `AuditLogger`**

| Field (private) | Type |
|----------------|------|
| `pool` | `SqlitePool` |

**Public Methods on `AuditLogger`:**

| Method | Signature | Purpose |
|--------|-----------|---------|
| `new` | `pub fn new(pool: SqlitePool) -> Self` | Create a logger sharing an existing connection pool (typically from `Store::pool()`) |
| `log` | `pub async fn log(&self, entry: &AuditEntry) -> Result<(), OmegaError>` | Insert an audit log entry into the `audit_log` table; logs a `debug!` trace with truncated input |

**Private Functions (module-level):**

| Function | Signature | Purpose |
|----------|-----------|---------|
| `truncate` | `fn truncate(s: &str, max: usize) -> &str` | Return the first `max` bytes of a string (used for debug logging) |

---

## Module Relationships

```
omega_memory::lib
 |
 +-- pub mod store
 |     +-- Store (pub struct)
 |     +-- Uses: omega_core::{MemoryConfig, Context, ContextEntry, OmegaError, IncomingMessage, OutgoingMessage}
 |     +-- Uses: sqlx::{SqlitePool, SqliteConnectOptions, SqlitePoolOptions}
 |     +-- Uses: uuid::Uuid, tracing::info
 |
 +-- pub mod audit
       +-- AuditLogger (pub struct)
       +-- AuditEntry (pub struct)
       +-- AuditStatus (pub enum)
       +-- Uses: omega_core::OmegaError
       +-- Uses: sqlx::SqlitePool
       +-- Uses: uuid::Uuid, tracing::debug
```

The `audit` module depends on `store` at runtime: `AuditLogger::new()` takes a `SqlitePool` that is obtained from `Store::pool()`. However, there is no compile-time dependency between the two modules -- the pool is threaded through by the gateway at initialization.

---

## Public API Surface

| Access Path | Kind | Description |
|-------------|------|-------------|
| `omega_memory::Store` | Struct (re-export) | Persistent conversation/message/fact store |
| `omega_memory::AuditLogger` | Struct (re-export) | Audit log writer |
| `omega_memory::store::Store` | Struct | Same as above via module path |
| `omega_memory::audit::AuditLogger` | Struct | Same as above via module path |
| `omega_memory::audit::AuditEntry` | Struct | Data structure for a single audit log entry |
| `omega_memory::audit::AuditStatus` | Enum | Outcome status (`Ok`, `Error`, `Denied`) |

---

## Dependencies (Cargo.toml)

| Dependency | Usage |
|------------|-------|
| `omega-core` | `MemoryConfig`, `Context`, `ContextEntry`, `OmegaError`, `IncomingMessage`, `OutgoingMessage`, `shellexpand` |
| `tokio` | Async runtime (required by sqlx and async methods) |
| `serde` / `serde_json` | Serialization of `MessageMetadata` to `metadata_json` column |
| `tracing` | `info!` in store initialization, `debug!` in audit logging |
| `thiserror` | Available but unused (errors use `OmegaError` from omega-core) |
| `anyhow` | Available but unused |
| `sqlx` | SQLite connection pool, query execution, raw SQL migrations |
| `chrono` | Available for timestamp types (unused directly -- SQLite `datetime()` handles timestamps) |
| `uuid` | UUID generation for conversation IDs, message IDs, fact IDs, audit log entry IDs |

---

## Database Schema (post-migration)

### `conversations`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `channel` | TEXT | NOT NULL | -- |
| `sender_id` | TEXT | NOT NULL | -- |
| `started_at` | TEXT | NOT NULL | `datetime('now')` |
| `updated_at` | TEXT | NOT NULL | `datetime('now')` |
| `summary` | TEXT | -- | NULL |
| `last_activity` | TEXT | NOT NULL | `datetime('now')` |
| `status` | TEXT | NOT NULL | `'active'` |

Indexes: `idx_conversations_channel_sender (channel, sender_id)`, `idx_conversations_status (status, last_activity)`

### `messages`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `conversation_id` | TEXT | NOT NULL, FK -> conversations(id) | -- |
| `role` | TEXT | NOT NULL, CHECK IN ('user', 'assistant') | -- |
| `content` | TEXT | NOT NULL | -- |
| `timestamp` | TEXT | NOT NULL | `datetime('now')` |
| `metadata_json` | TEXT | -- | NULL |

Index: `idx_messages_conversation (conversation_id, timestamp)`

### `facts`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `sender_id` | TEXT | NOT NULL | -- |
| `key` | TEXT | NOT NULL | -- |
| `value` | TEXT | NOT NULL | -- |
| `source_message_id` | TEXT | FK -> messages(id) | NULL |
| `created_at` | TEXT | NOT NULL | `datetime('now')` |
| `updated_at` | TEXT | NOT NULL | `datetime('now')` |

Constraint: `UNIQUE(sender_id, key)`

### `audit_log`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `timestamp` | TEXT | NOT NULL | `datetime('now')` |
| `channel` | TEXT | NOT NULL | -- |
| `sender_id` | TEXT | NOT NULL | -- |
| `sender_name` | TEXT | -- | NULL |
| `input_text` | TEXT | NOT NULL | -- |
| `output_text` | TEXT | -- | NULL |
| `provider_used` | TEXT | -- | NULL |
| `model` | TEXT | -- | NULL |
| `processing_ms` | INTEGER | -- | NULL |
| `status` | TEXT | NOT NULL, CHECK IN ('ok', 'error', 'denied') | `'ok'` |
| `denial_reason` | TEXT | -- | NULL |

Indexes: `idx_audit_log_timestamp (timestamp)`, `idx_audit_log_sender (channel, sender_id)`

### `_migrations`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `name` | TEXT | PRIMARY KEY | -- |
| `applied_at` | TEXT | NOT NULL | `datetime('now')` |

---

## Summary Table

| Component | File | Kind | Count |
|-----------|------|------|-------|
| `lib.rs` | lib.rs | Crate root | 2 module declarations, 2 re-exports |
| `store` | store.rs | Module | 1 public struct, 15 public methods, 2 private methods, 3 private functions, 1 constant |
| `audit` | audit.rs | Module | 2 public structs, 1 public enum, 2 public methods (across 1 struct), 1 private method, 1 private function |
| Migrations | migrations/ | SQL | 3 migration files, 4 tables, 5 indexes |
