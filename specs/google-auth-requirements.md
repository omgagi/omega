# Requirements: Google Account Setup (`/google` Command)

## Scope

### Domains Affected
- **Commands** (`backend/src/commands/mod.rs`) -- add `Google` variant to `Command` enum, match `/google`
- **Gateway pipeline** (`backend/src/gateway/pipeline.rs`) -- intercept `/google` command and `pending_google` sessions before provider call
- **Gateway module** (`backend/src/gateway/mod.rs`) -- register new `google_auth` submodule
- **Keywords data** (`backend/src/gateway/keywords_data.rs`) -- `GOOGLE_AUTH_TTL_SECS` constant
- **NEW module** (`backend/src/gateway/google_auth.rs`) -- state machine, credential storage, i18n messages, audit

### Files NOT Modified (consumed as-is)
- `backend/crates/omega-core/src/traits.rs` -- Channel trait (no `delete_message` method exists; credential message deletion deferred)
- `backend/crates/omega-core/src/message.rs` -- IncomingMessage struct (no platform message_id field)
- `backend/src/gateway/setup.rs` -- `/setup` pattern is the architectural template but not modified
- `backend/src/gateway/keywords.rs` -- no changes needed; i18n functions live in `google_auth.rs`
- `backend/crates/omega-memory/src/audit.rs` -- existing AuditLogger used as-is

### Filesystem Affected
- `~/.omega/stores/google.json` -- credential storage file (created by this feature)

## Summary (plain language)

Add a `/google` gateway command that guides users through a 4-step chat-based wizard to configure their Google account credentials (OAuth client ID, client secret, refresh token, and Gmail address). The entire flow stays in the gateway layer -- credentials never reach the AI provider. The command follows the same state machine pattern as `/setup` (fact-based session with TTL, cancellation, i18n), but is simpler: no AI involvement, no context files, just sequential prompts. Credentials are written to `~/.omega/stores/google.json` with restrictive file permissions.

## User Stories

- As a **user**, I want to type `/google` so that OMEGA walks me through configuring my Google account credentials step by step.
- As a **security-conscious user**, I want my Google credentials to never pass through the AI provider so that they are not leaked to any third party.
- As a **user in any of 8 supported languages**, I want the `/google` setup messages in my preferred language so the experience feels native.
- As a **user who makes mistakes**, I want to be able to cancel the `/google` setup at any step so that I am not locked into a partially completed flow.
- As a **user who gets distracted**, I want the session to automatically expire after 10 minutes so that a stale session does not block my other messages.
- As a **user who already configured Google**, I want `/google` to warn me that existing credentials will be overwritten so I do not lose my current setup accidentally.

## Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|------------|----------|-------------------|
| REQ-GAUTH-001 | `/google` command registration in `Command::parse()` | Must | `/google` returns `Some(Command::Google)`, botname suffix stripped, unknown commands unaffected |
| REQ-GAUTH-002 | `/google` intercepted in `pipeline.rs` before provider call | Must | Intercept in command dispatch section, starts google auth session, returns early |
| REQ-GAUTH-003 | `pending_google` fact-based state machine with steps | Must | State stored as fact: `<timestamp>\|<step>`, each response advances to next step |
| REQ-GAUTH-004 | Step 1: Prompt for client_id, store pending state | Must | On `/google`, respond with instructions and ask for client_id, store `pending_google` fact |
| REQ-GAUTH-005 | Step 2: Receive client_id, prompt for client_secret | Must | Validate non-empty, store as fact, advance step |
| REQ-GAUTH-006 | Step 3: Receive client_secret, prompt for refresh_token | Must | Validate non-empty, store as fact, advance step |
| REQ-GAUTH-007 | Step 4: Receive refresh_token, prompt for email | Must | Validate non-empty, store as fact, advance step |
| REQ-GAUTH-008 | Step 5: Receive email, write credentials, complete | Must | Validate email, write JSON to `~/.omega/stores/google.json`, set 0600 perms, cleanup facts |
| REQ-GAUTH-009 | Session TTL of 10 minutes | Must | `GOOGLE_AUTH_TTL_SECS` = 600, expired sessions cleaned up, localized expiry message |
| REQ-GAUTH-010 | Cancellation support at any step | Must | Reuse cancel keyword pattern, delete all temp facts, localized message |
| REQ-GAUTH-011 | Credentials NEVER enter AI provider pipeline | Must | `pending_google` check before context building, no `store_message()` call |
| REQ-GAUTH-012 | Audit logging without credential values | Must | `[GOOGLE_AUTH]` prefix, never actual credentials, status-only output |
| REQ-GAUTH-013 | Localized messages for all 8 languages | Should | EN/ES/PT/FR/DE/IT/NL/RU for all prompts, errors, completion, cancellation, expiry |
| REQ-GAUTH-014 | Overwrite warning when `google.json` already exists | Should | Check file existence, include warning in initial message |
| REQ-GAUTH-015 | Concurrent session guard (one session per user) | Should | Reject new `/google` if `pending_google` exists and not expired |
| REQ-GAUTH-016 | Temporary credential facts cleaned up on all exit paths | Should | Delete all temp facts on completion, cancellation, and expiry; idempotent |
| REQ-GAUTH-017 | Input validation with localized error messages | Should | Empty/whitespace rejected, email format validated, invalid input re-prompts same step |
| REQ-GAUTH-018 | Credential file JSON includes version field | Could | JSON includes `"version": 1` for future extensibility |
| REQ-GAUTH-019 | Delete credential messages from chat after setup | Won't | Deferred -- Channel trait has no `delete_message` method |
| REQ-GAUTH-020 | Module stays under 500 lines (excluding tests) | Must | Split i18n into separate file if needed |

