# Requirements: omega uninstall

## Scope
- **New file**: `backend/src/uninstall.rs` (~150-200 lines)
- **Modified file**: `backend/src/main.rs` (~15 lines — new `Uninstall` variant, match arm, module declaration)
- **Modified file**: `backend/src/service.rs` (~3 lines — change `service_file_path`, `stop_service`, `is_running` from private to `pub(crate)`)
- **Spec files**: `specs/uninstall-requirements.md` (new), `specs/SPECS.md` (updated index)

## Summary
Add a top-level `omega uninstall` CLI command that lets a user completely remove Omega from their system. The user chooses between two modes: (1) **complete removal** which deletes every file, directory, service, and binary symlink Omega ever created, or (2) **keep configuration** which preserves `~/.omega/config.toml` so the user can reinstall later without reconfiguring. The command uses branded interactive prompts with a final confirmation before any deletion occurs.

## User Stories
- As an Omega user, I want to run `omega uninstall` so that I can cleanly remove all Omega artifacts from my system without manually hunting for scattered files.
- As an Omega user, I want the option to keep my configuration when uninstalling so that I can reinstall Omega later without going through the setup wizard again.
- As an Omega user, I want to see exactly what will be deleted before confirming so that I don't accidentally lose important data.

## Omega File Footprint (canonical reference)

```
~/.omega/                          # Root data directory
  config.toml                      # Main configuration (PRESERVED in keep-config mode)
  data/                            # SQLite databases
    memory.db                      # Conversations, facts, tasks, audit, rewards, lessons
  logs/                            # Log files
    omega.log, omega.stdout.log, omega.stderr.log
  workspace/                       # AI subprocess working directory
    builds/                        # Build project outputs
  stores/                          # Domain-specific data
    google.json                    # Google OAuth credentials (optional)
  prompts/                         # Runtime prompt copies
  skills/                          # Skill definitions (*/SKILL.md)
  projects/                        # Project definitions (*/ROLE.md, */HEARTBEAT.md)
  topologies/                      # Runtime topology files
  whatsapp_session/                # WhatsApp session database (whatsapp.db)

~/Library/LaunchAgents/com.omega-cortex.omega.plist  # macOS service
~/.config/systemd/user/omega.service                 # Linux service
/usr/local/bin/omega                                 # Binary symlink
/usr/local/bin/omg-gog                               # Google tool binary (optional)
```

## Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|------------|----------|-------------------|
| REQ-UNINST-001 | Top-level `omega uninstall` CLI subcommand registered in clap | Must | `omega uninstall` is a valid command; `omega --help` lists `uninstall` |
| REQ-UNINST-002 | Two-mode selection: "Complete removal" vs "Keep configuration" | Must | Interactive select prompt offers exactly two options; selection determines which files are preserved |
| REQ-UNINST-003 | Stop running service before deletion | Must | Running service is stopped/unloaded before file removal; non-running service handled gracefully |
| REQ-UNINST-004 | Remove system service file | Must | macOS: plist deleted; Linux: unit file deleted and daemon reloaded; missing service file handled gracefully |
| REQ-UNINST-005 | Remove `~/.omega/` data directory (complete mode) | Must | Entire `~/.omega/` removed recursively including config.toml |
| REQ-UNINST-006 | Remove `~/.omega/` data except config.toml (keep-config mode) | Must | All subdirectories removed; `~/.omega/config.toml` preserved; `~/.omega/` directory itself preserved |
| REQ-UNINST-007 | Remove binary symlink `/usr/local/bin/omega` | Must | Symlink deleted if exists; missing symlink handled gracefully; permission error reported clearly |
| REQ-UNINST-008 | Final confirmation before destructive operations | Must | Explicit yes/no prompt after mode selection; default is "No" (safe default); cancellation aborts without deleting |
| REQ-UNINST-009 | Display summary of what will be deleted before confirmation | Should | List of existing paths shown before confirmation; non-existing items omitted |
| REQ-UNINST-010 | Branded CLI output using init_style helpers | Should | Uses omega_step, omega_success, omega_warning, omega_outro |
| REQ-UNINST-011 | Remove `/usr/local/bin/omg-gog` optional Google tool binary | Should | Deleted if exists, skipped if not; permission error reported as warning |
| REQ-UNINST-012 | Graceful handling of partial failures | Should | Each deletion step is independent; errors reported as warnings; final summary reflects partial failure |
| REQ-UNINST-013 | Systemd daemon-reload after service removal on Linux | Should | `systemctl --user daemon-reload` called after removing unit; failure reported as warning |
| REQ-UNINST-014 | Display reinstall guidance after keep-config uninstall | Could | Outro mentions preserved config path |
| REQ-UNINST-015 | Non-interactive mode via `--yes` or `--force` flag | Won't | Deferred |
| REQ-UNINST-016 | Backup data before deletion | Won't | Deferred |

