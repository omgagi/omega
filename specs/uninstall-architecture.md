# Architecture: omega uninstall

## Scope

Single new module `backend/src/uninstall.rs` (~150-200 lines) plus minimal integration changes in `main.rs` and `service.rs`. Implements the `omega uninstall` CLI subcommand for clean system removal.

## Overview

```
omega uninstall
      |
      v
[branded intro]
      |
      v
[mode select: Complete / Keep Config]
      |
      v
[scan filesystem -> build artifact list]
      |
      v
[display deletion summary]
      |
      v
[confirm? default=No] --No--> [abort, outro_cancel]
      |
     Yes
      |
      v
[stop service]         -- warn on failure, continue
      |
      v
[remove service file]  -- warn on failure, continue
      |
      v
[daemon-reload (Linux)]-- warn on failure, continue
      |
      v
[remove ~/.omega/]     -- complete: remove_dir_all
      |                -- keep-config: remove subdirs only
      v
[remove /usr/local/bin/omega]   -- warn on permission error
      |
      v
[remove /usr/local/bin/omg-gog] -- warn on permission error
      |
      v
[outro: success or partial-failure summary]
```

## Modules

### Module 1: uninstall.rs

- **Responsibility**: Orchestrate the full uninstall flow -- mode selection, artifact scanning, confirmation, deletion, and status reporting.
- **Public interface**:
  ```rust
  /// Entry point called from main.rs match arm.
  pub(crate) fn run() -> anyhow::Result<()>
  ```
- **Dependencies**:
  - `crate::init_style` -- branded output helpers (`omega_step`, `omega_success`, `omega_warning`, `omega_outro`, `omega_outro_cancel`, `omega_info`)
  - `crate::service` -- `service_file_path()`, `stop_service()`, `is_running()` (visibility change from `fn` to `pub(crate) fn`)
  - `cliclack` -- `select` for mode choice, `confirm` for final confirmation
  - `std::fs` -- `remove_dir_all`, `remove_file`, `remove_dir`
  - `std::path::PathBuf`
  - `std::process::Command` -- systemd daemon-reload on Linux
- **Implementation order**: 1 (single module, no internal ordering needed)

#### Internal Structure

The module is organized as a single file with these internal functions:

```rust
/// Uninstall mode selected by the user.
enum UninstallMode {
    Complete,
    KeepConfig,
}

/// Result tracker for partial-failure reporting.
struct UninstallResult {
    warnings: Vec<String>,
}

/// Entry point.
pub(crate) fn run() -> anyhow::Result<()>

/// Scan the filesystem and return a list of existing artifact paths.
/// Non-existing items are excluded.
fn scan_artifacts(mode: &UninstallMode) -> Vec<ArtifactEntry>

/// An artifact to be deleted (or preserved).
struct ArtifactEntry {
    path: PathBuf,
    label: String,       // human-readable description
    preserved: bool,     // true = shown but not deleted (keep-config mode)
}

/// Display the pre-deletion summary.
fn display_summary(artifacts: &[ArtifactEntry]) -> io::Result<()>

/// Stop the running service (if any). Returns warning string on failure.
fn step_stop_service(result: &mut UninstallResult)

/// Remove the service file. Returns warning string on failure.
fn step_remove_service_file(result: &mut UninstallResult)

/// Run systemd daemon-reload on Linux. No-op on macOS.
fn step_daemon_reload(result: &mut UninstallResult)

/// Remove ~/.omega/ directory (complete mode) or subdirectories (keep-config mode).
fn step_remove_data_dir(mode: &UninstallMode, result: &mut UninstallResult)

/// Remove a binary symlink (/usr/local/bin/omega or /usr/local/bin/omg-gog).
fn step_remove_symlink(path: &str, label: &str, result: &mut UninstallResult)
```

#### Flow Detail

1. **Branded intro**: `omega_step("Uninstall OMEGA")` -- no full-screen clear (unlike `omega_intro`), just a step announcement.

2. **Mode selection**: `cliclack::select` with two options:
   - "Complete removal -- delete everything including configuration"
   - "Keep configuration -- preserve ~/.omega/config.toml for reinstall"

3. **Artifact scan** (`scan_artifacts`): Check `Path::exists()` for each known artifact. Build a list of `ArtifactEntry` structs. The canonical artifact list:

   | Artifact | Complete Mode | Keep-Config Mode |
   |----------|--------------|-----------------|
   | `~/.omega/` (entire dir) | DELETE | -- |
   | `~/.omega/data/` | -- | DELETE |
   | `~/.omega/logs/` | -- | DELETE |
   | `~/.omega/workspace/` | -- | DELETE |
   | `~/.omega/stores/` | -- | DELETE |
   | `~/.omega/prompts/` | -- | DELETE |
   | `~/.omega/skills/` | -- | DELETE |
   | `~/.omega/projects/` | -- | DELETE |
   | `~/.omega/topologies/` | -- | DELETE |
   | `~/.omega/whatsapp_session/` | -- | DELETE |
   | `~/.omega/config.toml` | (included in dir) | PRESERVED |
   | Service file (platform-specific) | DELETE | DELETE |
   | `/usr/local/bin/omega` | DELETE | DELETE |
   | `/usr/local/bin/omg-gog` | DELETE | DELETE |

   In complete mode, only the top-level `~/.omega/` is listed (since `remove_dir_all` handles everything). In keep-config mode, each subdirectory is listed individually, and `config.toml` is shown as "PRESERVED".

