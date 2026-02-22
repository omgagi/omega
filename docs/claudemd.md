# Workspace CLAUDE.md Maintenance

## Overview

When OMEGA runs with the Claude Code CLI provider, the subprocess is invoked in `~/.omega/workspace/`. Claude Code automatically reads `CLAUDE.md` from its working directory. The `claudemd` module ensures this file exists and stays current, giving the subprocess persistent workspace context.

## Template-First Approach

The workspace CLAUDE.md uses a **bundled template** (`prompts/WORKSPACE_CLAUDE.md`) that is compiled into the binary. This template contains all standard operational rules that OMEGA's subprocess needs:

- **Directory Structure** — layout of `~/.omega/`
- **Infrastructure** — background loops (heartbeat, scheduler, CLAUDE.md refresh), critical distinctions, permissions
- **Diagnostic Protocol** — mandatory log investigation steps before claiming issues
- **Known False Diagnoses** — documented wrong claims to never repeat
- **Key Conventions** — sandbox, markers, memory DB

Dynamic content (Available Skills, Available Projects tables) is instance-specific and gets appended below a `<!-- DYNAMIC CONTENT BELOW -->` marker by `claude -p`.

**Why this matters:** Previously, the entire CLAUDE.md was AI-generated. Critical operational rules added manually would be lost on fresh deployments or the 24h refresh cycle. The template ensures these rules survive.

## How It Works

### Startup (ensure)

On gateway startup, if the provider is `claude-code` and `~/.omega/workspace/CLAUDE.md` doesn't exist:

1. The bundled template is written to `~/.omega/workspace/CLAUDE.md` — standard rules are now guaranteed
2. `claude -p` is spawned to explore skills and projects directories, then append dynamic tables below the marker

If `claude -p` fails (CLI not available, timeout), the template with all standard rules is still there. Graceful degradation.

### Background loop (refresh)

Every 24 hours:

1. The current file is read and dynamic content (below the marker) is extracted
2. The bundled template is re-deployed + the extracted dynamic content is re-appended
3. `claude -p` is spawned to update the dynamic sections if skills/projects have changed

This means the template is always re-deployed from the latest binary — any updates to standard rules in a new release are automatically picked up on the next refresh.

### Subprocess details

Both operations use direct `claude -p` subprocess calls with:
- `--output-format json` and `--dangerously-skip-permissions`
- OS-level filesystem protection via `omega_sandbox::protected_command()`
- 120-second timeout
- `CLAUDECODE` env var removed (prevents nested session errors)

This is a system maintenance operation, not a user message — it bypasses the Provider trait intentionally.

## File Locations

- **Source:** `src/claudemd.rs`
- **Template:** `prompts/WORKSPACE_CLAUDE.md` (bundled into binary via `include_str!`)
- **Output:** `~/.omega/workspace/CLAUDE.md`

## Configuration

None required. This feature is always-on for the Claude Code provider and inactive for all other providers. No config.toml section needed.

## Failure Handling

All failures are non-fatal. Errors are logged as warnings via `tracing::warn!`. The gateway continues running normally regardless of CLAUDE.md maintenance status. The template-first approach ensures standard rules are always deployed even when `claude -p` is unavailable.
