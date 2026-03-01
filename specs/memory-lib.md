# Technical Specification: `backend/crates/omega-memory/src/lib.rs`

## File

| Field | Value |
|-------|-------|
| Path | `backend/crates/omega-memory/src/lib.rs` |
| Crate | `omega-memory` |
| Role | Crate root -- declares modules and re-exports their primary types |

## Purpose

`lib.rs` is the entry point for the `omega-memory` crate. It serves three purposes:

1. Declare the two submodules that compose the persistent memory system.
2. Re-export the primary types (`Store`, `DueTask`, `AuditLogger`, `detect_language`) at crate root for ergonomic imports.
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
| `store` | `pub mod` (public) | Implemented | Persistent conversation memory. A directory module split into 8 submodules: `conversations`, `messages`, `facts`, `tasks`, `context`, `context_helpers`, `outcomes`, and `sessions`. Manages conversations, messages, facts, tasks, outcomes, lessons, sessions, and context building via SQLite. |

Both modules are fully implemented and publicly accessible.

## Re-exports

```rust
pub use audit::AuditLogger;
pub use store::detect_language;
pub use store::DueTask;
pub use store::Store;
```

Four `pub use` re-exports bring the primary types to the crate root:

```rust
// Via re-export (preferred):
use omega_memory::{Store, AuditLogger, DueTask, detect_language};

// Via module path (also valid):
use omega_memory::store::Store;
use omega_memory::audit::AuditLogger;
```

---

## Module Details

### `store` Module

**Directory:** `backend/crates/omega-memory/src/store/` (directory module with 8 submodules + test module)

**Module Doc Comment:**
```rust
//! SQLite-backed persistent memory store.
//!
//! Split into focused submodules:
//! - `conversations` -- conversation lifecycle (create, find, close, summaries)
//! - `messages` -- message storage and full-text search
//! - `facts` -- user facts, aliases, and limitations
//! - `tasks` -- scheduled task CRUD and dedup
//! - `context` -- context building and user profile formatting
//! - `context_helpers` -- onboarding stages, system prompt composition, language detection
```

**Submodules:**

| Submodule | Visibility | File | Responsibility |
|-----------|-----------|------|----------------|
| `conversations` | `mod` (private) | `conversations.rs` | Conversation lifecycle: create, find idle/active, close, summaries, history, stats |
| `messages` | `mod` (private) | `messages.rs` | Message storage (`store_exchange`) and FTS5 full-text search (`search_messages`) |
| `facts` | `mod` (private) | `facts.rs` | User facts, cross-channel aliases, and limitations |
| `tasks` | `mod` (private) | `tasks.rs` | Scheduled task CRUD, deduplication, retry logic |
| `context` | `mod` (private) | `context.rs` | Context building for AI providers, user profile formatting |
| `context_helpers` | `mod` (private) | `context_helpers.rs` | Onboarding stages, system prompt composition, language detection |
| `outcomes` | `mod` (private) | `outcomes.rs` | Reward-based learning: raw outcomes and distilled lessons |
| `sessions` | `mod` (private) | `sessions.rs` | Project-scoped CLI session persistence |
| `tests` | `cfg(test)` | `tests.rs` | Unit tests for all store functionality |

**Public Re-exports from `store/mod.rs`:**