4. **Display summary** (`display_summary`): Use `omega_info` for items to be deleted, `omega_success` with "(preserved)" for kept items. Only existing items are shown.

5. **Confirmation**: `cliclack::confirm("Proceed with uninstall?").initial_value(false).interact()?` -- safe default is No. On cancel/No, call `omega_outro_cancel("Uninstall cancelled")` and return Ok.

6. **Deletion steps** (each independent, failures accumulated in `UninstallResult::warnings`):
   - `step_stop_service` -- call `service::is_running()`, if true call `service::stop_service()`. Log via `omega_step`. Catch any failure as warning.
   - `step_remove_service_file` -- call `service::service_file_path()`, if exists call `std::fs::remove_file()`. Log via `omega_success` or `omega_warning`.
   - `step_daemon_reload` -- on Linux only: `Command::new("systemctl").args(["--user", "daemon-reload"])`. Log warning on failure.
   - `step_remove_data_dir` -- complete mode: `fs::remove_dir_all("~/.omega/")`. Keep-config mode: iterate known subdirs, `fs::remove_dir_all` each. Log per-item success/warning.
   - `step_remove_symlink("/usr/local/bin/omega", "omega binary")` -- `fs::remove_file`. On permission error, `omega_warning` with "Run: sudo rm /usr/local/bin/omega".
   - `step_remove_symlink("/usr/local/bin/omg-gog", "omg-gog binary")` -- same pattern.

7. **Outro**:
   - No warnings: `omega_outro("OMEGA has been completely removed")` or `omega_outro("OMEGA has been removed. Config preserved at ~/.omega/config.toml")`.
   - With warnings: `omega_warning("Uninstall completed with N warning(s)")` then `omega_outro(...)`.

#### Failure Modes

| Failure | Cause | Detection | Recovery | Impact |
|---------|-------|-----------|----------|--------|
| Permission denied on `/usr/local/bin/` symlink | User ran without sudo, symlink owned by root | `std::io::ErrorKind::PermissionDenied` on `remove_file` | Log warning with manual `sudo rm` command | Symlink remains; user can remove manually |
| Service stop fails | Service not installed, launchctl/systemctl error | `is_running()` returns false after stop attempt, or Command returns non-zero | Log warning, continue with file deletion | Process may still be running; files deleted underneath it (process will crash on next I/O) |
| `~/.omega/` removal fails | Directory in use (open file handles), permission issue | `remove_dir_all` returns `Err` | Log warning with the specific error | Partial data remains; user informed |
| Service file path resolution fails | HOME env var not set | `service_file_path()` returns `Err` | Log warning, skip service-related steps | Service file and stop are skipped |
| `cliclack` prompt fails | Non-interactive terminal (piped stdin) | `interact()` returns `Err` | Propagate error via `?` -- command aborts cleanly | Nothing deleted; safe |
| Subdirectory removal fails (keep-config) | One subdir fails | `remove_dir_all` returns `Err` for that subdir | Log warning, continue to next subdir | Partial cleanup; user informed which dirs remain |

#### Security Considerations

- **Trust boundary**: All inputs are from the local filesystem and the interactive terminal. No network, no untrusted data.
- **Sensitive data**: `~/.omega/config.toml` contains API keys and tokens. `~/.omega/stores/google.json` contains OAuth credentials. `~/.omega/data/memory.db` contains conversation history. All are deleted (complete mode) or explicitly handled (keep-config preserves only config.toml).
- **Attack surface**: Minimal. The command only deletes files at hardcoded paths derived from `$HOME`. No path traversal, no user-supplied paths.
- **Mitigations**:
  - Two-step confirmation (mode select + explicit yes/no) prevents accidental data loss
  - Default confirmation value is `false` (No) -- pressing Enter alone does NOT delete
  - Root execution is already blocked by the guard in `main.rs` (line 110-115)
  - No `unwrap()` -- all fallible operations use `?` or explicit error handling
  - Path construction uses `PathBuf::join()` -- no string concatenation of paths

#### Performance Budget

