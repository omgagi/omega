# Specification: src/claudemd.rs

## File Path
`src/claudemd.rs`

## Purpose
Workspace CLAUDE.md maintenance — ensures the Claude Code subprocess has persistent project context in its working directory (`~/.omega/workspace/`). Uses a **template-first** approach: a bundled template (`prompts/WORKSPACE_CLAUDE.md`) contains standard operational rules that survive across deployments and 24h refreshes. Dynamic content (skills/projects tables) is appended below a marker line by `claude -p`.

Both operations use direct subprocess calls to `claude -p` (not the Provider trait) since this is a meta-operation for workspace maintenance, not a user message.

## Constants

### `CLAUDEMD_TIMEOUT`
- **Type:** `Duration`
- **Value:** 120 seconds
- **Purpose:** Timeout for CLAUDE.md maintenance subprocess calls.

### `DYNAMIC_MARKER`
- **Type:** `&str`
- **Value:** `"<!-- DYNAMIC CONTENT BELOW"`
- **Purpose:** Marker line that separates the bundled template from dynamic content. Everything above this line is re-deployed from the binary on every refresh. Everything below is instance-specific (skills/projects tables) and preserved across refreshes.

## Functions

### `pub async fn ensure_claudemd(workspace: &Path, data_dir: &Path)`
**Purpose:** Ensure `CLAUDE.md` exists in the workspace directory using the bundled template + dynamic enrichment.

**Parameters:**
- `workspace: &Path` — Path to `~/.omega/workspace/`.
- `data_dir: &Path` — Path to `~/.omega/` (for filesystem protection and prompt context).

**Returns:** None (void).

**Logic:**
1. Check if `workspace/CLAUDE.md` exists. If yes, log info and return.
2. Write the bundled template (from `omega_core::config::bundled_workspace_claude()`) to `workspace/CLAUDE.md`. This guarantees standard operational rules are present even if the next step fails.
3. Run `claude -p` with a prompt to explore `data_dir/skills/` and `data_dir/projects/`, then APPEND skills/projects tables below the dynamic content marker.
4. On `claude -p` failure, log warning — the template with all standard rules is already deployed.

**Error Handling:** Non-fatal — warns on failure, never blocks startup. Graceful degradation: template is always deployed even if enrichment fails.

### `pub async fn refresh_claudemd(workspace: &Path, data_dir: &Path)`
**Purpose:** Re-deploy the bundled template (preserving dynamic content), then ask Claude Code to update dynamic sections.

**Parameters:** Same as `ensure_claudemd`.

**Returns:** None (void).

**Logic:**
1. If `workspace/CLAUDE.md` doesn't exist, delegate to `ensure_claudemd()`.
2. Read the current file and extract dynamic content below the `DYNAMIC_MARKER` using `extract_dynamic_content()`.
3. Write the bundled template + preserved dynamic content back to the file. This re-deploys all standard rules while keeping existing skills/projects tables.
4. Run `claude -p` with a prompt to update only the dynamic sections.
5. On failure, log warning — template rules are already re-deployed.

### `fn extract_dynamic_content(file_content: &str) -> Option<String>`
**Purpose:** Extract everything below the dynamic content marker line.

**Parameters:**
- `file_content: &str` — Full contents of the CLAUDE.md file.

**Returns:** `Some(content)` if the marker is found and there is non-empty content below it, `None` otherwise.

**Logic:**
1. Find the `DYNAMIC_MARKER` string in the file content.
2. Find the end of the marker line (next `\n`).
3. Extract and trim everything after that line.
4. Return `None` if empty, `Some(trimmed_content)` otherwise.

### `pub async fn claudemd_loop(workspace: PathBuf, data_dir: PathBuf, interval_hours: u64)`
**Purpose:** Background loop that periodically refreshes the workspace CLAUDE.md.

**Parameters:**
- `workspace: PathBuf` — Path to `~/.omega/workspace/`.
- `data_dir: PathBuf` — Path to `~/.omega/`.
- `interval_hours: u64` — Hours between refreshes (default: 24).

**Returns:** Never returns (infinite loop, aborted on shutdown).

**Logic:**
1. Sleep for `interval_hours * 3600` seconds.
2. Call `refresh_claudemd()`.
3. Repeat.