```rust
pub use context::{detect_language, format_user_profile};
pub use tasks::DueTask;
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

| Method | Signature | Submodule | Purpose |
|--------|-----------|-----------|---------|
| `new` | `pub async fn new(config: &MemoryConfig) -> Result<Self, OmegaError>` | `mod.rs` | Create a store: expand `~` in db_path, create parent dirs, open SQLite with WAL journal mode, run 13 migrations |
| `pool` | `pub fn pool(&self) -> &SqlitePool` | `mod.rs` | Return a reference to the underlying connection pool (used by `AuditLogger`) |
| `db_size` | `pub async fn db_size(&self) -> Result<u64, OmegaError>` | `mod.rs` | Get database file size in bytes via `PRAGMA page_count * page_size` |
| `find_idle_conversations` | `pub async fn find_idle_conversations(&self) -> Result<Vec<(String, String, String, String)>, OmegaError>` | `conversations` | Find active conversations idle beyond the timeout; returns `(id, channel, sender_id, project)` tuples |
| `find_all_active_conversations` | `pub async fn find_all_active_conversations(&self) -> Result<Vec<(String, String, String, String)>, OmegaError>` | `conversations` | Find all active conversations regardless of idle time (used for shutdown); returns `(id, channel, sender_id, project)` tuples |
| `get_conversation_messages` | `pub async fn get_conversation_messages(&self, conversation_id: &str) -> Result<Vec<(String, String)>, OmegaError>` | `conversations` | Get all messages for a conversation in chronological order; returns `(role, content)` tuples |
| `close_conversation` | `pub async fn close_conversation(&self, conversation_id: &str, summary: &str) -> Result<(), OmegaError>` | `conversations` | Set conversation status to `closed` and store a summary |
| `close_current_conversation` | `pub async fn close_current_conversation(&self, channel: &str, sender_id: &str, project: &str) -> Result<bool, OmegaError>` | `conversations` | Close the active conversation for a sender on a channel+project (for `/forget` command); returns whether a conversation was closed |
| `get_recent_summaries` | `pub async fn get_recent_summaries(&self, channel: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String)>, OmegaError>` | `conversations` | Get recent closed conversation summaries; returns `(summary, updated_at)` tuples ordered newest-first |
| `get_all_recent_summaries` | `pub async fn get_all_recent_summaries(&self, limit: i64) -> Result<Vec<(String, String)>, OmegaError>` | `conversations` | Get recent summaries across all users (for heartbeat enrichment); returns `(summary, updated_at)` tuples |
| `get_history` | `pub async fn get_history(&self, channel: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String)>, OmegaError>` | `conversations` | Get conversation summaries with timestamps for a sender; returns `(summary_or_fallback, updated_at)` tuples |
| `get_memory_stats` | `pub async fn get_memory_stats(&self, sender_id: &str) -> Result<(i64, i64, i64), OmegaError>` | `conversations` | Count conversations, messages, and facts for a sender; returns `(conv_count, msg_count, fact_count)` |
| `store_exchange` | `pub async fn store_exchange(&self, incoming: &IncomingMessage, response: &OutgoingMessage, project: &str) -> Result<(), OmegaError>` | `messages` | Store both the user message and assistant response in the conversation (project-scoped) |
| `search_messages` | `pub async fn search_messages(&self, query: &str, exclude_conversation_id: &str, sender_id: &str, limit: i64) -> Result<Vec<(String, String, String)>, OmegaError>` | `messages` | FTS5 full-text search across past conversations; returns `(role, content, timestamp)` tuples |
| `store_fact` | `pub async fn store_fact(&self, sender_id: &str, key: &str, value: &str) -> Result<(), OmegaError>` | `facts` | Upsert a fact by `(sender_id, key)` -- inserts or updates value |
| `get_fact` | `pub async fn get_fact(&self, sender_id: &str, key: &str) -> Result<Option<String>, OmegaError>` | `facts` | Get a single fact by sender and key |
| `delete_fact` | `pub async fn delete_fact(&self, sender_id: &str, key: &str) -> Result<bool, OmegaError>` | `facts` | Delete a single fact by sender and key; returns `true` if a row was deleted |
| `get_facts` | `pub async fn get_facts(&self, sender_id: &str) -> Result<Vec<(String, String)>, OmegaError>` | `facts` | Get all facts for a sender; returns `(key, value)` tuples ordered by key |
| `delete_facts` | `pub async fn delete_facts(&self, sender_id: &str, key: Option<&str>) -> Result<u64, OmegaError>` | `facts` | Delete facts: all facts for a sender if `key` is `None`, or a specific fact if `key` is `Some`; returns rows deleted |
| `get_all_facts` | `pub async fn get_all_facts(&self) -> Result<Vec<(String, String)>, OmegaError>` | `facts` | Get all facts across all users (for heartbeat enrichment); excludes `welcomed` key |
| `get_all_facts_by_key` | `pub async fn get_all_facts_by_key(&self, key: &str) -> Result<Vec<(String, String)>, OmegaError>` | `facts` | Get all `(sender_id, value)` pairs for a given fact key across all users |
| `is_new_user` | `pub async fn is_new_user(&self, sender_id: &str) -> Result<bool, OmegaError>` | `facts` | Check if a sender has never been welcomed (no `welcomed` fact) |
| `resolve_sender_id` | `pub async fn resolve_sender_id(&self, sender_id: &str) -> Result<String, OmegaError>` | `facts` | Resolve a sender_id to its canonical form via the `user_aliases` table |
| `create_alias` | `pub async fn create_alias(&self, alias_id: &str, canonical_id: &str) -> Result<(), OmegaError>` | `facts` | Create a cross-channel alias mapping: alias_id -> canonical_id |
| `find_canonical_user` | `pub async fn find_canonical_user(&self, exclude_sender_id: &str) -> Result<Option<String>, OmegaError>` | `facts` | Find an existing welcomed user different from `sender_id` (for auto-aliasing) |
| `store_limitation` | `pub async fn store_limitation(&self, title: &str, description: &str, proposed_plan: &str) -> Result<bool, OmegaError>` | `facts` | Store a limitation (deduplicates by title, case-insensitive); returns `true` if new |
| `get_open_limitations` | `pub async fn get_open_limitations(&self) -> Result<Vec<(String, String, String)>, OmegaError>` | `facts` | Get all open limitations: `(title, description, proposed_plan)` |
| `create_task` | `pub async fn create_task(&self, channel: &str, sender_id: &str, reply_target: &str, description: &str, due_at: &str, repeat: Option<&str>, task_type: &str, project: &str) -> Result<String, OmegaError>` | `tasks` | Create a scheduled task with two-level deduplication (exact + fuzzy) |
| `get_due_tasks` | `pub async fn get_due_tasks(&self) -> Result<Vec<DueTask>, OmegaError>` | `tasks` | Get tasks that are due for delivery |
| `complete_task` | `pub async fn complete_task(&self, id: &str, repeat: Option<&str>) -> Result<(), OmegaError>` | `tasks` | Complete a task: one-shot -> delivered, recurring -> advance due_at |
| `fail_task` | `pub async fn fail_task(&self, id: &str, error: &str, max_retries: u32) -> Result<bool, OmegaError>` | `tasks` | Fail an action task with retry logic; returns `true` if will retry |
| `get_tasks_for_sender` | `pub async fn get_tasks_for_sender(&self, sender_id: &str) -> Result<Vec<(String, String, String, Option<String>, String, String)>, OmegaError>` | `tasks` | Get pending tasks for a sender: `(id, description, due_at, repeat, task_type, project)` |
| `cancel_task` | `pub async fn cancel_task(&self, id_prefix: &str, sender_id: &str) -> Result<bool, OmegaError>` | `tasks` | Cancel a task by ID prefix (must match sender); idempotent |
| `update_task` | `pub async fn update_task(&self, id_prefix: &str, sender_id: &str, description: Option<&str>, due_at: Option<&str>, repeat: Option<&str>) -> Result<bool, OmegaError>` | `tasks` | Update fields of a pending task by ID prefix; only non-None fields are updated |
| `defer_task` | `pub async fn defer_task(&self, id: &str, new_due_at: &str) -> Result<(), OmegaError>` | `tasks` | Defer a pending task to a new due_at time (by exact ID) |
| `build_context` | `pub async fn build_context(&self, incoming: &IncomingMessage, base_system_prompt: &str, needs: &ContextNeeds, active_project: Option<&str>) -> Result<Context, OmegaError>` | `context` | Build a full `Context` for the AI provider: get/create conversation, load history, fetch facts, summaries, recall, tasks, outcomes, lessons in parallel, generate enriched system prompt |
| `store_outcome` | `pub async fn store_outcome(&self, sender_id: &str, domain: &str, score: i32, lesson: &str, source: &str, project: &str) -> Result<(), OmegaError>` | `outcomes` | Store a raw outcome from a REWARD marker (project-scoped) |
| `get_recent_outcomes` | `pub async fn get_recent_outcomes(&self, sender_id: &str, limit: i64, project: Option<&str>) -> Result<Vec<(i32, String, String, String)>, OmegaError>` | `outcomes` | Get recent outcomes for a sender; returns `(score, domain, lesson, timestamp)` |
| `get_all_recent_outcomes` | `pub async fn get_all_recent_outcomes(&self, hours: i64, limit: i64, project: Option<&str>) -> Result<Vec<(i32, String, String, String)>, OmegaError>` | `outcomes` | Get recent outcomes across all users within N hours (for heartbeat) |
| `store_lesson` | `pub async fn store_lesson(&self, sender_id: &str, domain: &str, rule: &str, project: &str) -> Result<(), OmegaError>` | `outcomes` | Store a distilled lesson with content-based dedup; multiple lessons per (sender, domain, project), capped at 10 |
| `get_lessons` | `pub async fn get_lessons(&self, sender_id: &str, project: Option<&str>) -> Result<Vec<(String, String, String)>, OmegaError>` | `outcomes` | Get lessons for a sender; returns `(domain, rule, project)` |
| `get_all_lessons` | `pub async fn get_all_lessons(&self, project: Option<&str>) -> Result<Vec<(String, String, String)>, OmegaError>` | `outcomes` | Get all lessons across all users (for heartbeat); returns `(domain, rule, project)` |
| `store_session` | `pub async fn store_session(&self, channel: &str, sender_id: &str, project: &str, session_id: &str) -> Result<(), OmegaError>` | `sessions` | Upsert a CLI session for a (channel, sender_id, project) tuple |
| `get_session` | `pub async fn get_session(&self, channel: &str, sender_id: &str, project: &str) -> Result<Option<String>, OmegaError>` | `sessions` | Look up the CLI session_id for a (channel, sender_id, project) tuple |
| `clear_session` | `pub async fn clear_session(&self, channel: &str, sender_id: &str, project: &str) -> Result<(), OmegaError>` | `sessions` | Delete the CLI session for a specific (channel, sender_id, project) |
| `clear_all_sessions_for_sender` | `pub async fn clear_all_sessions_for_sender(&self, sender_id: &str) -> Result<(), OmegaError>` | `sessions` | Delete all CLI sessions for a sender (used by /forget-all) |

**`pub(crate)` Methods on `Store`:**

| Method | Signature | Submodule | Purpose |
|--------|-----------|-----------|---------|
| `get_or_create_conversation` | `pub(crate) async fn get_or_create_conversation(&self, channel: &str, sender_id: &str, project: &str) -> Result<String, OmegaError>` | `conversations` | Find an active, non-idle conversation for a channel+sender+project or create a new one; returns conversation ID |

**Private Methods on `Store`:**

| Method | Signature | Submodule | Purpose |
|--------|-----------|-----------|---------|
| `run_migrations` | `async fn run_migrations(pool: &SqlitePool) -> Result<(), OmegaError>` | `mod.rs` | Run 13 SQL migrations with idempotent tracking via `_migrations` table; handles bootstrap from pre-tracking databases |

**Public Struct: `DueTask`**

| Field | Type | Purpose |
|-------|------|---------|
| `id` | `String` | Unique task identifier |
| `channel` | `String` | Channel name (e.g. "telegram") |
| `sender_id` | `String` | Sender/user identifier |
| `reply_target` | `String` | Reply target for message delivery |
| `description` | `String` | Human-readable task description |
| `repeat` | `Option<String>` | Repeat schedule (None = one-shot) |
| `task_type` | `String` | Task type: "reminder" or "action" |
| `project` | `String` | Project scope (empty string = global) |

**Public Free Functions:**

| Function | Signature | Submodule | Purpose |
|----------|-----------|-----------|---------|
| `detect_language` | `pub fn detect_language(text: &str) -> &'static str` | `context_helpers` | Detect the most likely language of a text using stop-word heuristics; supports English, Spanish, Portuguese, French, German, Italian, Dutch, Russian |
| `format_user_profile` | `pub fn format_user_profile(facts: &[(String, String)]) -> String` | `context` | Format user facts into a structured profile string, grouping identity facts first, then context, then the rest |