## Traceability Matrix

| Requirement ID | Priority | Test IDs | Architecture Section | Implementation Module |
|---------------|----------|----------|---------------------|---------------------|
| REQ-GAUTH-001 | Must | T-GAUTH-001a..g (commands/tests.rs: test_parse_google_command, test_parse_google_command_with_botname_suffix, test_parse_googlefoo_does_not_match, test_parse_google_case_sensitive, test_parse_google_registered_in_command_enum, test_help_includes_google, test_google_command_handle_fallback_returns_help, test_parse_all_commands includes /google) | Module 1: Command Registration | `backend/src/commands/mod.rs` |
| REQ-GAUTH-002 | Must | T-GAUTH-002 (pipeline integration -- verified structurally by test_pending_google_check_precedes_context_building) | Module 5: Pipeline Integration, Point A | `backend/src/gateway/pipeline.rs` |
| REQ-GAUTH-003 | Must | T-GAUTH-003a..d (google_auth: test_pending_google_fact_format, test_pending_google_valid_steps, test_pending_google_fact_extra_pipes, test_pending_google_fact_no_pipe) | Module 4: State Machine Design | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-004 | Must | T-GAUTH-004 (google_auth: test_step1_stores_pending_google_fact) | Module 4: Step transitions (step 1) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-005 | Must | T-GAUTH-005 (google_auth: test_step2_stores_client_id_and_advances) | Module 4: Step transitions (step 2) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-006 | Must | T-GAUTH-006 (google_auth: test_step3_stores_client_secret_and_advances) | Module 4: Step transitions (step 3) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-007 | Must | T-GAUTH-007 (google_auth: test_step4_stores_refresh_token_and_advances) | Module 4: Step transitions (step 4) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-008 | Must | T-GAUTH-008a..m (google_auth: test_is_valid_email_standard, test_is_valid_email_gmail, test_is_valid_email_plus_tag_subdomain, test_is_valid_email_empty, test_is_valid_email_whitespace_only, test_is_valid_email_no_at_sign, test_is_valid_email_no_dot_after_at, test_is_valid_email_missing_local_part, test_is_valid_email_dot_immediately_after_at, test_is_valid_email_with_surrounding_whitespace, test_is_valid_email_multiple_at_signs, test_write_google_credentials_creates_file, test_write_google_credentials_missing_stores_dir, test_write_google_credentials_overwrites_existing) | Module 4: Step transitions (completion) | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-009 | Must | T-GAUTH-009a..e (google_auth: test_session_within_ttl_is_valid, test_session_past_ttl_is_expired, test_session_at_exact_ttl_boundary, test_ttl_timestamp_extraction_from_fact, test_ttl_malformed_timestamp_defaults_to_zero) | Module 2: TTL Constant; Module 4: TTL check | `backend/src/gateway/keywords_data.rs`, `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-010 | Must | T-GAUTH-010a..b (google_auth: test_cancel_detection_reuses_is_build_cancelled, test_cancel_does_not_false_positive_on_credentials) | Module 4: Cancellation | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-011 | Must | T-GAUTH-011 (google_auth: test_pending_google_check_precedes_context_building) | Module 5: Pipeline Integration, Point B | `backend/src/gateway/pipeline.rs` |
| REQ-GAUTH-012 | Must | T-GAUTH-012a..b (google_auth: test_audit_google_auth_prefix_format, test_audit_status_strings) | Module 4: Audit logging | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-013 | Should | T-GAUTH-013a..q (google_auth_i18n: test_google_step1_message_all_languages_no_overwrite, test_google_step1_message_all_languages_with_overwrite, test_google_step2_message_all_languages, test_google_step3_message_all_languages, test_google_step4_message_all_languages, test_google_complete_message_all_languages, test_google_cancelled_message_all_languages, test_google_expired_message_all_languages, test_google_conflict_message_all_languages, test_google_empty_input_message_all_languages, test_google_invalid_email_message_all_languages, test_step*_message_default_english x10, test_*_differs_from_english x7) | Module 3: Localized Messages | `backend/src/gateway/google_auth_i18n.rs` |
| REQ-GAUTH-014 | Should | T-GAUTH-014a..c (google_auth_i18n: test_google_step1_message_overwrite_warning_present; google_auth: test_google_json_existence_check_exists, test_google_json_existence_check_not_exists) | Module 3: `google_step1_message(existing)` | `backend/src/gateway/google_auth_i18n.rs`, `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-015 | Should | T-GAUTH-015a..c (google_auth: test_concurrent_session_guard_blocks_new_session, test_concurrent_session_guard_allows_expired_session, test_concurrent_session_guard_no_existing_session) | Module 4: Concurrent session guard | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-016 | Should | T-GAUTH-016a..d (google_auth: test_cleanup_google_session_removes_all_facts, test_cleanup_google_session_idempotent, test_cleanup_google_session_no_facts_exist, test_cleanup_does_not_affect_other_users) | Module 4: `cleanup_google_session()` | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-017 | Should | T-GAUTH-017a..c (google_auth: test_empty_input_rejected, test_whitespace_only_input_rejected, test_valid_input_accepted; google_auth_i18n: test_google_empty_input_message_all_languages, test_google_invalid_email_message_all_languages) | Module 4: `is_valid_email()`, empty check | `backend/src/gateway/google_auth.rs`, `backend/src/gateway/google_auth_i18n.rs` |
| REQ-GAUTH-018 | Could | T-GAUTH-018a..b (google_auth: test_credential_json_format_includes_version, test_credential_json_uses_snake_case_keys) | Module 4: Credential File Format | `backend/src/gateway/google_auth.rs` |
| REQ-GAUTH-019 | Won't | N/A | N/A | N/A |
| REQ-GAUTH-020 | Must | Structural (verified by file count -- google_auth.rs + google_auth_i18n.rs both under 500 lines) | Module 3 + Module 4 split | `backend/src/gateway/google_auth.rs` + `backend/src/gateway/google_auth_i18n.rs` |