- **Latency target**: < 1s for the non-interactive part (scanning + deletion). The interactive prompts dominate wall-clock time.
- **Memory budget**: < 5MB RSS (only holds a Vec of ~15 path entries)
- **Complexity target**: O(n) where n = number of artifacts (constant, ~15)
- **Throughput target**: N/A (one-shot CLI command)

### Integration: main.rs Changes

Three changes required:

1. **Module declaration** (line ~15 area):
   ```rust
   mod uninstall;
   ```

2. **Enum variant** (inside `enum Commands`):
   ```rust
   /// Completely remove OMEGA from this system.
   Uninstall,
   ```

3. **Match arm** (inside `match cli.command`):
   ```rust
   Commands::Uninstall => {
       init_stdout_tracing("error");
       uninstall::run()?;
   }
   ```

### Integration: service.rs Visibility Changes

Three functions change from `fn` to `pub(crate) fn`:

```rust
// Line 107: fn service_file_path() -> pub(crate) fn service_file_path()
pub(crate) fn service_file_path() -> anyhow::Result<PathBuf>

// Line 263: fn stop_service() -> pub(crate) fn stop_service()
pub(crate) fn stop_service(svc_path: &Path)

// Line 300: fn is_running() -> pub(crate) fn is_running()
pub(crate) fn is_running() -> bool
```

No logic changes. No signature changes. Pure visibility upgrade.

## Failure Modes (system-level)

| Scenario | Affected Modules | Detection | Recovery Strategy | Degraded Behavior |
|----------|-----------------|-----------|-------------------|-------------------|
| Non-interactive terminal | uninstall.rs | cliclack `interact()` returns Err | Propagate error, abort cleanly | Nothing deleted; exit with error message |
| HOME not set | uninstall.rs, service.rs | `std::env::var("HOME")` returns Err | Skip service steps, use fallback for data dir | Partial cleanup; warn user |
| Concurrent Omega process running | uninstall.rs | `is_running()` returns true after stop | Warn user; continue deletion | Files deleted under running process; it will crash |
| Filesystem read-only | uninstall.rs | All `remove_*` calls return Err | Each step logs warning independently | Nothing deleted; all steps report failure |

## Security Model

### Trust Boundaries

- **Terminal input**: Trusted (local user at physical/SSH terminal). The interactive prompts (mode select, confirm) are the only inputs.
- **Filesystem paths**: All hardcoded from `$HOME`. No user-supplied paths. No config file read required.

### Data Classification

| Data | Classification | Storage | Deleted By |
|------|---------------|---------|------------|
| API keys/tokens | Secret | `~/.omega/config.toml` | Complete mode (or preserved in keep-config) |
| OAuth credentials | Secret | `~/.omega/stores/google.json` | Both modes |
| Conversation history | Confidential | `~/.omega/data/memory.db` | Both modes |
| WhatsApp session | Confidential | `~/.omega/whatsapp_session/` | Both modes |
| Skills/Projects | Internal | `~/.omega/skills/`, `~/.omega/projects/` | Both modes |
| Service file | Internal | Platform-specific path | Both modes |

### Attack Surface

- **Denial of service via repeated uninstall**: No risk -- command is idempotent (non-existing items skipped gracefully).
- **Social engineering**: Risk is "someone tricks user into running `omega uninstall`". Mitigation: two-step confirmation with safe default.

## Graceful Degradation

| Dependency | Normal Behavior | Degraded Behavior | User Impact |
|-----------|----------------|-------------------|-------------|
| `service::service_file_path()` | Resolves platform service path | Returns Err (HOME not set) | Service stop/removal skipped; warning shown |
| `service::is_running()` | Detects running service | Returns false (command failed) | Service not stopped before deletion; may crash |
| `std::fs::remove_dir_all` | Removes directory tree | Returns Err (permission/lock) | Specific dir remains; warning with path shown |
| `std::fs::remove_file` | Removes file | Returns Err (permission) | File remains; warning with manual command shown |
| `cliclack::select/confirm` | Interactive prompts | Returns Err (non-interactive) | Command aborts cleanly; nothing deleted |

## Performance Budgets

| Operation | Latency (p50) | Latency (p99) | Memory | Notes |
|-----------|---------------|---------------|--------|-------|
| Artifact scan | < 5ms | < 20ms | < 1KB | stat() calls on ~15 paths |
| Service stop | < 2s | < 5s | N/A | launchctl/systemctl subprocess |
| Directory removal | < 100ms | < 1s | N/A | Depends on data size |
| Symlink removal | < 5ms | < 50ms | N/A | Single unlink syscall |
| Total (non-interactive) | < 3s | < 7s | < 5MB | Dominated by service stop |

## Data Flow