**Private Functions and Types (module-level):**

| Item | Kind | Submodule | Purpose |
|------|------|-----------|---------|
| `build_system_prompt` | function | `context_helpers` | Generate a dynamic system prompt by combining base prompt with facts, summaries, recall, tasks, outcomes, lessons, language directive, and onboarding hints |
| `compute_onboarding_stage` | function | `context_helpers` | Compute the next onboarding stage (0-5) based on current state |
| `onboarding_hint_text` | function | `context_helpers` | Return the onboarding hint text for a given stage |
| `format_relative_time` | function | `context_helpers` | Format a UTC timestamp as relative time string (e.g., "3h ago") |
| `SystemPromptContext` | struct | `context_helpers` | Parameters for building a dynamic system prompt |
| `normalize_due_at` | function | `tasks` | Normalize a datetime string for dedup comparison |
| `descriptions_are_similar` | function | `tasks` | Check if two task descriptions are semantically similar via word overlap |
| `significant_words` | function | `tasks` | Extract significant words from text for fuzzy matching |

**Migration System:**

The `run_migrations` method implements a custom migration tracker:

1. Creates a `_migrations` table with columns `name TEXT PRIMARY KEY` and `applied_at TEXT`.
2. Bootstrap logic: if `_migrations` is empty but the schema already has Phase 3 columns (e.g. `summary` on `conversations`), marks migrations `001_init`, `002_audit_log`, and `003_memory_enhancement` as already applied.
3. For each migration in the ordered list, checks `_migrations` for prior application; if not found, executes the SQL and records it.

