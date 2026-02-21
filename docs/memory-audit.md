# Developer Guide: Audit Logging in Omega

## Path
`crates/omega-memory/src/audit.rs`

## Overview

Every interaction that flows through Omega's gateway is recorded in an audit log. Whether a user sends a message and gets a response, gets denied by auth, or triggers a provider error, the event is written to the `audit_log` table in SQLite. This guide explains what the audit system does, how it works, how to use it, and how to query the data it produces.

## What the Audit System Does

The audit system answers three questions about every interaction:

1. **Who?** -- Which user, on which channel, sent which message.
2. **What happened?** -- Did the AI respond successfully, did the provider fail, or was the user denied?
3. **How?** -- Which provider and model handled the request, and how long did it take.

The audit log is append-only. Once a row is written, it is never updated or deleted. This makes it a reliable source of truth for security review, usage analysis, and debugging.

## Core Types

### AuditEntry

`AuditEntry` is a plain data struct that describes a single interaction. You construct one and pass it to `AuditLogger::log()`.

```rust
use omega_memory::audit::{AuditEntry, AuditStatus};

let entry = AuditEntry {
    channel: "telegram".to_string(),
    sender_id: "123456789".to_string(),
    sender_name: Some("Alice".to_string()),
    input_text: "What is the weather today?".to_string(),
    output_text: Some("I don't have real-time weather data...".to_string()),
    provider_used: Some("Claude Code CLI".to_string()),
    model: Some("claude-opus-4-6".to_string()),
    processing_ms: Some(3200),
    status: AuditStatus::Ok,
    denial_reason: None,
};
```

**Field Guide:**

| Field | Type | When to set | When to leave `None` |
|-------|------|-------------|----------------------|
| `channel` | `String` | Always | -- |
| `sender_id` | `String` | Always | -- |
| `sender_name` | `Option<String>` | When the channel provides a display name | When display name is unavailable |
| `input_text` | `String` | Always | -- |
| `output_text` | `Option<String>` | On success (AI response) or error (`"ERROR: ..."`) | On auth denial (no response generated) |
| `provider_used` | `Option<String>` | When the provider was called (success or error) | On auth denial (provider never called) |
| `model` | `Option<String>` | On success (provider reports model) | On error or denial |
| `processing_ms` | `Option<i64>` | On success (provider reports timing) | On error or denial |
| `status` | `AuditStatus` | Always | -- |
| `denial_reason` | `Option<String>` | On auth denial (reason string) | On success or error |

### AuditStatus

`AuditStatus` is an enum with three variants representing the outcome of an interaction:

| Variant | Stored as | Meaning |
|---------|-----------|---------|
| `AuditStatus::Ok` | `"ok"` | The provider returned a successful response |
| `AuditStatus::Error` | `"error"` | The provider was called but returned an error |
| `AuditStatus::Denied` | `"denied"` | The auth check rejected the sender before the provider was called |

The string representation is enforced by a `CHECK` constraint on the `status` column in SQLite. Attempting to write any other value will cause the insert to fail.

### AuditLogger

`AuditLogger` is the writer. It holds a reference to the SQLite connection pool and provides a single method, `log()`, which inserts an `AuditEntry` into the database.

```rust
use omega_memory::audit::AuditLogger;
use sqlx::SqlitePool;

// The pool is typically obtained from Store::pool()
let logger = AuditLogger::new(pool.clone());
```

## How to Log an Audit Event

### Step 1: Create the AuditLogger

The `AuditLogger` is created once at gateway startup. It shares the same `SqlitePool` as the memory `Store`:

```rust
let audit = AuditLogger::new(memory.pool().clone());
```

You do not need to create the `audit_log` table manually. It is created by migration `002_audit_log.sql` when the `Store` initializes.

### Step 2: Construct an AuditEntry

Build the entry based on what happened:

**Successful interaction:**
```rust
let entry = AuditEntry {
    channel: incoming.channel.clone(),
    sender_id: incoming.sender_id.clone(),
    sender_name: incoming.sender_name.clone(),
    input_text: incoming.text.clone(),
    output_text: Some(response.text.clone()),
    provider_used: Some(response.metadata.provider_used.clone()),
    model: response.metadata.model.clone(),
    processing_ms: Some(response.metadata.processing_time_ms as i64),
    status: AuditStatus::Ok,
    denial_reason: None,
};
```

**Auth denial:**
```rust
let entry = AuditEntry {
    channel: incoming.channel.clone(),
    sender_id: incoming.sender_id.clone(),
    sender_name: incoming.sender_name.clone(),
    input_text: incoming.text.clone(),
    output_text: None,
    provider_used: None,
    model: None,
    processing_ms: None,
    status: AuditStatus::Denied,
    denial_reason: Some("telegram user 999 not in allowed_users".to_string()),
};
```

**Provider error:**
```rust
let entry = AuditEntry {
    channel: incoming.channel.clone(),
    sender_id: incoming.sender_id.clone(),
    sender_name: incoming.sender_name.clone(),
    input_text: incoming.text.clone(),
    output_text: Some(format!("ERROR: {e}")),
    provider_used: Some(self.provider.name().to_string()),
    model: None,
    processing_ms: None,
    status: AuditStatus::Error,
    denial_reason: None,
};
```

