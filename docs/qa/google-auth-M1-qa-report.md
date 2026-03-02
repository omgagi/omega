# QA Report: Google Auth `/google` Command (Milestone 1)

## Scope Validated
- Command registration (`commands/mod.rs`)
- State machine logic (`gateway/google_auth.rs`)
- Localized messages (`gateway/google_auth_i18n.rs`)
- Pipeline integration (`gateway/pipeline.rs`)
- TTL constant (`gateway/keywords_data.rs`)
- Module registration (`gateway/mod.rs`)
- i18n help text (`i18n/commands.rs`)

## Summary
**PASS** -- All 13 Must requirements and all 5 Should requirements are met. The 1 Could requirement (REQ-GAUTH-018) is implemented and verified. The implementation is clean, well-tested (84 google-specific tests, all passing), follows the existing `/setup` pattern, and correctly isolates credentials from the AI provider pipeline. No blocking issues found. Two non-blocking documentation drift items and one style observation are noted below.

## System Entrypoint
- **Test suite**: `nix develop --command bash -c "cargo test --workspace"` -- 1056 tests, 0 failures
- **Google-specific tests**: `cargo test --workspace google` -- 84 tests, 0 failures
- **Clippy**: `cargo clippy --workspace -- -D warnings` -- 0 warnings
- **Note**: The system was not started as a running service for this validation. The `/google` command is a pure gateway state machine (no AI provider, no external services), making it fully validatable through unit/integration tests and structural analysis.

## Traceability Matrix Status

| Requirement ID | Priority | Has Tests | Tests Pass | Acceptance Met | Notes |
|---|---|---|---|---|---|
| REQ-GAUTH-001 | Must | Yes (7 tests) | Yes | Yes | Command parsing, botname stripping, case sensitivity, help inclusion |
| REQ-GAUTH-002 | Must | Yes (structural) | Yes | Yes | `/google` intercepted at pipeline.rs line 192, before generic command dispatch |
| REQ-GAUTH-003 | Must | Yes (4 tests) | Yes | Yes | Fact format `<ts>\|<step>`, all step names, edge cases |
| REQ-GAUTH-004 | Must | Yes (1 test) | Yes | Yes | Step 1 stores `pending_google` with `\|client_id` |
| REQ-GAUTH-005 | Must | Yes (1 test) | Yes | Yes | Step 2 stores `_google_client_id`, advances to `client_secret` |
| REQ-GAUTH-006 | Must | Yes (1 test) | Yes | Yes | Step 3 stores `_google_client_secret`, advances to `refresh_token` |
| REQ-GAUTH-007 | Must | Yes (1 test) | Yes | Yes | Step 4 stores `_google_refresh_token`, advances to `email` |
| REQ-GAUTH-008 | Must | Yes (13 tests) | Yes | Yes | Email validation, file write, permissions (0600), overwrite, missing dir |
| REQ-GAUTH-009 | Must | Yes (5 tests) | Yes | Yes | TTL=600s, boundary conditions, malformed timestamp handling |
| REQ-GAUTH-010 | Must | Yes (2 tests) | Yes | Yes | Reuses `is_build_cancelled()`, no false positives on credentials |
| REQ-GAUTH-011 | Must | Yes (1 structural) | Yes | Yes | `pending_google` check at line 260, `build_context` at line 351 -- 91 lines apart |
| REQ-GAUTH-012 | Must | Yes (2 tests) | Yes | Yes | `[GOOGLE_AUTH]` prefix, status-only strings, no credential values |
| REQ-GAUTH-013 | Should | Yes (17 tests) | Yes | Yes | All 10 functions x 8 languages, English fallback, distinct translations |
| REQ-GAUTH-014 | Should | Yes (3 tests) | Yes | Yes | Overwrite warning conditional, `google_step1_message(_, true)` longer than `false` |
| REQ-GAUTH-015 | Should | Yes (3 tests) | Yes | Yes | Concurrent guard blocks active, allows expired, allows fresh |
| REQ-GAUTH-016 | Should | Yes (4 tests) | Yes | Yes | Cleanup removes all 4 facts, idempotent, no-op on empty, user isolation |
| REQ-GAUTH-017 | Should | Yes (5 tests) | Yes | Yes | Empty/whitespace rejected, email validated, localized error messages |
| REQ-GAUTH-018 | Could | Yes (2 tests) | Yes | Yes | JSON includes `"version": 1`, snake_case keys |
| REQ-GAUTH-019 | Won't | N/A | N/A | N/A | Deferred -- Channel trait has no `delete_message` method |
| REQ-GAUTH-020 | Must | Yes (structural) | Yes | Yes | google_auth.rs: 370 non-test lines; google_auth_i18n.rs: 181 non-test lines |

