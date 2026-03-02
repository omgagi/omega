# Architecture: Google Account Setup (`/google` Command)

## Scope

Gateway-layer command for capturing and storing Google OAuth credentials via a chat-based wizard. Affects: `commands/mod.rs`, `gateway/mod.rs`, `gateway/pipeline.rs`, `gateway/keywords_data.rs`, and two new files: `gateway/google_auth.rs` (state machine + logic) and `gateway/google_auth_i18n.rs` (localized messages for 8 languages).

This is a single-milestone feature. No AI provider involvement -- pure gateway state machine.

## Overview

```
User types /google
    |
    v
pipeline.rs intercepts Command::Google
    |
    v
google_auth::start_google_session()
    |-- Check concurrent session (pending_google fact)
    |-- Check existing google.json (overwrite warning)
    |-- Store pending_google fact: "<ts>|client_id"
    |-- Send Step 1 prompt (ask for client_id)
    |
    v
User sends client_id
    |
    v
pipeline.rs detects pending_google fact (before context building)
    |
    v
google_auth::handle_google_response()
    |-- Check TTL (10 min)
    |-- Check cancellation
    |-- Parse current step from fact
    |-- Validate input
    |-- Store credential in temp fact
    |-- Advance to next step or complete
    |
    (repeat for client_secret, refresh_token, email)
    |
    v
On final step (email received):
    |-- Write ~/.omega/stores/google.json (0600 perms)
    |-- Cleanup all temp facts
    |-- Audit log [GOOGLE_AUTH] (no credential values)
    |-- Send completion message
```

## Modules

### Module 1: Command Registration (`backend/src/commands/mod.rs`)

- **Responsibility**: Add `Google` variant to `Command` enum so `/google` is parsed.
- **Public interface**: `Command::Google` variant; `Command::parse()` match arm for `/google`.
- **Dependencies**: None (self-contained enum addition).
- **Implementation order**: 1

**Changes:**
```rust
// Add to Command enum:
Google,

// Add to Command::parse() match:
"/google" => Some(Self::Google),

// Add to handle() match (fallback, like Setup):
Command::Google => status::handle_help(&lang),
```

#### Failure Modes
| Failure | Cause | Detection | Recovery | Impact |
|---------|-------|-----------|----------|--------|
| Unknown command fallthrough | `/google` not matched | Unit test | Fix match arm | Users see provider response instead of wizard |

#### Security Considerations
- **Trust boundary**: None at this layer -- just enum parsing.
- **Sensitive data**: None.
- **Attack surface**: None -- input is already sanitized by pipeline step 2.
- **Mitigations**: Botname suffix stripping already handled by existing `split('@')` logic.

### Module 2: TTL Constant (`backend/src/gateway/keywords_data.rs`)

- **Responsibility**: Define `GOOGLE_AUTH_TTL_SECS` constant.
- **Public interface**: `pub(super) const GOOGLE_AUTH_TTL_SECS: i64 = 600;`
- **Dependencies**: None.
- **Implementation order**: 2

**Changes:**
```rust
/// Maximum seconds a Google auth session stays valid.
pub(super) const GOOGLE_AUTH_TTL_SECS: i64 = 600; // 10 minutes
```

### Module 3: Localized Messages (`backend/src/gateway/google_auth_i18n.rs`)

- **Responsibility**: All user-facing messages for the `/google` flow in 8 languages.
- **Public interface**: `pub(super)` functions consumed by `google_auth.rs`.
- **Dependencies**: None.
- **Implementation order**: 3

**Rationale for separate file**: The `/google` flow requires ~10 i18n functions, each with 8 language variants. At ~15 lines per function, that is ~150 lines of i18n alone. Combined with state machine logic in a single file, the 500-line limit would be hit. Splitting follows the exact same pattern as `keywords.rs`/`keywords_data.rs`.

#### Function Signatures