**Migrations (13 total):**

| Migration | File | Tables/Columns Affected |
|-----------|------|------------------------|
| `001_init` | `migrations/001_init.sql` | Creates `conversations`, `messages`, `facts` with indexes |
| `002_audit_log` | `migrations/002_audit_log.sql` | Creates `audit_log` with indexes |
| `003_memory_enhancement` | `migrations/003_memory_enhancement.sql` | Adds `summary`, `last_activity`, `status` to `conversations`; recreates `facts` with sender_id scoping |
| `004_fts5_recall` | `migrations/004_fts5_recall.sql` | Creates `messages_fts` FTS5 virtual table, 3 sync triggers, backfill |
| `005_scheduled_tasks` | `migrations/005_scheduled_tasks.sql` | Creates `scheduled_tasks` table with 2 indexes |
| `006_limitations` | `migrations/006_limitations.sql` | Creates `limitations` table with unique case-insensitive title index |
| `007_task_type` | `migrations/007_task_type.sql` | Adds `task_type` column to `scheduled_tasks` |
| `008_user_aliases` | `migrations/008_user_aliases.sql` | Creates `user_aliases` table |
| `009_task_retry` | `migrations/009_task_retry.sql` | Adds `retry_count`, `last_error` columns to `scheduled_tasks` |
| `010_outcomes` | `migrations/010_outcomes.sql` | Creates `outcomes` and `lessons` tables |
| `011_project_learning` | `migrations/011_project_learning.sql` | Adds `project` column to `outcomes`, `lessons`, `scheduled_tasks`; recreates `lessons` with project in unique constraint |
| `012_project_sessions` | `migrations/012_project_sessions.sql` | Creates `project_sessions` table; adds `project` column to `conversations` |
| `013_multi_lessons` | `migrations/013_multi_lessons.sql` | Recreates `lessons` table without unique constraint; allows multiple lessons per (sender, domain, project) |