### Gaps Found
- None. Every Must and Should requirement has tests. Every test passes. Every test ID in the traceability matrix corresponds to an actual test function.

## Acceptance Criteria Results

### Must Requirements

#### REQ-GAUTH-001: `/google` command registration
- [x] `/google` returns `Some(Command::Google)` -- verified by `test_parse_google_command`
- [x] Botname suffix stripped (`/google@omega_bot`) -- verified by `test_parse_google_command_with_botname_suffix`
- [x] Unknown commands unaffected (`/googlefoo` returns `None`) -- verified by `test_parse_googlefoo_does_not_match`
- [x] Case-sensitive (`/Google` returns `None`) -- verified by `test_parse_google_case_sensitive`
- [x] Included in comprehensive command list -- verified by `test_parse_all_commands`
- [x] Help text includes `/google` in all 8 languages -- verified by `test_help_includes_google` and `i18n/commands.rs` line 176

#### REQ-GAUTH-002: Pipeline intercept before provider call
- [x] `Command::Google` intercepted at pipeline.rs line 192 -- verified by code inspection
- [x] Returns early after `start_google_session()` -- confirmed `return;` at line 195
- [x] Placed after `/setup` intercept, before generic command handling -- correct ordering

#### REQ-GAUTH-003: Fact-based state machine
- [x] Fact format `<timestamp>|<step>` -- verified by `test_pending_google_fact_format`
- [x] Valid steps: client_id, client_secret, refresh_token, email -- verified by `test_pending_google_valid_steps`
- [x] Edge cases: extra pipes and missing pipe handled -- verified by tests

#### REQ-GAUTH-004 through REQ-GAUTH-007: Step transitions
- [x] Step 1: Stores `pending_google` with `|client_id` -- PASS
- [x] Step 2: Stores `_google_client_id`, advances to `client_secret` -- PASS
- [x] Step 3: Stores `_google_client_secret`, advances to `refresh_token` -- PASS
- [x] Step 4: Stores `_google_refresh_token`, advances to `email` -- PASS

#### REQ-GAUTH-008: Final step - write credentials
- [x] Email validation (13 test cases covering standard, gmail, plus-tag, empty, whitespace, no-@, no-dot, missing-local, dot-after-@, surrounding-whitespace, multiple-@, unicode) -- PASS
- [x] File written to `<data_dir>/stores/google.json` -- verified by `test_write_google_credentials_creates_file`
- [x] File permissions set to 0600 on Unix -- verified by test (mode & 0o777 == 0o600)
- [x] Missing `stores/` directory auto-created via `create_dir_all` -- verified by `test_write_google_credentials_missing_stores_dir`
- [x] Overwrite of existing file works -- verified by `test_write_google_credentials_overwrites_existing`
- [x] Cleanup of all temp facts after write -- verified in production code (line 316)

#### REQ-GAUTH-009: Session TTL of 10 minutes
- [x] `GOOGLE_AUTH_TTL_SECS = 600` in keywords_data.rs line 394 -- PASS
- [x] Within TTL (5 min) is valid -- PASS
- [x] Past TTL (601s) is expired -- PASS
- [x] At exact boundary (600s) is valid (inclusive) -- PASS
- [x] Malformed timestamp defaults to 0 (immediately expired) -- PASS

#### REQ-GAUTH-010: Cancellation support
- [x] Reuses `is_build_cancelled()` which does exact-match on lowercased input -- PASS
- [x] Cancel keywords detected: "cancel", "no", "cancelar", "annuler", "nein" etc. -- PASS
- [x] No false positives on credential values: "GOCSPX-abc123", "user@gmail.com" etc. -- PASS
- [x] Cleanup called on cancellation (line 197) -- PASS

#### REQ-GAUTH-011: Credentials NEVER enter AI provider pipeline
- [x] `pending_google` check at pipeline.rs line 260 -- BEFORE `build_context()` at line 351
- [x] `pending_google` check returns early (line 269 `return;`) -- no `store_exchange` reached
- [x] `store_exchange` only called in `routing.rs` line 199, which is only reached via `handle_direct_response` at pipeline.rs line 449 -- well after the guard
- [x] Security comment present at pipeline.rs lines 258-259
- [x] CRITICAL: This is the most important security requirement and it is correctly implemented

#### REQ-GAUTH-012: Audit logging without credential values
- [x] `input_text` is always `"[GOOGLE_AUTH]"` (line 355) -- never credential values
- [x] `output_text` is `"[GOOGLE_AUTH] {status}"` where status is one of: started, complete, cancelled, expired, error
- [x] `provider_used` is `None` -- no provider involved
- [x] No credential field names or values appear in audit entries

#### REQ-GAUTH-020: Module stays under 500 lines
- [x] google_auth.rs: 370 non-test lines (1388 total with tests) -- PASS
- [x] google_auth_i18n.rs: 181 non-test lines (535 total with tests) -- PASS

