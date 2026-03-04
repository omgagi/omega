# QA Report: omega uninstall (Milestone M1)

## Scope Validated
- `backend/src/uninstall.rs` -- full implementation (593 lines, 383 non-test)
- `backend/src/main.rs` -- CLI integration (mod declaration, Commands::Uninstall variant, match arm)
- `backend/src/service.rs` -- visibility changes (service_file_path, stop_service, is_running)
- `specs/uninstall-requirements.md` -- requirements with 16 IDs (REQ-UNINST-001 through REQ-UNINST-016)
- `specs/uninstall-architecture.md` -- architecture design
- `specs/SPECS.md` -- index updated

## Summary
**PASS** -- All Must and Should requirements are met. The implementation faithfully follows the architecture design. All 9 unit tests pass. The build compiles cleanly with zero clippy warnings and correct formatting. The `omega uninstall` command appears in `omega --help`. Two non-blocking documentation drift issues were found (cli-functionalities.md and src-main-rs.md do not yet document the new `omega uninstall` top-level command).

## System Entrypoint
- **Build**: `cd backend && nix develop --command bash -c "cargo build --release"` -- compiles successfully
- **Test suite**: `cd backend && nix develop --command bash -c "cargo test --workspace"` -- 41 passed, 0 failed
- **Clippy**: `cd backend && nix develop --command bash -c "cargo clippy --workspace"` -- zero warnings
- **Format**: `cd backend && nix develop --command bash -c "cargo fmt --check"` -- clean
- **Help output**: `~/.cargo/target-global/release/omega --help` -- lists `uninstall` command with doc comment "Completely remove OMEGA from this system"

## Traceability Matrix Status

| Requirement ID | Priority | Has Tests | Tests Pass | Acceptance Met | Notes |
|---|---|---|---|---|---|
| REQ-UNINST-001 | Must | Yes (integration) | Yes | Yes | `omega --help` lists `uninstall`; main.rs has `mod uninstall`, `Commands::Uninstall`, match arm |
| REQ-UNINST-002 | Must | Yes (test_uninstall_mode_from_str) | Yes | Yes | `cliclack::select` with "complete" and "keep" items; `UninstallMode::from()` handles mapping |
| REQ-UNINST-003 | Must | No (relies on service.rs) | N/A | Yes | `step_stop_service()` calls `service::is_running()` then `service::stop_service()`; graceful skip if not running |
| REQ-UNINST-004 | Must | No (relies on service.rs) | N/A | Yes | `step_remove_service_file()` calls `service::service_file_path()` + `fs::remove_file()`; graceful skip if missing |
| REQ-UNINST-005 | Must | Yes (test_step_remove_data_dir_complete_mode) | Yes | Yes | `fs::remove_dir_all()` on `~/.omega/`; verified in tempdir |
| REQ-UNINST-006 | Must | Yes (test_step_remove_data_dir_keep_config_mode) | Yes | Yes | Iterates KEEP_CONFIG_SUBDIRS (9 dirs); config.toml verified preserved in test |
| REQ-UNINST-007 | Must | Yes (test_step_remove_symlink_existing_file, test_step_remove_symlink_nonexistent_path) | Yes | Yes | `step_remove_symlink("/usr/local/bin/omega", ...)` with PermissionDenied handling |
| REQ-UNINST-008 | Must | No (interactive) | N/A | Yes | `cliclack::confirm().initial_value(false)`; cancellation calls `omega_outro_cancel` and returns Ok |
| REQ-UNINST-009 | Should | Yes (scan_artifacts tests x3) | Yes | Yes | `scan_artifacts()` builds list from existing paths; `display_summary()` shows delete/preserved labels |
| REQ-UNINST-010 | Should | No (visual) | N/A | Yes | Code uses `omega_step`, `omega_success`, `omega_info`, `omega_warning`, `omega_outro`, `omega_outro_cancel` -- all branded helpers |
| REQ-UNINST-011 | Should | Yes (reuses step_remove_symlink tests) | Yes | Yes | `step_remove_symlink("/usr/local/bin/omg-gog", "omg-gog binary", ...)` called in run() |
| REQ-UNINST-012 | Should | Yes (test_uninstall_result_warning_accumulation) | Yes | Yes | `UninstallResult` accumulates warnings; each step is independent; outro reports warning count |
| REQ-UNINST-013 | Should | No (Linux-only, uses cfg!) | N/A | Yes | `step_daemon_reload()` with `cfg!(target_os = "linux")` guard; handles success, non-zero, and command failure |
| REQ-UNINST-014 | Could | No | N/A | Yes | Outro message: "Config preserved at ~/.omega/config.toml" when mode == KeepConfig |
| REQ-UNINST-015 | Won't | N/A | N/A | N/A | Deferred -- no `--yes`/`--force` flag |
| REQ-UNINST-016 | Won't | N/A | N/A | N/A | Deferred -- no backup before deletion |