```rust
/// Step 1: Initial prompt asking for client_id.
/// Includes overwrite warning if `existing` is true.
pub(super) fn google_step1_message(lang: &str, existing: bool) -> String;

/// Step 2: Received client_id, asking for client_secret.
pub(super) fn google_step2_message(lang: &str) -> &'static str;

/// Step 3: Received client_secret, asking for refresh_token.
pub(super) fn google_step3_message(lang: &str) -> &'static str;

/// Step 4: Received refresh_token, asking for email.
pub(super) fn google_step4_message(lang: &str) -> &'static str;

/// Completion: All credentials stored successfully.
pub(super) fn google_complete_message(lang: &str) -> &'static str;

/// Cancellation confirmation.
pub(super) fn google_cancelled_message(lang: &str) -> &'static str;

/// Session expired (10 min TTL).
pub(super) fn google_expired_message(lang: &str) -> &'static str;

/// Concurrent session guard -- active session already exists.
pub(super) fn google_conflict_message(lang: &str) -> &'static str;

/// Validation error: empty input.
pub(super) fn google_empty_input_message(lang: &str) -> &'static str;

/// Validation error: invalid email format.
pub(super) fn google_invalid_email_message(lang: &str) -> &'static str;
```

#### Failure Modes
| Failure | Cause | Detection | Recovery | Impact |
|---------|-------|-----------|----------|--------|
| Missing language | User has unknown `preferred_language` fact | Default `_` arm in every match | Falls through to English | User sees English instead of native language |

### Module 4: State Machine (`backend/src/gateway/google_auth.rs`)

- **Responsibility**: Session lifecycle: start, step transitions, validation, credential storage, cleanup, audit.
- **Public interface**: Two `impl Gateway` methods: `start_google_session()`, `handle_google_response()`.
- **Dependencies**: `keywords` (re-exported `GOOGLE_AUTH_TTL_SECS`, `is_build_cancelled()`), `google_auth_i18n`, `omega_memory::Store` (facts), `omega_memory::audit::AuditLogger`, `tokio::fs` (write `google.json`).
- **Implementation order**: 4

#### State Machine Design

**Fact-based state** (same pattern as `/setup`):

The session state is stored as a single fact with key `pending_google` and value format:

```
<unix_timestamp>|<step>
```

Where `<step>` is one of: `client_id`, `client_secret`, `refresh_token`, `email`.

The step name indicates **what the user is expected to provide next** (i.e., what we are waiting for).

**Temporary credential facts** (stored during session, cleaned up on all exit paths):

| Fact key | Stored after | Contains |
|----------|-------------|----------|
| `_google_client_id` | Step 1 response validated | The client_id value |
| `_google_client_secret` | Step 2 response validated | The client_secret value |
| `_google_refresh_token` | Step 3 response validated | The refresh_token value |

The `_` prefix convention signals these are internal/temporary facts that should not appear in user-facing `/facts` output. (The existing facts display already filters by showing all facts, but these are cleaned up promptly so exposure window is minimal.)

**Step transitions:**

```
/google command
  --> store fact: pending_google = "<ts>|client_id"
  --> send: google_step1_message

User sends client_id
  --> validate: non-empty, trimmed
  --> store fact: _google_client_id = <value>
  --> update fact: pending_google = "<ts>|client_secret"
  --> send: google_step2_message

User sends client_secret
  --> validate: non-empty, trimmed
  --> store fact: _google_client_secret = <value>
  --> update fact: pending_google = "<ts>|refresh_token"
  --> send: google_step3_message

User sends refresh_token
  --> validate: non-empty, trimmed
  --> store fact: _google_refresh_token = <value>
  --> update fact: pending_google = "<ts>|email"
  --> send: google_step4_message

User sends email
  --> validate: non-empty, trimmed, contains '@' and '.'
  --> read all 3 temp facts
  --> write google.json to ~/.omega/stores/google.json
  --> set file permissions to 0600
  --> cleanup all temp facts + pending_google
  --> audit log
  --> send: google_complete_message
```