## Impact Analysis

### Existing Code Affected
- `backend/src/commands/mod.rs`: Add `Google` variant and match arm -- Risk: low
- `backend/src/gateway/mod.rs`: Add `mod google_auth;` -- Risk: low
- `backend/src/gateway/pipeline.rs`: Add `/google` intercept and `pending_google` check -- Risk: medium
- `backend/src/gateway/keywords_data.rs`: Add `GOOGLE_AUTH_TTL_SECS` constant -- Risk: low

### Regression Risk Areas
- `/setup` sessions: Both use fact-based state machines with different fact names; no conflict -- Risk: low
- Active senders buffer: Google auth responses flow through `dispatch_message()` normally -- Risk: low
- Audit log: Uses distinct `[GOOGLE_AUTH]` prefix -- Risk: low

## Identified Risks

- **Credentials in SQLite WAL**: During session, credential values exist as facts. SQLite WAL may retain data after deletion. Mitigation: `memory.db` is sandbox-protected; window is max 10 minutes.
- **Group chat exposure**: If used in a group, other participants see pasted credentials. Mitigation: OMEGA is private-mode only.
- **Pipeline ordering regression**: Future refactoring might move `pending_google` check after provider context building. Mitigation: mandatory comment + integration test.

## Assumptions

| # | Assumption | Confirmed |
|---|-----------|-----------|
| 1 | Temporary credential values in facts table are acceptable for session duration | Yes |
| 2 | `is_build_cancelled()` is reusable for google auth cancellation | Yes |
| 3 | File permission 0600 works on both macOS and Linux | Yes |
| 4 | No `delete_message` capability exists in Channel trait | Yes |
| 5 | `pending_google` fact name does not conflict with other pending facts | Yes |
| 6 | JSON file uses snake_case keys | Yes |
