# Requirements: Google Account Setup (`/google` Command)

## Scope

### Domains Affected
- **Commands** (`backend/src/commands/mod.rs`) -- `Google` variant in `Command` enum
- **Gateway pipeline** (`backend/src/gateway/pipeline.rs`) -- intercept `/google` and `pending_google` sessions
- **Gateway module** (`backend/src/gateway/mod.rs`) -- register `google_auth`, `google_auth_i18n`, `google_auth_oauth`
- **Keywords data** (`backend/src/gateway/keywords_data.rs`) -- `GOOGLE_AUTH_TTL_SECS` constant (1800s)
- **Core types** (`omega-core/src/message.rs`) -- `platform_message_id` field on `IncomingMessage`
- **Core traits** (`omega-core/src/traits.rs`) -- `delete_message()` default method on `Channel`
- **Telegram channel** (`omega-channels/src/telegram/`) -- `delete_message` impl, `platform_message_id` set in polling, `/google` in bot commands
- **NEW module** (`backend/src/gateway/google_auth.rs`) -- 5-step state machine with OAuth token exchange
- **NEW module** (`backend/src/gateway/google_auth_i18n.rs`) -- localized messages for 8 languages
- **NEW module** (`backend/src/gateway/google_auth_oauth.rs`) -- OAuth URL building, token exchange, email fetch

### Filesystem Affected
- `~/.omega/stores/google.json` -- credential storage file (created by this feature)

## Summary (plain language)

Add a `/google` gateway command that guides users through a 5-step chat-based wizard to configure their Google account credentials. The wizard collects a GCP Project ID, shows a comprehensive setup guide with project-specific links, collects Client ID and Client Secret, then handles the OAuth flow server-side: builds an authorization URL, exchanges the auth code for tokens, and auto-detects the user's email. Credential messages (client_secret, auth_code) are deleted from chat for security. The entire flow stays in the gateway layer -- credentials never reach the AI provider.

## User Stories

- As a **user**, I want to type `/google` so that OMEGA walks me through configuring my Google account with a guided wizard including direct GCP links.
- As a **user**, I want to just paste an authorization code instead of manually obtaining a refresh token.
- As a **security-conscious user**, I want my credential messages deleted from chat after capture.
- As a **security-conscious user**, I want my Google credentials to never pass through the AI provider.
- As a **user in any of 8 supported languages**, I want the `/google` setup messages in my preferred language.
- As a **user who makes mistakes**, I want to cancel the `/google` setup at any step.
- As a **user who gets distracted**, I want the session to expire after 30 minutes.
- As a **user who already configured Google**, I want `/google` to warn me that existing credentials will be overwritten.

## Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|------------|----------|-------------------|
| REQ-GAUTH-001 | `/google` command registration in `Command::parse()` | Must | `/google` returns `Some(Command::Google)` |
| REQ-GAUTH-002 | `/google` intercepted in `pipeline.rs` before provider call | Must | Starts google auth session, returns early |
| REQ-GAUTH-003 | `pending_google` fact-based state machine with steps | Must | State stored as `<timestamp>\|<step>`, each response advances |
| REQ-GAUTH-004 | Step 1: Ask for Project ID, show GCP creation link | Must | On `/google`, show instructions and ask for Project ID |
| REQ-GAUTH-005 | Step 2: Show setup guide with project-specific API/consent/credential links | Must | Dynamic URLs using project ID, ask for Client ID |
| REQ-GAUTH-006 | Step 3: Receive Client ID, ask for Client Secret | Must | Store Client ID, delete message not needed (not sensitive) |
| REQ-GAUTH-007 | Step 4: Receive Client Secret, build OAuth URL, ask for auth code | Must | Delete secret message, build URL with scopes/offline/consent |
| REQ-GAUTH-008 | Step 5: Receive auth code, exchange for tokens, fetch email, complete | Must | Delete code message, HTTPS token exchange, auto-detect email, write google.json |
| REQ-GAUTH-009 | Session TTL of 30 minutes | Must | `GOOGLE_AUTH_TTL_SECS` = 1800, expired sessions cleaned up |
| REQ-GAUTH-010 | Cancellation support at any step | Must | Reuse cancel keyword pattern, delete all temp facts |
| REQ-GAUTH-011 | Credentials NEVER enter AI provider pipeline | Must | `pending_google` check before context building |
| REQ-GAUTH-012 | Audit logging without credential values | Must | `[GOOGLE_AUTH]` prefix, status-only |
| REQ-GAUTH-013 | Localized messages for all 8 languages | Should | EN/ES/PT/FR/DE/IT/NL/RU for all prompts |
| REQ-GAUTH-014 | Overwrite warning when `google.json` already exists | Should | Check file existence, include warning |
| REQ-GAUTH-015 | Concurrent session guard (one session per user) | Should | Reject new `/google` if `pending_google` exists and not expired |
| REQ-GAUTH-016 | Temporary credential facts cleaned up on all exit paths | Should | Delete all temp facts on completion, cancellation, and expiry |
| REQ-GAUTH-017 | Input validation with localized error messages | Should | Empty/whitespace rejected, email format validated |
| REQ-GAUTH-018 | Credential file JSON includes version field | Could | JSON includes `"version": 1` |
| REQ-GAUTH-019 | Delete credential messages from chat after capture | Must | Client secret and auth code messages deleted (best-effort) |
| REQ-GAUTH-020 | Module stays under 500 lines (excluding tests) | Must | Split into google_auth.rs + google_auth_i18n.rs + google_auth_oauth.rs |
| REQ-GAUTH-021 | Email auto-detection with fallback | Should | Fetch email from Google userinfo API; if fails, ask user manually |
| REQ-GAUTH-022 | `platform_message_id` on IncomingMessage | Must | Telegram sets it from message_id; other channels set None |
| REQ-GAUTH-023 | `delete_message()` on Channel trait | Must | Default no-op; Telegram implements via Bot API deleteMessage |
| REQ-GAUTH-024 | `/google` registered in Telegram bot commands | Should | Appears in autocomplete menu |

| REQ-GAUTH-025 | `GOOGLE_SETUP` marker triggers setup wizard autonomously | Must | AI emits `GOOGLE_SETUP` on its own line; gateway calls `start_google_session()` |
| REQ-GAUTH-026 | `GOOGLE_SETUP` in safety net `strip_all_remaining_markers()` | Must | Marker stripped even if primary processing misses it |
| REQ-GAUTH-027 | Google keywords in `META_KW` for conditional prompt injection | Must | `google`, `gmail`, `calendar`, `drive` trigger meta section |

## Impact Analysis

### Existing Code Affected
- `omega-core/src/message.rs`: New `platform_message_id` field -- Risk: low (serde default)
- `omega-core/src/traits.rs`: New `delete_message()` default method -- Risk: low (no-op default)
- `omega-channels/src/telegram/polling.rs`: Set `platform_message_id`, impl `delete_message` -- Risk: low
- `omega-channels/src/telegram/send.rs`: Add `delete_message_by_id()`, `/google` in commands -- Risk: low
- `backend/src/gateway/mod.rs`: Add `delete_user_message()` helper, register module -- Risk: low
- `backend/src/gateway/keywords_data.rs`: TTL changed from 600 to 1800 -- Risk: low
- All `IncomingMessage` construction sites: Add `platform_message_id: None/Some(...)` -- Risk: low

### Regression Risk Areas
- `/setup` sessions: Different fact names; no conflict -- Risk: low
- Existing google.json format: Same JSON structure; compatible -- Risk: low