### `async fn run_claude(prompt: &str, workspace: &Path, data_dir: &Path) -> Result<(), String>`
**Purpose:** Run `claude -p` as a direct subprocess for CLAUDE.md maintenance.

**Parameters:**
- `prompt: &str` — The prompt to send to Claude Code.
- `workspace: &Path` — Working directory for the subprocess.
- `data_dir: &Path` — Data directory for filesystem protection.

**Returns:** `Result<(), String>` — Ok on success, Err with description on failure.

**Logic:**
1. Build command via `omega_sandbox::protected_command("claude", data_dir)`.
2. Set `current_dir(workspace)`, remove `CLAUDECODE` env var.
3. Pass `-p`, `--output-format json`, `--dangerously-skip-permissions`.
4. Execute with 120s timeout via `tokio::time::timeout()`.
5. Check exit status — Ok if success, Err with truncated stderr otherwise.

## Bundled Template

The template is stored at `prompts/WORKSPACE_CLAUDE.md` and bundled into the binary via `include_str!` in `omega-core/src/config.rs`. It contains:

- `# OMEGA Workspace` — intro
- `## Directory Structure` — static layout of `~/.omega/`
- `## Your Infrastructure` — background loops, critical distinctions, permissions
- `## Diagnostic Protocol` — mandatory log investigation steps
- `## Known False Diagnoses` — documented wrong claims to never repeat
- `## Key Conventions` — sandbox, markers, DB path
- `<!-- DYNAMIC CONTENT BELOW ... -->` — marker line

Dynamic sections (Available Skills, Available Projects) are NOT in the template — they're instance-specific and generated/updated by `claude -p`.

## Tests

### `test_ensure_claudemd_skips_when_exists`
**Type:** Async unit test (`#[tokio::test]`)
Verifies that `ensure_claudemd()` returns immediately without spawning a subprocess when `CLAUDE.md` already exists.

### `test_claudemd_path_construction`
**Type:** Synchronous unit test (`#[test]`)
Verifies that the CLAUDE.md path is correctly constructed from the workspace path.

### `test_template_contains_standard_sections`
**Type:** Synchronous unit test (`#[test]`)
Verifies the bundled template contains all expected sections: main heading, directory structure, infrastructure, diagnostic protocol, known false diagnoses, key conventions, and the dynamic content marker.

### `test_extract_dynamic_content_with_content`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_dynamic_content()` correctly extracts content below the marker line.

### `test_extract_dynamic_content_empty`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_dynamic_content()` returns `None` when the marker exists but no content follows.

### `test_extract_dynamic_content_no_marker`
**Type:** Synchronous unit test (`#[test]`)
Verifies that `extract_dynamic_content()` returns `None` when no marker is present.

### `test_refresh_preserves_template_sections`
**Type:** Synchronous unit test (`#[test]`)
Verifies that the refresh logic preserves both template sections (diagnostic protocol, known false diagnoses, key conventions) and dynamic content (skills table) after reconstruction.

## Dependencies

### External Crates
- `tokio` — Async runtime, process spawning, timeout.
- `tracing` — Structured logging (`info!`, `warn!`).

### Internal Dependencies
- `omega_core::config::bundled_workspace_claude` — Bundled template accessor.
- `omega_sandbox::protected_command` — OS-level filesystem protection (always-on blocklist).

## Design Decisions

- **Template-first:** The bundled template guarantees standard operational rules are always present, even if `claude -p` fails. This solves the problem of critical rules being lost on fresh deployments or 24h refreshes.
- **Dynamic content marker:** A clear HTML comment separates bundled rules from instance-specific content. The refresh logic preserves dynamic content while re-deploying the latest template.
- **Graceful degradation:** If `claude -p` is unavailable, the template alone provides all standard rules. Dynamic content (skills/projects tables) is a nice-to-have, not critical.
- **Direct subprocess, not Provider trait:** This is a system maintenance operation. Using the Provider trait would mix concerns.
- **Non-blocking startup:** `ensure_claudemd` is spawned as a background task, doesn't block channel startup.
- **Non-fatal:** All failures are logged as warnings, never crash the gateway.
- **Claude Code only:** Guarded by `provider.name() == "claude-code"` in the gateway.
- **24-hour refresh:** Low cost, keeps the file reasonably current without burning tokens.
- **No config section:** Always-on for Claude Code provider — follows the "less is more" principle.