### Should Requirements

#### REQ-GAUTH-013: Localized messages for all 8 languages
- [x] All 10 i18n functions support EN, ES, PT, FR, DE, IT, NL, RU -- verified by 17 tests
- [x] Unknown languages fall back to English via `_` match arm -- verified by 10 "default_english" tests
- [x] Each non-English language produces distinct text -- verified by 7 "differs_from_english" tests

#### REQ-GAUTH-014: Overwrite warning
- [x] `google_step1_message(lang, existing: bool)` includes warning when `true` -- PASS
- [x] Warning is language-specific (8 variants) -- PASS
- [x] File existence check uses `stores_path.exists()` -- PASS

#### REQ-GAUTH-015: Concurrent session guard
- [x] Active session (within TTL) blocks new `/google` -- PASS
- [x] Expired session allows new `/google` (old session cleaned up) -- PASS
- [x] No existing session allows new `/google` -- PASS

#### REQ-GAUTH-016: Temporary credential facts cleanup
- [x] All 4 facts cleaned up: `pending_google`, `_google_client_id`, `_google_client_secret`, `_google_refresh_token` -- PASS
- [x] Idempotent (double cleanup does not panic) -- PASS
- [x] No-op when no facts exist -- PASS
- [x] Does not affect other users' facts -- PASS
- [x] Cleanup called on all exit paths: completion (line 316), cancellation (line 197), expiry (line 188), error (lines 295, 323, 336) -- PASS

#### REQ-GAUTH-017: Input validation with localized errors
- [x] Empty input rejected at all steps (line 207) -- PASS
- [x] Whitespace-only input rejected (trimmed then checked) -- PASS
- [x] Email format validated via `is_valid_email()` -- PASS
- [x] Invalid input re-prompts same step (no state advance) -- PASS
- [x] Localized error messages for empty input and invalid email -- PASS

### Could Requirements

#### REQ-GAUTH-018: Version field in credential JSON
- [x] JSON includes `"version": 1` -- PASS
- [x] Keys are snake_case (not camelCase) -- PASS

## End-to-End Flow Results

| Flow | Steps | Result | Notes |
|---|---|---|---|
| Happy path: /google through completion | 5 (command + 4 credential steps) | PASS | Verified through structural code review and unit tests |
| Cancellation mid-flow | 2 (command + cancel keyword) | PASS | All temp facts cleaned, localized message sent |
| Session expiry | 2 (command + delayed response) | PASS | TTL check on every response, cleanup + expiry message |
| Concurrent session rejection | 2 (command + second /google) | PASS | Conflict message sent, existing session preserved |
| Overwrite warning | 1 (command when google.json exists) | PASS | Warning appended to step 1 message |
| Missing stores directory | 5 (command through completion) | PASS | `create_dir_all` auto-creates before write |
| Invalid email retry | 2+ (command + invalid email + valid email) | PASS | Re-prompts same step without state advance |

## Exploratory Testing Findings

| # | What Was Tried | Expected | Actual | Severity |
|---|---|---|---|---|
| 1 | Cancel keyword "no" as credential value | Should not be a realistic scenario | `is_build_cancelled` would detect "no" as cancellation | Low -- OAuth credentials are never single common words |
| 2 | Cancel keyword "n" as a credential value | Would trigger false cancellation | "n" is in BUILD_CANCEL_KW -- exact match would fire | Low -- same reasoning; "n" is not a valid OAuth value |
| 3 | `unwrap()` calls on lines 308-310 in production code | Should use `?` or pattern matching per project rules | `unwrap()` is used after a `None` guard at line 293 -- safe but violates style rule | Low -- functionally safe, style-only concern |
| 4 | Malformed `pending_google` fact (no pipe separator) | Should handle gracefully | Step is empty string "", falls to `_` arm at line 333, cleanup and error message | None -- handled correctly |
| 5 | `pending_google` fact with future timestamp | TTL check would compute negative delta | `(now - created_at)` would be negative, which is `<= 600`, so session would be "valid" | Low -- only possible if system clock is wrong; no real-world impact |

## Failure Mode Validation