### Gaps Found
- REQ-UNINST-003 and REQ-UNINST-004 do not have dedicated unit tests for the uninstall integration with service.rs. This is acceptable because: (a) the service functions are already tested in service.rs, and (b) the step functions in uninstall.rs are thin wrappers that delegate to service.rs. The step functions also depend on subprocess calls (launchctl/systemctl) that cannot be meaningfully unit-tested.
- REQ-UNINST-008 (confirmation prompt) is inherently interactive and cannot be unit-tested without mocking cliclack. This is acceptable.
- REQ-UNINST-010 (branded output) is visual and verified by code inspection. The correct helper functions are called.
- REQ-UNINST-013 (daemon-reload) is Linux-only and verified by code inspection. The `cfg!` guard is correct.

## Acceptance Criteria Results

### Must Requirements

#### REQ-UNINST-001: Top-level CLI subcommand
- [x] `omega uninstall` is a valid command -- verified via `omega --help` output
- [x] `omega --help` lists `uninstall` with description "Completely remove OMEGA from this system"
- [x] No config file required -- `run()` reads only HOME env var, not config.toml

#### REQ-UNINST-002: Two-mode selection
- [x] Interactive select prompt offers exactly two options ("Complete removal" and "Keep configuration")
- [x] "complete" string maps to `UninstallMode::Complete` (verified by test_uninstall_mode_from_str)
- [x] "keep" string maps to `UninstallMode::KeepConfig` (verified by test_uninstall_mode_from_str)
- [x] Unknown strings default to Complete (safe default, verified by test)

#### REQ-UNINST-003: Stop running service
- [x] `step_stop_service()` checks `service::is_running()` first
- [x] Only calls `stop_service()` when running
- [x] Non-running service: silently skipped (no warning)
- [x] Service file path failure: warning added to result

#### REQ-UNINST-004: Remove service file
- [x] `step_remove_service_file()` resolves path via `service::service_file_path()`
- [x] Non-existent service file: early return, no error
- [x] Removal failure: warning added to result with specific error message

#### REQ-UNINST-005: Complete removal of ~/.omega/
- [x] `step_remove_data_dir()` with Complete mode calls `fs::remove_dir_all()` on ~/.omega/
- [x] Non-existent directory: early return, no error
- [x] Removal failure: warning added to result (verified by code inspection)
- [x] Test verifies directory is fully removed

#### REQ-UNINST-006: Keep-config removal of ~/.omega/
- [x] Iterates KEEP_CONFIG_SUBDIRS: data, logs, workspace, stores, prompts, skills, projects, topologies, whatsapp_session -- all 9 subdirectories from requirements
- [x] config.toml preserved -- test explicitly asserts `omega_dir.join("config.toml").exists()`
- [x] Missing subdirectories skipped without error
- [x] ~/.omega/ directory itself preserved

#### REQ-UNINST-007: Remove binary symlink
- [x] Calls `step_remove_symlink("/usr/local/bin/omega", ...)`
- [x] Non-existent symlink: early return, no warning
- [x] Permission denied: specific warning with `sudo rm` command
- [x] Other errors: warning with error message

#### REQ-UNINST-008: Final confirmation
- [x] `cliclack::confirm("Proceed with uninstall?").initial_value(false)` -- default is No
- [x] On false/cancel: calls `omega_outro_cancel("Uninstall cancelled")` and returns Ok(())
- [x] Nothing deleted when cancelled

### Should Requirements