### Step 3: Write the Entry

Call `log()` and handle (or discard) the result:

```rust
// Option A: Discard errors (gateway pattern -- audit must not block user response)
let _ = audit.log(&entry).await;

// Option B: Propagate errors (if audit failure should be a hard error)
audit.log(&entry).await?;
```

The gateway uses Option A because delivering the response to the user is more important than logging the interaction. If the database is temporarily unavailable, the audit write fails silently and the user still gets their answer.

## Database Schema

The `audit_log` table is created by migration `002_audit_log.sql`:

```sql
CREATE TABLE IF NOT EXISTS audit_log (
    id              TEXT PRIMARY KEY,
    timestamp       TEXT NOT NULL DEFAULT (datetime('now')),
    channel         TEXT NOT NULL,
    sender_id       TEXT NOT NULL,
    sender_name     TEXT,
    input_text      TEXT NOT NULL,
    output_text     TEXT,
    provider_used   TEXT,
    model           TEXT,
    processing_ms   INTEGER,
    status          TEXT NOT NULL DEFAULT 'ok' CHECK (status IN ('ok', 'error', 'denied')),
    denial_reason   TEXT
);
```

**Key points:**
- `id` is a UUIDv4 generated in Rust, not a SQLite autoincrement.
- `timestamp` is populated automatically by SQLite's `datetime('now')` default. The Rust code does not set it.
- `status` has a CHECK constraint limiting values to `ok`, `error`, and `denied`.
- Two indexes exist for efficient querying (see below).

### Indexes

| Index | Columns | Optimizes |
|-------|---------|-----------|
| `idx_audit_log_timestamp` | `timestamp` | Time-range queries |
| `idx_audit_log_sender` | `channel, sender_id` | Per-user, per-channel queries |

## Querying Audit Data

The audit module does not provide read methods. To query audit data, use SQL directly against the `audit_log` table. Here are common queries:

### Recent interactions for a user
```sql
SELECT timestamp, channel, input_text, output_text, status, processing_ms
FROM audit_log
WHERE sender_id = '123456789'
ORDER BY timestamp DESC
LIMIT 20;
```

### All denied interactions
```sql
SELECT timestamp, channel, sender_id, input_text, denial_reason
FROM audit_log
WHERE status = 'denied'
ORDER BY timestamp DESC;
```

### Error rate over the last 24 hours
```sql
SELECT status, COUNT(*) as count
FROM audit_log
WHERE timestamp > datetime('now', '-1 day')
GROUP BY status;
```

### Average response time by provider
```sql
SELECT provider_used, model,
       COUNT(*) as requests,
       AVG(processing_ms) as avg_ms,
       MAX(processing_ms) as max_ms
FROM audit_log
WHERE status = 'ok' AND processing_ms IS NOT NULL
GROUP BY provider_used, model;
```

### Most active users
```sql
SELECT sender_id, sender_name, COUNT(*) as interactions
FROM audit_log
WHERE status = 'ok'
GROUP BY sender_id
ORDER BY interactions DESC;
```

### Interactions in a time range
```sql
SELECT *
FROM audit_log
WHERE timestamp BETWEEN '2025-01-01 00:00:00' AND '2025-01-31 23:59:59'
ORDER BY timestamp ASC;
```

### Search for specific input text
```sql
SELECT timestamp, sender_id, input_text, output_text
FROM audit_log
WHERE input_text LIKE '%weather%'
ORDER BY timestamp DESC;
```

## Integration with the Gateway Pipeline

The audit system is called at three specific points in the gateway's `handle_message()` pipeline:

### Pipeline Position

```
[1. Auth Check] --denied--> AUDIT (Denied) --> Send deny msg --> Done
      |
   allowed
      |
[2. Sanitize] --> [3. Command?] --> [4. Typing] --> [5. Context]
      |
[6. Provider] ---error---> AUDIT (Error) --> Send error msg --> Done
      |
   success
      |
[7. Memory Store] --> [8. AUDIT (Ok)] --> [9. Send Response] --> Done
```

### What Gets Audited

| Scenario | Audit Status | When in Pipeline |
|----------|-------------|------------------|
| Auth denies the user | `Denied` | After Stage 1, before any processing |
| Provider returns an error | `Error` | After Stage 6, before response delivery |
| Successful exchange | `Ok` | After Stage 7 (memory store), before Stage 9 (send) |

### What Does NOT Get Audited

- **Bot commands** (`/uptime`, `/help`, `/status`, `/facts`, `/memory`, `/forget`) -- these are handled locally and return before reaching the audit stage.
- **Context build failures** -- if the memory store cannot build a context, an error is sent to the user but no audit entry is created.
- **Background tasks** -- conversation summarization and fact extraction run in a background task and do not generate audit entries.

## Event Type Patterns

Understanding the three event types helps you interpret audit data:

### Successful Exchange (`status = 'ok'`)