| Failure Scenario | Triggered | Detected | Recovered | Degraded OK | Notes |
|---|---|---|---|---|---|
| SQLite fact storage fails | Untestable (requires SQLite corruption) | Yes -- `Err` on `store_fact` | Yes -- logs warning, sends error message | Yes | Lines 147-156 for start, lines 216-222 for steps |
| Temp fact missing at completion | Untestable directly | Yes -- `is_none()` check at line 293 | Yes -- cleanup + error message + audit "error" | Yes | Lines 293-303 |
| File write fails | Untestable (requires disk full/permission denied) | Yes -- `Err` from `write_google_credentials` | Yes -- cleanup + error message + audit "error" | Yes | Lines 321-330 |
| Permission set fails | Untestable safely | Yes -- `map_err` on `set_permissions` at line 86 | Partial -- file still written but not 0600 | Acceptable -- `~/.omega/` is user-owned | Risk acknowledged in architecture |
| Session expires mid-flow | Tested via TTL logic tests | Yes -- timestamp comparison | Yes -- cleanup + expiry message + audit | Yes | Lines 186-193 |
| Concurrent session attempt | Tested | Yes -- `get_fact` check | Yes -- conflict message sent | Yes -- existing session preserved | Lines 117-136 |
| Unknown step in state machine | Tested via code paths | Yes -- `_` match arm at line 333 | Yes -- cleanup + error message | Yes | Handles gracefully |

## Security Validation

| Attack Surface | Test Performed | Result | Notes |
|---|---|---|---|
| Credentials entering AI provider | Structural analysis of pipeline.rs ordering | PASS | `pending_google` check at line 260, `build_context` at line 351 -- 91 lines of separation with early return |
| Credentials in audit log | Code inspection of `audit_google()` method | PASS | `input_text` is always `"[GOOGLE_AUTH]"`, never credential values. `output_text` is `"[GOOGLE_AUTH] {status}"` |
| Credentials in `store_exchange` | Code trace through pipeline | PASS | `store_exchange` only called in `routing.rs:199` via `handle_direct_response` at pipeline.rs:449, well after the guard |
| File permissions on google.json | Test `test_write_google_credentials_creates_file` | PASS | Verified `mode & 0o777 == 0o600` |
| stores/ directory creation | Test `test_write_google_credentials_missing_stores_dir` | PASS | `create_dir_all` before write |
| Input injection via credentials | Structural analysis | PASS | Credential values are stored as-is in JSON via `serde_json::json!()` macro (proper escaping). Pipeline sanitization runs before command dispatch. |
| SQLite WAL retention of facts | Design review | Out of Scope | Acknowledged risk in architecture. Mitigated by sandbox protection on `memory.db`. Window is max 10 minutes. |
| Cancel keyword false positive | Test `test_cancel_does_not_false_positive_on_credentials` | PASS | Realistic credential values do not match cancel keywords |

## Specs/Docs Drift

| File | Documented Behavior | Actual Behavior | Severity |
|------|-------------------|-----------------|----------|
| `docs/src-gateway-rs.md` (line ~40) | Module table lists all gateway submodules | `google_auth.rs` and `google_auth_i18n.rs` are NOT listed in the module table | Medium -- new developer would not know these modules exist from the docs |
| `specs/google-auth-architecture.md` (line 277) | "On non-Unix platforms, the permission step is skipped silently" | Actual: `set_permissions` returns `Err` which is propagated via `map_err` -- it does not skip silently, it errors | Low -- non-Unix is not a supported platform for Omega, so this mismatch is academic |
| `specs/google-auth-architecture.md` (line 315-316) | Point A insertion at "pipeline.rs line ~189" | Actual: line 192 in current pipeline.rs | Low -- line numbers drift as code evolves; comment intent is correct |
| `specs/google-auth-architecture.md` (line 327) | Point B insertion at "pipeline.rs line ~250" | Actual: line 260 in current pipeline.rs | Low -- same as above |

## Blocking Issues (must fix before merge)
None.

## Non-Blocking Observations
- **[OBS-001]**: `docs/src-gateway-rs.md` module table -- Missing `google_auth.rs` and `google_auth_i18n.rs` entries. Recommend adding them after the `setup_response.rs` row.
- **[OBS-002]**: `google_auth.rs` lines 308-310 -- Three `unwrap()` calls in production code on `client_id.unwrap()`, `client_secret.unwrap()`, `refresh_token.unwrap()`. These are safe (guarded by `is_none()` check at line 293) but violate the project rule "No `unwrap()` -- use `?` and proper error types." Recommend replacing with `.expect("guarded by is_none check")` or destructuring via `if let (Some(cid), Some(cs), Some(rt))`.
- **[OBS-003]**: Cancel keywords as credential values -- If a user's OAuth credential is literally "no", "n", "cancel", "stop", "nein", "non", etc., it would be detected as cancellation. This is extremely unlikely for real OAuth values but worth documenting as a known limitation.

## Modules Not Validated
None. All modules in scope were fully validated.

## Final Verdict

**PASS** -- All 13 Must requirements met. All 5 Should requirements met. The 1 Could requirement is implemented and verified. No blocking issues. 84 google-specific tests pass. Zero clippy warnings. Production code is under 500 lines per file. Credentials are correctly isolated from the AI provider pipeline. Approved for review.
