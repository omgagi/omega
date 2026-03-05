# Architecture: Google Account Setup (`/google` Command)

## Scope

Gateway-layer command for capturing and storing Google OAuth credentials via a chat-based wizard with server-side OAuth token exchange. Affects: `commands/mod.rs`, `gateway/mod.rs`, `gateway/pipeline.rs`, `gateway/keywords_data.rs`, and three gateway files: `gateway/google_auth.rs` (state machine + logic), `gateway/google_auth_i18n.rs` (localized messages for 8 languages), and `gateway/google_auth_oauth.rs` (OAuth URL building, token exchange, email fetch).

Also adds message deletion infrastructure: `platform_message_id` on `IncomingMessage`, `delete_message()` on `Channel` trait, implemented for Telegram.

This is a single-milestone feature. No AI provider involvement -- pure gateway state machine with HTTPS calls to Google's OAuth endpoints.

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
    |-- Store pending_google fact: "<ts>|project_id"
    |-- Send Step 1 prompt (ask for Project ID)
    |
    v
User sends Project ID
    |
    v
google_auth::handle_google_response()
    |-- Store _google_project_id
    |-- Send setup guide with project-specific GCP links
    |-- Ask user to paste downloaded JSON credentials file
    |
    v
User pastes JSON credentials (message deleted for security)
    |-- Extract client_id + client_secret from JSON
    |-- Store _google_client_id + _google_client_secret
    |-- Build OAuth URL with client_id
    |-- Send OAuth URL, ask for auth code
    |
    v
User sends auth code (message deleted for security)
    |-- Exchange code for tokens via HTTPS POST
    |-- Fetch user email from Google userinfo API
    |-- Write google.json, cleanup, send completion
    |
    v
If email fetch fails:
    |-- Store refresh_token, ask user for email (fallback)
    |-- On email received: write google.json, cleanup, complete
```

## Modules

### Module 1: Command Registration (`backend/src/commands/mod.rs`)

- **Responsibility**: `Google` variant in `Command` enum; `/google` parsed.
- No changes needed (already implemented in prior milestone).

### Module 2: TTL Constant (`backend/src/gateway/keywords_data.rs`)

- `GOOGLE_AUTH_TTL_SECS: i64 = 1800` (30 minutes -- increased to allow GCP setup time)

### Module 3: OAuth Helpers (`backend/src/gateway/google_auth_oauth.rs`)

- **Responsibility**: OAuth URL construction, token exchange, email fetching, GCP URL helpers.
- **Public interface**: `pub(super)` functions consumed by `google_auth.rs`.
- **Dependencies**: `reqwest` (HTTP client), `urlencoding` (URL encoding), `serde` (JSON deserialization).

#### Functions

```rust
/// Build Google OAuth authorization URL with offline access + consent prompt.
pub(super) fn build_authorization_url(client_id: &str) -> String;

/// Exchange authorization code for access + refresh tokens.
pub(super) async fn exchange_code_for_tokens(
    client_id: &str, client_secret: &str, code: &str,
) -> Result<TokenResponse, OmegaError>;

/// Fetch authenticated user's email from Google userinfo API.
pub(super) async fn fetch_user_email(access_token: &str) -> Result<String, OmegaError>;

/// GCP Console API Library URL for a project and API.
pub(super) fn gcp_api_library_url(project: &str, api: &str) -> String;

/// GCP Console URL for a path and project.
pub(super) fn gcp_console_url(project: &str, path: &str) -> String;
```

#### OAuth Configuration

- **Redirect URI**: `https://omgagi.ai/oauth/callback/`
- **Scopes**: gmail.modify, calendar, drive, documents, spreadsheets, presentations, forms.body, mail.google.com, tasks, contacts, chat.messages
- **access_type**: `offline` (returns refresh_token)
- **prompt**: `consent` (forces consent screen, ensures refresh_token)

### Module 4: Localized Messages (`backend/src/gateway/google_auth_i18n.rs`)

- **Responsibility**: All user-facing messages for the `/google` flow in 8 languages.
- **Public interface**: `pub(super)` functions consumed by `google_auth.rs`.

#### Function Signatures

