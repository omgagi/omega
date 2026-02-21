# src/service.rs — Service Management Specification

## Path
`src/service.rs`

## Purpose
OS-aware service management for Omega. Detects macOS vs Linux and generates the appropriate service file (LaunchAgent plist or systemd user unit). Provides `install`, `uninstall`, and `status` commands with cliclack-styled interactive output. Pure generation functions are separated from I/O functions for testability.

## Module Overview
The `service.rs` module contains:
- `xml_escape(s) -> String` — Public pure function to escape XML special characters
- `generate_plist(binary, config, working_dir, data_dir) -> String` — Public pure function generating macOS LaunchAgent plist content
- `generate_systemd_unit(binary, config, working_dir, data_dir) -> String` — Public pure function generating Linux systemd user unit content
- `service_file_path() -> PathBuf` — Private helper returning the OS-specific service file location
- `install(config_path) -> Result<()>` — Public interactive function to install the service
- `uninstall() -> Result<()>` — Public interactive function to remove the service
- `status() -> Result<()>` — Public interactive function to check service status
- `stop_service(path)` — Private helper to deactivate the service
- `activate_service(path) -> bool` — Private helper to activate the service
- `is_running() -> bool` — Private helper to check if service is running

### Constants
- `LABEL: &str = "com.omega-cortex.omega"` — macOS LaunchAgent label, also used for `launchctl list` grep

### UX Layer: cliclack
All user interaction uses the `cliclack` crate. The following primitives are used:

| Primitive | Usage |
|-----------|-------|
| `cliclack::intro(msg)` | Opens each command session (install, uninstall, status) |
| `cliclack::outro(msg)` | Closes each command session on success |
| `cliclack::confirm(label)` | Asks user to overwrite existing service file |
| `cliclack::spinner()` | Animated spinner during service activation/deactivation |
| `cliclack::log::info(msg)` | Displays binary path, config path, service file path |
| `cliclack::log::success(msg)` | Confirms file write, installed status |
| `cliclack::log::warning(msg)` | Reports not-installed or not-running status |

---

## Pure Functions

### `xml_escape(s: &str) -> String`
Escapes `&`, `<`, `>` for safe embedding in XML/plist content. Order matters: `&` is replaced first to avoid double-escaping.

### `generate_plist(binary_path, config_path, working_dir, data_dir) -> String`
Generates a complete macOS LaunchAgent plist XML document.

**Template features:**
- `Label`: `com.omega-cortex.omega`
- `ProgramArguments`: `[binary, -c, config, start]`
- `WorkingDirectory`: Absolute path to config file's parent directory
- `RunAtLoad`: `true` — starts on login
- `KeepAlive`: `true` — auto-restarts on crash
- `StandardOutPath` / `StandardErrorPath`: Log to `{data_dir}/logs/omega.stdout.log` and `logs/omega.stderr.log`
- `EnvironmentVariables.PATH`: `/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin`

All path arguments are XML-escaped before interpolation.

### `generate_systemd_unit(binary_path, config_path, working_dir, data_dir) -> String`
Generates a complete Linux systemd user unit file.

**Template features:**
- `[Unit]`: After/Wants `network-online.target`
- `[Service]`: Type=simple, ExecStart with binary/config/start, WorkingDirectory, Restart=on-failure, RestartSec=5
- `StandardOutput` / `StandardError`: append to `{data_dir}/logs/omega.stdout.log` and `logs/omega.stderr.log`
- `[Install]`: WantedBy=default.target (user session)

---

## Service File Paths

| OS | Path |
|----|------|
| macOS | `~/Library/LaunchAgents/com.omega-cortex.omega.plist` |
| Linux | `~/.config/systemd/user/omega.service` |

Home directory is resolved from the `HOME` environment variable.

---

## Install Flow

1. `cliclack::intro("omega service install")`
2. Resolve binary path via `std::env::current_exe()` + `canonicalize()`
3. Resolve config path via `Path::canonicalize()` — bail if file not found with "run `omega init` first"
4. Derive working directory from config file's parent
5. Derive data directory via `shellexpand("~/.omega")`
6. Determine service file path from OS
7. If service file exists: prompt to overwrite, stop old service first if confirmed
8. Generate file content (plist or systemd unit based on OS)
9. Create parent directories, write file
10. Activate via `launchctl load` or `systemctl --user enable --now`
11. `cliclack::outro("Omega will now start automatically on login")`

## Uninstall Flow

1. Check if service file exists — if not, "nothing to do"
2. Stop service via `launchctl unload` or `systemctl --user stop+disable`
3. Remove service file
4. `cliclack::outro("Service removed")`

## Status Flow

1. Check if service file exists → report installed/not installed
2. Check if running via `launchctl list | grep` or `systemctl --user is-active`
3. Display results via cliclack log primitives

---

## Platform Helpers

### macOS
- **Activate:** `launchctl load <plist_path>`
- **Deactivate:** `launchctl unload <plist_path>`
- **Check running:** `launchctl list` and grep for label

### Linux
- **Activate:** `systemctl --user daemon-reload` then `systemctl --user enable --now omega.service`
- **Deactivate:** `systemctl --user stop omega.service` then `systemctl --user disable omega.service`
- **Check running:** `systemctl --user is-active --quiet omega.service`

All subprocess calls use `std::process::Command` with `.output()`. Failures are handled gracefully (no panics).

---

## Error Handling

- **Unsupported OS:** Returns error "only macOS and Linux are supported"
- **HOME not set:** Returns error "cannot determine home directory"
- **Binary not found:** Returns error from `current_exe()` with context
- **Config file missing:** Returns error with "run `omega init` first" hint
- **Service activation failure:** Spinner shows error, but install completes (file is written)
- **All subprocess errors:** Captured via `unwrap_or(false)`, never panic

---

## Unit Tests

### Test Suite: `tests` (5 tests)

| Test | Assertions |
|------|------------|
| `test_generate_plist_content` | Label, binary path, config path, working dir, RunAtLoad, KeepAlive, log paths present |
| `test_generate_plist_xml_escaping` | Paths with `<`, `>`, `&` are properly escaped; raw brackets absent |
| `test_generate_systemd_unit_content` | ExecStart, WorkingDirectory, Restart, RestartSec, log paths present |
| `test_generate_systemd_unit_structure` | [Unit], [Service], [Install] sections present, Description and WantedBy correct |
| `test_xml_escape` | Identity for clean strings, correct escaping of `&`, `<`, `>`, combined |

---

## Dependencies
- `std::path::{Path, PathBuf}` — Path manipulation
- `std::process::Command` — Subprocess execution (launchctl, systemctl)
- `std::fs` — File operations (create_dir_all, write, remove_file)
- `std::env` — `current_exe()`, `var("HOME")`
- `anyhow` — Error handling
- `cliclack` — Interactive CLI prompts
- `console` — Terminal styling (bold)
- `omega_core::shellexpand` — Home directory expansion for data_dir

## Called By
- `src/main.rs` — `Commands::Service { action }` match arm
- `src/init.rs` — Optional service install at end of wizard

## Files Created/Modified
- `~/Library/LaunchAgents/com.omega-cortex.omega.plist` (macOS)
- `~/.config/systemd/user/omega.service` (Linux)