This is the normal case. A user sent a message, the provider responded, and everything worked.

- `output_text` contains the full AI response.
- `provider_used` identifies which backend handled it (e.g., "Claude Code CLI").
- `model` identifies the specific model (e.g., "claude-opus-4-6").
- `processing_ms` tells you how long the provider took.
- `denial_reason` is `NULL`.

### Auth Denial (`status = 'denied'`)

An unauthorized user attempted to use Omega.

- `output_text` is `NULL` (no response was generated).
- `provider_used` is `NULL` (the provider was never called).
- `model` is `NULL`.
- `processing_ms` is `NULL`.
- `denial_reason` contains the specific reason (e.g., "telegram user 999 not in allowed_users").

This is your primary signal for detecting unauthorized access attempts.

### Provider Error (`status = 'error'`)

The provider was called but failed. This could be a CLI crash, a timeout, a malformed response, or an API error.

- `output_text` contains `"ERROR: {error_message}"` for debugging.
- `provider_used` is set (the provider was attempted).
- `model` is `NULL` (no model info available on failure).
- `processing_ms` is `NULL`.
- `denial_reason` is `NULL`.

A spike in error events suggests a provider issue (API outage, configuration problem, etc.).

## Privacy and Security Considerations

### What is stored

The audit log stores the full text of both the user's input and the AI's response. This is intentional -- it enables debugging, security review, and usage analysis. However, it means the database contains potentially sensitive information.

### Recommendations

1. **Secure the database file.** The SQLite database at `~/.omega/data/memory.db` contains audit data. Restrict file permissions to the Omega user.

2. **Do not expose audit queries to untrusted users.** The audit log contains everyone's messages. Access should be limited to the system operator.

3. **Consider data retention.** Over time, the audit log will grow. You may want to periodically archive or purge old entries:
   ```sql
   DELETE FROM audit_log WHERE timestamp < datetime('now', '-90 days');
   ```

4. **No secrets in messages.** The audit system does not filter or redact sensitive content. If a user sends a password or API key in a message, it will be stored verbatim.

## How the Pool is Shared

The audit system does not have its own database or connection pool. It shares the same `SqlitePool` as the `Store`:

```
Store::new() creates SqlitePool
    |
    +--> Store uses pool for conversations, messages, facts
    |
    +--> Store::pool() exposes pool reference
         |
         +--> Gateway clones pool: AuditLogger::new(memory.pool().clone())
              |
              +--> AuditLogger uses pool for audit_log writes
```

Both the `Store` and `AuditLogger` write to the same `~/.omega/data/memory.db` file. The pool manages connection sharing automatically via sqlx's built-in pooling (max 4 connections).

## Debugging Audit Issues

### Audit writes are failing

If you see `"audit log write failed"` in the tracing logs, the likely causes are:

1. **Database is locked.** Another process has an exclusive lock on the SQLite file. Check for other Omega instances or database browsers holding the file open.

2. **Migration not applied.** The `audit_log` table does not exist. This happens if the database was created before migration `002_audit_log` was added. Re-running Omega should apply pending migrations automatically.

3. **Disk full.** SQLite cannot write because the filesystem is full.

4. **Status constraint violation.** The `CHECK (status IN ('ok', 'error', 'denied'))` constraint will reject any other value. This should never happen unless the `AuditStatus::as_str()` method is modified incorrectly.

### Audit writes are not appearing

Remember that the gateway uses `let _ =` to discard audit results. If the write fails, no error is propagated -- it's silently dropped. Check the tracing logs at `debug` level to see if writes are succeeding.

### Timestamp discrepancies

The `timestamp` column is set by SQLite's `datetime('now')`, which uses UTC. If you're comparing audit timestamps to local time, account for the timezone difference.

## Example: Adding a New Audit Point

If you add a new stage to the gateway pipeline and want to audit it, follow this pattern:

```rust
use omega_memory::audit::{AuditEntry, AuditStatus};

// After the new stage completes (or fails):
let _ = self
    .audit
    .log(&AuditEntry {
        channel: incoming.channel.clone(),
        sender_id: incoming.sender_id.clone(),
        sender_name: incoming.sender_name.clone(),
        input_text: incoming.text.clone(),
        output_text: Some("your output here".to_string()),
        provider_used: None,  // or Some(...) if a provider was involved
        model: None,
        processing_ms: None,  // or Some(elapsed_ms) if you measured timing
        status: AuditStatus::Ok,  // or Error/Denied as appropriate
        denial_reason: None,
    })
    .await;
```

Key decisions:
- Use `let _ =` if audit failure should not block the pipeline.
- Use `?` if audit failure should propagate and stop processing.
- Set `status` to the most appropriate variant for the outcome.

## Summary

The audit system is deliberately simple: one struct for data (`AuditEntry`), one enum for status (`AuditStatus`), and one writer (`AuditLogger`) with a single `log()` method. It writes to a shared SQLite database, relies on SQLite defaults for timestamps, and is designed to be fire-and-forget in the gateway pipeline. Query it directly with SQL when you need to review interactions, detect unauthorized access, or debug provider issues.