**SQLite Configuration:**
- Journal mode: WAL (Write-Ahead Logging)
- Max pool connections: 4
- `create_if_missing: true`

---

### `audit` Module

**File:** `backend/crates/omega-memory/src/audit.rs`

**Module Doc Comment:**
```rust
//! Audit log -- records every interaction through Omega.
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
| `truncate` | `fn truncate(s: &str, max: usize) -> &str` | Return the first `max` bytes of a string, aligned to a valid char boundary (used for debug logging) |

---

## Module Relationships

```
omega_memory::lib
 |
 +-- pub mod store  (directory module: store/)
 |     +-- Store (pub struct)
 |     +-- DueTask (pub struct, re-exported)
 |     +-- detect_language (pub fn, re-exported)
 |     +-- format_user_profile (pub fn)
 |     +-- Submodules: conversations, messages, facts, tasks,
 |     |               context, context_helpers, outcomes, sessions
 |     +-- Uses: omega_core::{MemoryConfig, Context, ContextEntry, ContextNeeds,
 |     |         OmegaError, IncomingMessage, OutgoingMessage, SYSTEM_FACT_KEYS, shellexpand}
 |     +-- Uses: sqlx::{SqlitePool, SqliteConnectOptions, SqlitePoolOptions}
 |     +-- Uses: uuid::Uuid, chrono, tracing::info, serde_json
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
| `omega_memory::Store` | Struct (re-export) | Persistent conversation/message/fact/task/outcome/session store |
| `omega_memory::AuditLogger` | Struct (re-export) | Audit log writer |
| `omega_memory::DueTask` | Struct (re-export) | Scheduled task due for delivery |
| `omega_memory::detect_language` | Function (re-export) | Language detection via stop-word heuristics |
| `omega_memory::store::Store` | Struct | Same as above via module path |
| `omega_memory::store::format_user_profile` | Function | Format user facts into a structured profile |
| `omega_memory::audit::AuditLogger` | Struct | Same as above via module path |
| `omega_memory::audit::AuditEntry` | Struct | Data structure for a single audit log entry |
| `omega_memory::audit::AuditStatus` | Enum | Outcome status (`Ok`, `Error`, `Denied`) |