## Acceptance Criteria (detailed)

### REQ-UNINST-001: Top-level CLI subcommand
- Given a compiled omega binary, when the user runs `omega uninstall`, then the uninstall flow begins
- Given a compiled omega binary, when the user runs `omega --help`, then `uninstall` appears in the command list
- Given no config file exists, the command still works (it removes whatever artifacts exist)

### REQ-UNINST-002: Two-mode selection
- Given the user runs `omega uninstall`, when the mode selection appears, then two options are shown
- Given the user selects "Complete removal", then ALL Omega artifacts are targeted for deletion
- Given the user selects "Keep configuration", then `~/.omega/config.toml` is excluded from deletion

### REQ-UNINST-003: Stop running service
- Given a running Omega service, when uninstall proceeds, then the service is stopped before any files are deleted
- Given no service is running, when uninstall proceeds, then no error occurs
- Reuses `service::stop_service()` (requires visibility change to `pub(crate)`)

### REQ-UNINST-004: Remove service file
- Given a macOS system with the plist, when uninstall proceeds, then the plist file is deleted
- Given a Linux system with the unit file, when uninstall proceeds, then the unit file is deleted
- Given no service file exists, this step is skipped without error
- Reuses `service::service_file_path()` (requires visibility change to `pub(crate)`)

### REQ-UNINST-005: Complete removal of ~/.omega/
- Given "Complete removal" mode, `~/.omega/` and ALL contents are removed via `std::fs::remove_dir_all`
- Given `~/.omega/` does not exist, this step is skipped without error

### REQ-UNINST-006: Keep-config removal of ~/.omega/
- Given "Keep configuration" mode, subdirectories are removed: `data/`, `logs/`, `workspace/`, `stores/`, `prompts/`, `skills/`, `projects/`, `topologies/`, `whatsapp_session/`
- `~/.omega/config.toml` still exists after uninstall completes
- Missing subdirectories are skipped without error

### REQ-UNINST-007: Remove binary symlink
- Given `/usr/local/bin/omega` exists, when deletion proceeds, then it is removed
- Given removal fails due to permissions, a warning is displayed with manual `sudo rm` command

### REQ-UNINST-008: Final confirmation
- Default value is `false` (No) — safe default
- User must explicitly confirm to proceed
- Cancellation aborts without deleting anything

### REQ-UNINST-009: Pre-deletion summary
- Each existing artifact is listed with its path
- Non-existing artifacts are not listed
- In keep-config mode, summary distinguishes preserved vs deleted items

### REQ-UNINST-010: Branded output
- Uses `init_style::omega_step` for phase announcements
- Uses `init_style::omega_success` for each deleted artifact
- Uses `init_style::omega_warning` for skipped/failed items
- Uses `init_style::omega_outro` for completion message

### REQ-UNINST-011: Remove omg-gog binary
- Deleted if exists, silently skipped if not
- Permission error reported as warning with manual removal command

### REQ-UNINST-012: Partial failure handling
- Each deletion step is independent — failure in one does not abort others
- Final message says "Uninstall completed with warnings" if any step failed

