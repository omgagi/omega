# Specification: src/claudemd.rs

## File Path
`src/claudemd.rs`

## Purpose
Workspace CLAUDE.md maintenance — ensures the Claude Code subprocess has persistent project context in its working directory (`~/.omega/workspace/`). On startup, creates the file if missing. A background loop periodically refreshes it.

Both operations use direct subprocess calls to `claude -p` (not the Provider trait) since this is a meta-operation for workspace maintenance, not a user message.

## Constants

### `CLAUDEMD_TIMEOUT`
- **Type:** `Duration`
- **Value:** 120 seconds
- **Purpose:** Timeout for CLAUDE.md maintenance subprocess calls.

## Functions

### `pub async fn ensure_claudemd(workspace: &Path, data_dir: &Path, sandbox_mode: SandboxMode)`
**Purpose:** Ensure `CLAUDE.md` exists in the workspace directory. If missing, runs `claude -p` with an init prompt.

**Parameters:**
- `workspace: &Path` — Path to `~/.omega/workspace/`.
- `data_dir: &Path` — Path to `~/.omega/` (for sandbox enforcement and prompt context).
- `sandbox_mode: SandboxMode` — Active sandbox mode for OS-level enforcement.

**Returns:** None (void).

**Logic:**
1. Check if `workspace/CLAUDE.md` exists. If yes, log info and return.
2. Build an init prompt instructing Claude Code to explore the workspace and sibling directories (skills/, projects/) and create a concise CLAUDE.md.
3. Call `run_claude()` with the prompt.
4. On error, log warning (non-fatal).

**Error Handling:** Non-fatal — warns on failure, never blocks startup.

### `pub async fn refresh_claudemd(workspace: &Path, data_dir: &Path, sandbox_mode: SandboxMode)`
**Purpose:** Ask Claude Code to review and update the existing workspace CLAUDE.md.

**Parameters:** Same as `ensure_claudemd`.

**Returns:** None (void).

**Logic:**
1. If `workspace/CLAUDE.md` doesn't exist, delegate to `ensure_claudemd()`.
2. Build an update prompt instructing Claude Code to review and update the file if the workspace has changed.
3. Call `run_claude()` with the prompt.
4. On error, log warning (non-fatal).

### `pub async fn claudemd_loop(workspace: PathBuf, data_dir: PathBuf, sandbox_mode: SandboxMode, interval_hours: u64)`
**Purpose:** Background loop that periodically refreshes the workspace CLAUDE.md.

**Parameters:**
- `workspace: PathBuf` — Path to `~/.omega/workspace/`.
- `data_dir: PathBuf` — Path to `~/.omega/`.
- `sandbox_mode: SandboxMode` — Active sandbox mode.
- `interval_hours: u64` — Hours between refreshes (default: 24).

**Returns:** Never returns (infinite loop, aborted on shutdown).

**Logic:**
1. Sleep for `interval_hours * 3600` seconds.
2. Call `refresh_claudemd()`.
3. Repeat.

### `async fn run_claude(prompt: &str, workspace: &Path, data_dir: &Path, sandbox_mode: SandboxMode) -> Result<(), String>`
**Purpose:** Run `claude -p` as a direct subprocess for CLAUDE.md maintenance.

**Parameters:**
- `prompt: &str` — The prompt to send to Claude Code.
- `workspace: &Path` — Working directory for the subprocess.
- `data_dir: &Path` — Data directory for sandbox enforcement.
- `sandbox_mode: SandboxMode` — Sandbox mode for OS-level enforcement.

**Returns:** `Result<(), String>` — Ok on success, Err with description on failure.

**Logic:**
1. Build command via `omega_sandbox::sandboxed_command("claude", sandbox_mode, data_dir)`.
2. Set `current_dir(workspace)`, remove `CLAUDECODE` env var.
3. Pass `-p`, `--output-format json`, `--dangerously-skip-permissions`.
4. Execute with 120s timeout via `tokio::time::timeout()`.
5. Check exit status — Ok if success, Err with truncated stderr otherwise.

## Tests

### `test_ensure_claudemd_skips_when_exists`
**Type:** Async unit test (`#[tokio::test]`)
Verifies that `ensure_claudemd()` returns immediately without spawning a subprocess when `CLAUDE.md` already exists.

### `test_claudemd_path_construction`
**Type:** Synchronous unit test (`#[test]`)
Verifies that the CLAUDE.md path is correctly constructed from the workspace path.

## Dependencies

### External Crates
- `tokio` — Async runtime, process spawning, timeout.
- `tracing` — Structured logging (`info!`, `warn!`).

### Internal Dependencies
- `omega_core::config::SandboxMode` — Sandbox mode enum.
- `omega_sandbox::sandboxed_command` — OS-level sandbox enforcement.

## Design Decisions

- **Direct subprocess, not Provider trait:** This is a system maintenance operation. Using the Provider trait would mix concerns.
- **Non-blocking startup:** `ensure_claudemd` is spawned as a background task, doesn't block channel startup.
- **Non-fatal:** All failures are logged as warnings, never crash the gateway.
- **Claude Code only:** Guarded by `provider.name() == "claude-code"` in the gateway.
- **24-hour refresh:** Low cost, keeps the file reasonably current without burning tokens.
- **No config section:** Always-on for Claude Code provider — follows the "less is more" principle.