---

## Dependencies (Cargo.toml)

| Dependency | Usage |
|------------|-------|
| `omega-core` | `MemoryConfig`, `Context`, `ContextEntry`, `ContextNeeds`, `OmegaError`, `IncomingMessage`, `OutgoingMessage`, `SYSTEM_FACT_KEYS`, `shellexpand` |
| `tokio` | Async runtime (required by sqlx and async methods), `tokio::join!` for parallel queries |
| `serde` / `serde_json` | Serialization of `MessageMetadata` to `metadata_json` column |
| `tracing` | `info!` in store initialization, `debug!` in audit logging |
| `thiserror` | Available but unused (errors use `OmegaError` from omega-core) |
| `anyhow` | Available but unused |
| `sqlx` | SQLite connection pool, query execution, raw SQL migrations |
| `chrono` | Timestamp formatting in `format_relative_time`, `Utc::now()` for relative time calculations |
| `uuid` | UUID generation for conversation IDs, message IDs, fact IDs, task IDs, outcome IDs, session IDs, audit log entry IDs |

---

## Database Schema (post-migration 013)

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
| `project` | TEXT | NOT NULL | `''` |

Indexes: `idx_conversations_channel_sender`, `idx_conversations_status`, `idx_conversations_project`

### `messages`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `conversation_id` | TEXT | NOT NULL, FK -> conversations(id) | -- |
| `role` | TEXT | NOT NULL, CHECK IN ('user', 'assistant') | -- |
| `content` | TEXT | NOT NULL | -- |
| `timestamp` | TEXT | NOT NULL | `datetime('now')` |
| `metadata_json` | TEXT | -- | NULL |

Index: `idx_messages_conversation`

### `messages_fts` (FTS5 Virtual Table)

Content-less FTS5 index on `messages.content`, synced via triggers.

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

Indexes: `idx_audit_log_timestamp`, `idx_audit_log_sender`