### REQ-UNINST-013: Systemd daemon-reload
- On Linux, `systemctl --user daemon-reload` after removing unit file
- Failure reported as warning

### REQ-UNINST-014: Reinstall guidance
- Keep-config outro includes "Config preserved at ~/.omega/config.toml"

## Impact Analysis

### Existing Code Affected
- `backend/src/main.rs`: Add `mod uninstall`, `Commands::Uninstall` variant, match arm — **Risk: low** (purely additive)
- `backend/src/service.rs`: Change `service_file_path()`, `stop_service()`, `is_running()` from `fn` to `pub(crate) fn` — **Risk: low** (visibility-only)
- `backend/src/init_style.rs`: No changes needed (already `pub(crate)`) — **Risk: none**

### What Breaks If This Changes
- Nothing breaks. Purely additive feature. No existing code paths modified.

### Regression Risk Areas
- **Service management**: Reuses `stop_service()` and `service_file_path()` — already tested
- **File footprint drift**: Future Omega versions adding new artifact locations must update the uninstall module

## Traceability Matrix

| Requirement ID | Priority | Test IDs | Architecture Section | Implementation Module |
|---------------|----------|----------|---------------------|---------------------|
| REQ-UNINST-001 | Must | TBD | Integration: main.rs Changes | backend/src/main.rs, backend/src/uninstall.rs |
| REQ-UNINST-002 | Must | TBD | Flow Detail (step 2: Mode selection) | backend/src/uninstall.rs |
| REQ-UNINST-003 | Must | TBD | Flow Detail (step 6: step_stop_service) | backend/src/uninstall.rs, backend/src/service.rs |
| REQ-UNINST-004 | Must | TBD | Flow Detail (step 6: step_remove_service_file) | backend/src/uninstall.rs, backend/src/service.rs |
| REQ-UNINST-005 | Must | TBD | Flow Detail (step 6: step_remove_data_dir, complete) | backend/src/uninstall.rs |
| REQ-UNINST-006 | Must | TBD | Flow Detail (step 6: step_remove_data_dir, keep-config) | backend/src/uninstall.rs |
| REQ-UNINST-007 | Must | TBD | Flow Detail (step 6: step_remove_symlink) | backend/src/uninstall.rs |
| REQ-UNINST-008 | Must | TBD | Flow Detail (step 5: Confirmation) | backend/src/uninstall.rs |
| REQ-UNINST-009 | Should | TBD | Flow Detail (steps 3-4: Artifact scan + Display summary) | backend/src/uninstall.rs |
| REQ-UNINST-010 | Should | TBD | Flow Detail (all steps use init_style helpers) | backend/src/uninstall.rs, backend/src/init_style.rs |
| REQ-UNINST-011 | Should | TBD | Flow Detail (step 6: step_remove_symlink omg-gog) | backend/src/uninstall.rs |
| REQ-UNINST-012 | Should | TBD | Failure Modes, UninstallResult accumulator | backend/src/uninstall.rs |
| REQ-UNINST-013 | Should | TBD | Flow Detail (step 6: step_daemon_reload) | backend/src/uninstall.rs |
| REQ-UNINST-014 | Could | TBD | Flow Detail (step 7: Outro with config path) | backend/src/uninstall.rs |

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Permission denied on `/usr/local/bin/` symlink removal | Medium | Report clear warning with manual `sudo rm` command |
| Incomplete artifact list as project evolves | Low | Document canonical file footprint; maintenance note in module |
| Accidental data loss | Medium | Default "No" confirmation; two-step flow (mode + confirm) |
| Concurrent non-service Omega process still running | Low | Service stop handles the service case; manual process is user's responsibility |

## Out of Scope
- Non-interactive `--yes`/`--force` flag (deferred)
- Backup before deletion (deferred)
- Removing the compiled binary at `~/.cargo/target-global/release/omega` (managed by cargo)
- Removing `omg-gog` internal data beyond `~/.omega/stores/google.json`