#### REQ-UNINST-009: Pre-deletion summary
- [x] `scan_artifacts()` checks `Path::exists()` for each known artifact
- [x] Non-existing artifacts excluded from list
- [x] `display_summary()` shows "(delete)" for items to remove and "(preserved)" for kept items
- [x] In keep-config mode: config.toml shown as preserved, subdirectories shown as delete
- [x] In complete mode: single ~/.omega/ entry shown

#### REQ-UNINST-010: Branded CLI output
- [x] Uses `omega_step` for phase announcements (intro, summary header, "Stopping service...")
- [x] Uses `omega_success` for each deleted artifact
- [x] Uses `omega_warning` for skipped/failed items
- [x] Uses `omega_info` for summary items to be deleted, and "No Omega artifacts found"
- [x] Uses `omega_outro` / `omega_outro_cancel` for completion/cancellation messages

#### REQ-UNINST-011: Remove omg-gog binary
- [x] `step_remove_symlink("/usr/local/bin/omg-gog", "omg-gog binary", ...)` called
- [x] Shares same symlink removal logic with permission-denied handling
- [x] Non-existent: silently skipped

#### REQ-UNINST-012: Partial failure handling
- [x] Each deletion step is independent -- all 6 steps run regardless of prior failures
- [x] `UninstallResult` accumulates warnings via `warn()` method
- [x] Outro: "Uninstall completed with N warning(s)" when warnings present
- [x] Test verifies warning accumulation works correctly

#### REQ-UNINST-013: Systemd daemon-reload
- [x] `step_daemon_reload()` guarded by `cfg!(target_os = "linux")`
- [x] No-op on macOS (returns immediately)
- [x] On Linux: runs `systemctl --user daemon-reload`
- [x] Handles success, non-zero exit, and command failure -- each adds appropriate warning

### Could Requirements

#### REQ-UNINST-014: Reinstall guidance
- [x] Keep-config outro: "OMEGA has been removed. Config preserved at ~/.omega/config.toml"
- [x] Displayed both with and without warnings

## End-to-End Flow Results

| Flow | Steps | Result | Notes |
|---|---|---|---|
| Binary compilation | cargo build --release | PASS | Compiles with zero warnings |
| CLI integration | omega --help | PASS | `uninstall` command listed with correct description |
| Test suite | cargo test --workspace | PASS | 41 passed, 0 failed (includes 9 uninstall tests) |
| Clippy lint | cargo clippy --workspace | PASS | Zero warnings |
| Format check | cargo fmt --check | PASS | Clean |

Note: The `omega uninstall` command itself was NOT executed against a live system because it is destructive (deletes ~/.omega/, service files, symlinks). The interactive prompts (cliclack::select, cliclack::confirm) also require a TTY. Validation was performed through: (1) unit tests covering core logic, (2) code inspection of the interactive flow, and (3) binary help output verification.

## Exploratory Testing Findings

| # | What Was Tried | Expected | Actual | Severity |
|---|---|---|---|---|
| 1 | Check for `unwrap()` in non-test code | None found | None found | N/A (pass) |
| 2 | Check for `unsafe` blocks | None | None found | N/A (pass) |
| 3 | Check for `println!` (should use tracing/init_style) | None | None found | N/A (pass) |
| 4 | Check for `panic!/todo!/unimplemented!` | None | None found | N/A (pass) |
| 5 | File line count (500-line rule, excluding tests) | Under 500 | 383 non-test lines | N/A (pass) |
| 6 | KEEP_CONFIG_SUBDIRS vs requirements footprint | 9 subdirs matching spec | Match: data, logs, workspace, stores, prompts, skills, projects, topologies, whatsapp_session | N/A (pass) |
| 7 | Empty artifacts list (no ~/.omega/, no symlinks, no service) | Graceful exit, no crash | Code: "No Omega artifacts found" + outro, returns Ok(()) | N/A (pass) |
| 8 | HOME env var not set | Clear error | `anyhow::bail!("cannot determine home directory (HOME not set)")` | N/A (pass) |
| 9 | Path construction for traversal risk | All paths from $HOME via PathBuf::join | Confirmed: no user-supplied paths, no string concatenation | N/A (pass) |

## Failure Mode Validation