### `scheduled_tasks`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `channel` | TEXT | NOT NULL | -- |
| `sender_id` | TEXT | NOT NULL | -- |
| `reply_target` | TEXT | NOT NULL | -- |
| `description` | TEXT | NOT NULL | -- |
| `due_at` | TEXT | NOT NULL | -- |
| `repeat` | TEXT | -- | NULL |
| `status` | TEXT | NOT NULL | `'pending'` |
| `created_at` | TEXT | NOT NULL | `datetime('now')` |
| `delivered_at` | TEXT | -- | NULL |
| `task_type` | TEXT | NOT NULL | `'reminder'` |
| `retry_count` | INTEGER | NOT NULL | `0` |
| `last_error` | TEXT | -- | NULL |
| `project` | TEXT | NOT NULL | `''` |

Indexes: `idx_scheduled_tasks_due`, `idx_scheduled_tasks_sender`

### `limitations`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `title` | TEXT | NOT NULL | -- |
| `description` | TEXT | NOT NULL | -- |
| `proposed_plan` | TEXT | NOT NULL | `''` |
| `status` | TEXT | NOT NULL | `'open'` |
| `created_at` | TEXT | NOT NULL | `datetime('now')` |
| `resolved_at` | TEXT | -- | NULL |

Index: `idx_limitations_title` (UNIQUE, COLLATE NOCASE)

### `user_aliases`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `alias_sender_id` | TEXT | PRIMARY KEY | -- |
| `canonical_sender_id` | TEXT | NOT NULL | -- |

### `outcomes`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `sender_id` | TEXT | NOT NULL | -- |
| `domain` | TEXT | NOT NULL | -- |
| `score` | INTEGER | NOT NULL | -- |
| `lesson` | TEXT | NOT NULL | -- |
| `source` | TEXT | NOT NULL | -- |
| `timestamp` | TEXT | NOT NULL | `datetime('now')` |
| `project` | TEXT | NOT NULL | `''` |

Index: `idx_outcomes_project`

### `lessons`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `sender_id` | TEXT | NOT NULL | -- |
| `domain` | TEXT | NOT NULL | -- |
| `rule` | TEXT | NOT NULL | -- |
| `project` | TEXT | NOT NULL | `''` |
| `occurrences` | INTEGER | NOT NULL | `1` |
| `created_at` | TEXT | NOT NULL | `datetime('now')` |
| `updated_at` | TEXT | NOT NULL | `datetime('now')` |

Indexes: `idx_lessons_sender`, `idx_lessons_project`, `idx_lessons_domain`

Note: No unique constraint on lessons (removed in migration 013). Multiple lessons per (sender_id, domain, project) are allowed.

### `project_sessions`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `id` | TEXT | PRIMARY KEY | -- |
| `channel` | TEXT | NOT NULL | -- |
| `sender_id` | TEXT | NOT NULL | -- |
| `project` | TEXT | NOT NULL | `''` |
| `session_id` | TEXT | NOT NULL | -- |
| `parent_project` | TEXT | -- | NULL |
| `created_at` | TEXT | NOT NULL | `datetime('now')` |
| `updated_at` | TEXT | NOT NULL | `datetime('now')` |

Constraint: `UNIQUE(channel, sender_id, project)`
Index: `idx_project_sessions_lookup`

### `_migrations`

| Column | Type | Constraints | Default |
|--------|------|-------------|---------|
| `name` | TEXT | PRIMARY KEY | -- |
| `applied_at` | TEXT | NOT NULL | `datetime('now')` |

---

## Summary Table

| Component | Location | Kind | Count |
|-----------|----------|------|-------|
| `lib.rs` | lib.rs | Crate root | 2 module declarations, 4 re-exports |
| `store` | store/ | Directory module | 1 public struct (Store), 1 public struct (DueTask), 42 public methods, 1 pub(crate) method, 1 private method, 2 public free functions, 6+ private functions, 1 constant |
| `audit` | audit.rs | Module | 2 public structs, 1 public enum, 2 public methods, 1 private method, 1 private function |
| Migrations | migrations/ | SQL | 13 migration files, 11 tables (+ 1 virtual), multiple indexes |