#### Public Functions

```rust
impl Gateway {
    /// Start a new `/google` session. Called from pipeline.rs on Command::Google.
    pub(super) async fn start_google_session(
        &self,
        incoming: &IncomingMessage,
    );

    /// Handle a follow-up message during an active google auth session.
    /// Called from pipeline.rs when `pending_google` fact exists.
    pub(super) async fn handle_google_response(
        &self,
        incoming: &IncomingMessage,
        pending_value: &str,
    );
}
```

#### Private Functions

```rust
/// Write the credential JSON file to ~/.omega/stores/google.json.
/// Returns Ok(()) on success, Err(message) on failure.
async fn write_google_credentials(
    data_dir: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    email: &str,
) -> Result<(), String>;

/// Clean up all temporary google auth facts for a sender.
async fn cleanup_google_session(
    memory: &Store,
    sender_id: &str,
);

/// Validate an email address (basic: non-empty, contains '@', has '.' after '@').
fn is_valid_email(email: &str) -> bool;
```

#### Credential File Format (JSON)

```json
{
  "version": 1,
  "client_id": "123456789-abc.apps.googleusercontent.com",
  "client_secret": "GOCSPX-...",
  "refresh_token": "1//0abc...",
  "email": "user@gmail.com"
}
```

- **Path**: `~/.omega/stores/google.json`
- **Permissions**: `0o600` (owner read/write only)
- **Encoding**: UTF-8 JSON, pretty-printed with `serde_json::to_string_pretty`
- **Version field**: `1` (REQ-GAUTH-018, for future extensibility)
- **Keys**: snake_case, matching fact names without prefix

#### File Permissions Strategy

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&path, perms).map_err(|e| format!("chmod: {e}"))?;
}
```

On non-Unix platforms (unlikely for Omega, which targets macOS/Linux), the permission step is skipped silently. The `stores/` directory is already created at startup in `main.rs` (line 293).

#### Failure Modes
| Failure | Cause | Detection | Recovery | Impact |
|---------|-------|-----------|----------|--------|
| Fact storage fails | SQLite error | `store_fact()` returns `Err` | Log warning, send generic error, cleanup | Session aborted, user retries |
| Temp fact read fails at completion | SQLite error or fact missing | `get_fact()` returns `None`/`Err` | Log error, cleanup, send failure message | Credentials lost, user must restart |
| File write fails | Disk full, permission denied | `tokio::fs::write()` error | Send localized error, cleanup facts | Credentials not saved, user retries |
| Permission set fails | OS error | `set_permissions()` error | Log warning, file still written | File accessible to other users (low risk: ~/.omega is user-owned) |
| Session expires mid-flow | User takes >10 min between steps | TTL check on next response | Send expiry message, cleanup all temp facts | User must restart from step 1 |
| Concurrent session attempt | User sends /google while session active | `pending_google` fact check | Send conflict message | Existing session preserved |

#### Security Considerations
- **Trust boundary**: User input is the credential values. They come from chat (already sanitized for injection by pipeline step 2). They are not parsed or interpreted, just stored.
- **Sensitive data**: All 4 credential fields are confidential. They are stored in SQLite facts temporarily (max 10 minutes), then written to `google.json` with restricted permissions. Facts are deleted on all exit paths (completion, cancellation, expiry).
- **Attack surface**: (1) SQLite WAL may retain deleted fact data -- mitigated by `memory.db` sandbox protection. (2) Credentials visible in chat history -- mitigated by OMEGA being private-mode only. (3) File permissions on `google.json` -- mitigated by 0600 mode.
- **Mitigations**: Credentials NEVER enter `store_message()`, NEVER reach AI provider context, NEVER appear in audit logs. Only status strings like `"started"`, `"complete"`, `"cancelled"`, `"expired"` are audited.

#### Performance Budget
- **Latency target**: < 50ms per step (no provider call, just SQLite + file I/O)
- **Memory budget**: Negligible (no large buffers, just string manipulation)
- **Complexity target**: O(1) per step (fixed number of fact reads/writes)

### Module 5: Pipeline Integration (`backend/src/gateway/pipeline.rs`)

- **Responsibility**: Intercept `/google` command and `pending_google` sessions before provider context building.
- **Public interface**: None (changes to existing `handle_message()` method).
- **Dependencies**: `commands::Command::Google`, `google_auth` module.
- **Implementation order**: 5

#### Integration Points (exact locations)

**Point A: Command dispatch section (after `/setup` intercept, before generic command handling)**

Insert at pipeline.rs line ~189 (after the `/setup` intercept block's closing `return;`, before `let ctx = commands::CommandContext`):

```rust
// --- /google intercept (REQ-GAUTH-002) ---
if matches!(cmd, commands::Command::Google) {
    self.start_google_session(&incoming).await;
    return;
}
```

**Point B: Pending session check (after `pending_setup` check, before `pending_discovery` check)**

Insert at pipeline.rs line ~250 (after the `pending_setup` block's `return;`, before the `handle_pending_discovery` call):

```rust
// --- 4a-GOOGLE. PENDING GOOGLE AUTH SESSION CHECK (REQ-GAUTH-011) ---
let pending_google: Option<String> = self
    .memory
    .get_fact(&incoming.sender_id, "pending_google")
    .await
    .ok()
    .flatten();