```rust
pub(super) fn google_step_project_id_message(lang: &str, existing: bool) -> String;
pub(super) fn google_step_setup_guide_message(lang: &str, project_id: &str) -> String;
pub(super) fn google_invalid_json_message(lang: &str) -> &'static str;
pub(super) fn google_step_auth_code_message(lang: &str, auth_url: &str) -> String;
pub(super) fn google_step_complete_message(lang: &str, email: &str) -> String;
pub(super) fn google_token_exchange_error_message(lang: &str) -> &'static str;
pub(super) fn google_email_fallback_message(lang: &str) -> &'static str;
pub(super) fn google_cancelled_message(lang: &str) -> &'static str;
pub(super) fn google_expired_message(lang: &str) -> &'static str;
pub(super) fn google_conflict_message(lang: &str) -> &'static str;
pub(super) fn google_empty_input_message(lang: &str) -> &'static str;
pub(super) fn google_invalid_email_message(lang: &str) -> &'static str;
// + error messages (start_error, store_error, missing_data, write_error, unknown_step)
```

### Module 5: State Machine (`backend/src/gateway/google_auth.rs`)

- **Responsibility**: Session lifecycle: start, step transitions, OAuth exchange, validation, credential storage, message deletion, cleanup, audit.
- **Public interface**: Two `impl Gateway` methods: `start_google_session()`, `handle_google_response()`.

#### State Machine Design

**Fact-based state** (same pattern as `/setup`):

```
<unix_timestamp>|<step>
```

Steps: `project_id`, `setup_guide`, `auth_code`, `email_fallback`.

**Temporary credential facts:**

| Fact key | Stored after | Contains |
|----------|-------------|----------|
| `_google_project_id` | Project ID received | GCP project ID |
| `_google_client_id` | JSON credentials parsed | OAuth client ID |
| `_google_client_secret` | JSON credentials parsed | OAuth client secret |
| `_google_refresh_token` | Email fallback only | Refresh token (from token exchange) |

**Message deletion:** JSON credentials and auth code messages are deleted from chat after capture (best-effort via `delete_user_message()`).

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

### Module 6: Message Deletion Infrastructure

- **`IncomingMessage.platform_message_id`**: Optional field set by Telegram polling (message_id).
- **`Channel::delete_message()`**: Default no-op trait method; Telegram implementation calls `deleteMessage` API.
- **`Gateway::delete_user_message()`**: Best-effort helper that deletes the user's incoming message.
- **Telegram `register_commands()`**: `/google` added to bot command menu.

## Security Model

### Credential Message Deletion

Sensitive messages (JSON credentials, auth_code) are deleted from chat after capture:
- Telegram: Bot API `deleteMessage` (works in private chats, messages < 48h old)
- Best-effort: failures logged but don't block the flow
- WhatsApp: Not implemented (no delete API available)

### Provider Isolation

The `pending_google` check in pipeline.rs short-circuits before context building. Credentials NEVER cross this boundary.

### Audit

Entries use `[GOOGLE_AUTH]` prefix with status-only content. Credential values never logged.

## Design Decisions

| Decision | Alternatives Considered | Justification |
|----------|------------------------|---------------|
| Server-side OAuth token exchange | Ask user for raw refresh_token | Refresh tokens are hard to obtain manually; OAuth code exchange is standard |
| 30-minute TTL (up from 10) | Keep 10 min; 60 min | GCP setup (APIs, consent, credentials) takes time; 30 min balances UX and security |
| Project-specific GCP links in setup guide | Generic instructions | Direct links reduce user friction and errors |
| Delete credential messages | Leave in chat | Security: credentials visible in chat history are a risk |
| Email auto-detection via userinfo API | Always ask user | Better UX; fallback to manual entry if API fails |
| `urlencoding` crate for URL construction | Manual percent encoding | Correct, maintained, zero-dep crate |

## External Dependencies

- **serde_json** (already in workspace) -- JSON serialization
- **chrono** (already in workspace) -- timestamp for TTL
- **tokio::fs** (already in workspace) -- async file I/O
- **reqwest** (already in workspace) -- HTTP client for OAuth endpoints
- **urlencoding** (new, v2) -- URL percent-encoding for OAuth URL construction