| Failure Scenario | Triggered | Detected | Recovered | Degraded OK | Notes |
|---|---|---|---|---|---|
| Permission denied on symlink removal | Not Triggered (requires root-owned file) | Yes (code matches PermissionDenied) | Yes (warning with sudo command) | Yes | Warning includes manual `sudo rm` command |
| Service stop failure | Not Triggered (would need running service) | Yes (code checks service_file_path Err) | Yes (warning added) | Yes | Continues to next step |
| Service file path resolution failure (HOME unset) | Not Triggered | Yes (anyhow::bail in run()) | Yes (propagates error) | Yes | Clean exit with error message |
| Non-interactive terminal | Not Triggered | Yes (cliclack returns Err) | Yes (propagated via `?`) | Yes | Nothing deleted; safe abort |
| Filesystem read-only | Not Triggered | Yes (remove_dir_all/remove_file return Err) | Yes (warning per step) | Yes | All steps report failure independently |
| Partial subdir removal failure (keep-config) | Not Triggered | Yes (per-subdir error handling) | Yes (warning, continues to next) | Yes | Other subdirs still removed |

## Security Validation

| Attack Surface | Test Performed | Result | Notes |
|---|---|---|---|
| Path traversal | Code inspection -- all paths derived from $HOME via PathBuf::join() | PASS | No user-supplied paths; hardcoded artifact list |
| Root execution | Root guard exists in main.rs (line 113-118) | PASS | libc::geteuid() check before any command runs |
| Accidental data loss | Two-step confirmation: mode select + explicit yes/no with default=No | PASS | Safe default prevents accidental deletion |
| Secret exposure in logs | Code uses init_style helpers (write to stderr via Term) | PASS | No secrets logged; only path names displayed |
| Prompt injection via cliclack | No user-supplied text passed to shell commands | PASS | systemctl args are hardcoded strings |

## Specs/Docs Drift

| File | Documented Behavior | Actual Behavior | Severity |
|------|-------------------|-----------------|----------|
| `docs/functionalities/cli-functionalities.md` | Lists 5 commands + service management. Overview says "5 commands (Start, Status, Ask, Init, Pair)". No mention of `omega uninstall`. | Binary has 7 top-level commands including `uninstall` and `setup`. | medium |
| `docs/src-main-rs.md` | Documents commands 1-5 (start, status, ask, init, pair) + service. No section for `omega uninstall`. Summary table has 8 rows, none for `omega uninstall`. | `omega uninstall` exists as a top-level command. | medium |
| `docs/architecture.md` | No mention of uninstall command. | Uninstall command exists. | low |
| `specs/SPECS.md` | Updated with uninstall-requirements.md and uninstall-architecture.md entries. | Matches. | N/A (no drift) |

## Blocking Issues (must fix before merge)

None.

## Non-Blocking Observations

- **[OBS-001]**: `docs/functionalities/cli-functionalities.md` -- The functionalities table does not include `omega uninstall` as a CLI command entry. The overview text says "5 commands" but there are now 7 top-level commands (Start, Status, Ask, Init, Setup, Pair, Uninstall). Should be updated for completeness.

- **[OBS-002]**: `docs/src-main-rs.md` -- The user guide does not document the `omega uninstall` command. There is no section explaining its purpose, usage, or what it does. The summary table at the bottom also omits it. Should be updated.

- **[OBS-003]**: The `ArtifactEntry.path` field has `#[allow(dead_code)]` because it is only used in tests for assertions (line 40-41). This is a pragmatic choice and not a problem, but it could be cleaner to either: (a) use `#[cfg(test)]` on the field, or (b) use `path` in the display_summary function. Minor style observation only.

- **[OBS-004]**: `step_stop_service()` calls `service::stop_service()` without checking the return -- but `stop_service()` returns `()` (fire-and-forget via `let _ = Command...output()`). This means if the stop actually fails, the "Service stopped" success message is still shown. The service.rs function swallows the error. This is pre-existing behavior in service.rs, not introduced by this change, and is low-severity since deletion proceeds regardless.

## Modules Not Validated (if context limited)

None -- all modules in scope were fully validated.

## Final Verdict

**PASS** -- All Must and Should requirements met. All 9 unit tests pass. Build compiles with zero warnings. No blocking issues found. The implementation faithfully follows the architecture design. Two non-blocking documentation drift items should be addressed before or shortly after merge.

**QA APPROVED.**