if let Some(google_value) = pending_google {
    self.handle_google_response(&incoming, &google_value).await;
    return;
}
```

**Critical ordering**: The `pending_google` check MUST be before context building (step 4b+) and before `store_message()` to ensure credentials never enter the AI provider pipeline. This satisfies REQ-GAUTH-011.

#### Why no typing indicator for /google

The `/setup` command starts a typing indicator because it invokes the AI provider (Brain agent), which can take 10-30 seconds. The `/google` command does NOT invoke any AI provider -- it is pure gateway logic (SQLite + file I/O) completing in <50ms. A typing indicator would appear and vanish instantly, providing no UX value.

#### Failure Modes
| Failure | Cause | Detection | Recovery | Impact |
|---------|-------|-----------|----------|--------|
| Pipeline ordering regression | Future refactor moves `pending_google` after context building | Integration test | Fix ordering | Credentials leak to provider context |
| `get_fact` fails for pending_google | SQLite error | `.ok().flatten()` returns `None` | Message falls through to normal pipeline | Credential response sent to AI provider |

#### Security Considerations
- **Trust boundary**: The `pending_google` check acts as a security gate. If it fires, the message NEVER reaches `build_context()` or `store_message()`.
- **Mitigations**: Mandatory comment marking the security-critical ordering. Integration test to verify credentials do not reach provider.

### Module 6: Gateway Module Registration (`backend/src/gateway/mod.rs`)

- **Responsibility**: Register new submodules.
- **Public interface**: None (internal module registration).
- **Dependencies**: None.
- **Implementation order**: 2 (alongside TTL constant)

**Changes:**
```rust
// Add after `setup_response`:
mod google_auth;
mod google_auth_i18n;
```

## Failure Modes (system-level)

| Scenario | Affected Modules | Detection | Recovery Strategy | Degraded Behavior |
|----------|-----------------|-----------|-------------------|-------------------|
| SQLite unavailable | google_auth (all steps) | `store_fact`/`get_fact` errors | Log error, send generic failure message | User cannot start/continue session |
| Disk full | google_auth (completion step) | `tokio::fs::write` error | Send localized error, cleanup facts | Credentials captured but not persisted to file |
| `stores/` directory missing | google_auth (completion) | `write` error | Attempt `create_dir_all` before write | Auto-recovery; fail if directory creation also fails |
| Fact cleanup incomplete | google_auth (all exit paths) | Silent (orphaned facts in SQLite) | Next `/google` session overwrites orphaned facts | No user impact; minor SQLite bloat |
| Pipeline ordering violation | pipeline (pending_google check) | Integration test failure | Revert to correct ordering | Credentials exposed to AI provider |

## Security Model

### Trust Boundaries

- **User input boundary**: Credential values arrive as plain chat messages. They are sanitized for injection patterns (step 2 of pipeline) but are not structurally validated beyond basic non-empty/email-format checks. They are trusted as-is for storage.
- **Provider isolation boundary**: The `pending_google` check in pipeline.rs is the critical security gate. When active, it short-circuits the pipeline before any context building, message storage, or provider invocation. Credentials NEVER cross this boundary.
- **Audit boundary**: Audit entries use the `[GOOGLE_AUTH]` prefix with status-only content. Credential values are never written to the audit log.

### Data Classification

| Data | Classification | Storage | Access Control |
|------|---------------|---------|---------------|
| client_id | Confidential | Temp fact (SQLite, max 10 min) then google.json | Facts: per-sender; File: 0600 |
| client_secret | Confidential | Temp fact (SQLite, max 10 min) then google.json | Facts: per-sender; File: 0600 |
| refresh_token | Confidential | Temp fact (SQLite, max 10 min) then google.json | Facts: per-sender; File: 0600 |
| email | Internal | Temp fact (SQLite, max 10 min) then google.json | Facts: per-sender; File: 0600 |
| pending_google state | Internal | Fact (SQLite) | Per-sender |

### Attack Surface

- **SQLite WAL retention**: Deleted facts may persist in WAL until checkpoint. Risk: low (memory.db is sandbox-protected). Mitigation: sandbox prevents external read.
- **Chat history visibility**: Credentials are visible in chat UI to anyone with physical device access. Risk: medium. Mitigation: OMEGA is private-mode only (single-user channels).
- **File permission race**: Between `write` and `set_permissions`, file briefly has default umask permissions. Risk: very low (single-user system, `~/.omega/` is user-owned). Mitigation: Write to temp file then rename (not implemented -- overhead not justified for single-user).

## Graceful Degradation

| Dependency | Normal Behavior | Degraded Behavior | User Impact |
|-----------|----------------|-------------------|-------------|
| SQLite | Facts stored/read successfully | Error on fact operation | Session fails with error message; user retries |
| Filesystem | google.json written with 0600 | Write fails | Error message; credentials in temp facts cleaned up; user retries |
| `stores/` directory | Already exists (created at startup) | Missing | Auto-created before write attempt |

## Performance Budgets

| Operation | Latency (p50) | Latency (p99) | Memory | Notes |
|-----------|---------------|---------------|--------|-------|
| Start session | < 10ms | < 50ms | < 1KB | 2 fact ops + 1 file existence check |
| Each step | < 10ms | < 50ms | < 1KB | 2-3 fact ops |
| Completion | < 20ms | < 100ms | < 5KB | 3 fact reads + 1 JSON write + 5 fact deletes |

## Data Flow

```
/google command
  --> pipeline.rs: Command::Google intercept
  --> google_auth.rs: start_google_session()
  --> Store: store_fact("pending_google", "<ts>|client_id")
  --> Channel: send step 1 message