```
User (terminal)
      |
      | (select mode)
      v
uninstall::run()
      |
      | scan_artifacts()
      | -> stat() each known path
      | -> build ArtifactEntry list
      |
      | display_summary()
      | -> omega_info/omega_success per item
      |
      | (confirm?)
      |
      | step_stop_service()
      | -> service::is_running()
      | -> service::stop_service(svc_path)
      |
      | step_remove_service_file()
      | -> service::service_file_path()
      | -> fs::remove_file()
      |
      | step_daemon_reload() [Linux only]
      | -> Command::new("systemctl")
      |
      | step_remove_data_dir()
      | -> fs::remove_dir_all() or per-subdir
      |
      | step_remove_symlink() x2
      | -> fs::remove_file()
      |
      v
outro (success or partial-failure)
```

## Design Decisions

| Decision | Alternatives Considered | Justification |
|----------|------------------------|---------------|
| Synchronous (no async) | `async fn run()` | No I/O that benefits from async. All operations are sequential filesystem calls + subprocess. Avoids tokio dependency for a simple CLI flow. |
| Single `run()` entry point | Separate functions exposed to main.rs | Keeps the public surface minimal. All orchestration logic contained in one module. |
| Hardcoded artifact paths | Read paths from config.toml | Requirements explicitly state "does NOT need to read config.toml." Hardcoded paths from `$HOME` are simpler, work even when config is corrupted/missing, and match the canonical footprint. |
| `UninstallResult` warning accumulator | Return `Result` from each step | Each step must be independent (REQ-UNINST-012). Accumulating warnings allows all steps to run and report at the end. |
| `cliclack::select` for mode | Two separate yes/no prompts | Cleaner UX. Single decision point. Matches the existing init wizard pattern. |
| Reuse `service::stop_service()` | Duplicate the launchctl/systemctl logic | DRY. The function already handles both platforms correctly. Only needs a visibility change. |
| No `--yes`/`--force` flag | Include non-interactive mode | Explicitly deferred (REQ-UNINST-015, Won't priority). Keeps the initial implementation simple and safe. |
| Default confirm = false | Default = true | Destructive operation. Safe default prevents accidental data loss (REQ-UNINST-008). |

## External Dependencies

No new dependencies. All used crates are already in the workspace:

- `cliclack` -- interactive prompts (already used by `init.rs`, `service.rs`)
- `console` -- terminal styling (already used by `init_style.rs`)
- `anyhow` -- error handling (already used everywhere)
- `std::fs`, `std::path`, `std::process` -- standard library

## Milestones

| ID | Name | Scope (Modules) | Scope (Requirements) | Est. Size | Dependencies |
|----|------|-----------------|---------------------|-----------|-------------|
| M1 | Uninstall Command | uninstall.rs, main.rs (integration), service.rs (visibility) | REQ-UNINST-001 to REQ-UNINST-014 | S | None |

Single milestone. The feature is a self-contained CLI command with no external module dependencies beyond the existing `service.rs` helpers and `init_style.rs` output functions.

## Requirement Traceability

| Requirement ID | Architecture Section | Module(s) |
|---------------|---------------------|-----------|
| REQ-UNINST-001 | Integration: main.rs Changes | `backend/src/main.rs`, `backend/src/uninstall.rs` |
| REQ-UNINST-002 | Flow Detail (step 2: Mode selection) | `backend/src/uninstall.rs` |
| REQ-UNINST-003 | Flow Detail (step 6: step_stop_service) | `backend/src/uninstall.rs`, `backend/src/service.rs` |
| REQ-UNINST-004 | Flow Detail (step 6: step_remove_service_file) | `backend/src/uninstall.rs`, `backend/src/service.rs` |
| REQ-UNINST-005 | Flow Detail (step 6: step_remove_data_dir, complete mode) | `backend/src/uninstall.rs` |
| REQ-UNINST-006 | Flow Detail (step 6: step_remove_data_dir, keep-config mode) | `backend/src/uninstall.rs` |
| REQ-UNINST-007 | Flow Detail (step 6: step_remove_symlink /usr/local/bin/omega) | `backend/src/uninstall.rs` |
| REQ-UNINST-008 | Flow Detail (step 5: Confirmation) | `backend/src/uninstall.rs` |
| REQ-UNINST-009 | Flow Detail (step 3-4: Artifact scan + Display summary) | `backend/src/uninstall.rs` |
| REQ-UNINST-010 | Flow Detail (all steps use init_style helpers) | `backend/src/uninstall.rs`, `backend/src/init_style.rs` |
| REQ-UNINST-011 | Flow Detail (step 6: step_remove_symlink /usr/local/bin/omg-gog) | `backend/src/uninstall.rs` |
| REQ-UNINST-012 | Failure Modes table, UninstallResult accumulator | `backend/src/uninstall.rs` |
| REQ-UNINST-013 | Flow Detail (step 6: step_daemon_reload) | `backend/src/uninstall.rs` |
| REQ-UNINST-014 | Flow Detail (step 7: Outro with config path) | `backend/src/uninstall.rs` |