Subsequent messages (while pending_google exists):
  --> pipeline.rs: pending_google fact detected (BEFORE context building)
  --> google_auth.rs: handle_google_response()
  --> Store: get_fact, store_fact (temp credentials + state)
  --> Channel: send next step message

Completion (email step):
  --> Store: get_fact x3 (read temp credentials)
  --> Filesystem: write ~/.omega/stores/google.json (0600)
  --> Store: delete_fact x4 (cleanup all temp facts + pending_google)
  --> Audit: log [GOOGLE_AUTH] status only
  --> Channel: send completion message

Cancellation/Expiry:
  --> Store: delete_fact x4 (cleanup all temp facts + pending_google)
  --> Channel: send cancelled/expired message
```

## Design Decisions

| Decision | Alternatives Considered | Justification |
|----------|------------------------|---------------|
| Fact-based state machine | Dedicated SQLite table; in-memory HashMap | Follows `/setup` pattern exactly; no schema migration needed; survives restarts |
| Separate i18n file | Inline in google_auth.rs; use `i18n.rs` module | 500-line limit requires split; dedicated file follows `keywords.rs`/`keywords_data.rs` pattern; i18n functions are self-contained |
| Temp facts with `_` prefix | Single JSON blob fact; in-memory struct | Individual facts allow partial cleanup on failure; `_` prefix is a convention signal |
| `is_build_cancelled()` reuse | Dedicated google cancel keywords | Same cancel vocabulary applies; DRY; already tested in 8 languages |
| No typing indicator | Add typing indicator like /setup | /google is <50ms per step (no provider call); typing indicator would flash and vanish |
| `pending_google` check after `pending_setup` | Before `pending_setup`; combined check | Setup is more complex (AI-backed), should be checked first; ordering matches conceptual priority |
| Basic email validation (@ and .) | Regex; full RFC 5322 | Users paste their own email -- complex validation adds friction without value. Re-prompt on failure is sufficient |
| `serde_json::to_string_pretty` for JSON | Manual string formatting; compact JSON | Pretty-print is human-readable for debugging; serde_json is already a dependency |
| Store pending_google BEFORE sending message | After sending message | If send fails, cleanup is still needed; storing first ensures consistent state |
| 10-minute TTL (not 30) | Match /setup's 30 min TTL | /google is 4 quick paste steps, not a multi-round AI conversation; shorter TTL reduces credential exposure window |

## External Dependencies

- **serde_json** (already in workspace) -- JSON serialization for google.json
- **chrono** (already in workspace) -- timestamp for TTL checks
- **tokio::fs** (already in workspace) -- async file write
- **std::os::unix::fs::PermissionsExt** (stdlib, cfg(unix)) -- 0600 file permissions

No new external dependencies required.

## Requirement Traceability

| Requirement ID | Architecture Section | Module(s) |
|---------------|---------------------|-----------|
| REQ-GAUTH-001 | Module 1: Command Registration | `backend/src/commands/mod.rs` |
| REQ-GAUTH-002 | Module 5: Pipeline Integration, Point A | `backend/src/gateway/pipeline.rs` |
| REQ-GAUTH-003 | Module 4: State Machine Design | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-004 | Module 4: Step transitions (step 1) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-005 | Module 4: Step transitions (step 2) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-006 | Module 4: Step transitions (step 3) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-007 | Module 4: Step transitions (step 4) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-008 | Module 4: Step transitions (completion) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-009 | Module 2: TTL Constant; Module 4: TTL check | `backend/src/gateway/keywords_data.rs`, `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-010 | Module 4: Cancellation (reuses `is_build_cancelled()`) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-011 | Module 5: Pipeline Integration, Point B | `backend/src/gateway/pipeline.rs` |
| REQ-GAUTH-012 | Module 4: Audit logging | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-013 | Module 3: Localized Messages | `backend/src/gateway/google_auth_i18n.rs` |
| REQ-GAUTH-014 | Module 3: `google_step1_message(existing: bool)` | `backend/src/gateway/google_auth_i18n.rs`, `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-015 | Module 4: Concurrent session guard | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-016 | Module 4: `cleanup_google_session()` on all paths | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-017 | Module 4: `is_valid_email()`, empty input check | `backend/src/gateway/google_auth.rs`, `backend/src/gateway/google_auth_i18n.rs` |
| REQ-GAUTH-018 | Module 4: Credential File Format (version field) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-019 | N/A (Won't -- deferred) | N/A |
| REQ-GAUTH-020 | Module 3 + Module 4 split | `backend/src/gateway/google_auth.rs` + `backend/src/gateway/google_auth_i18n.rs` |
