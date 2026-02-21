# Workspace CLAUDE.md Maintenance

## Overview

When OMEGA runs with the Claude Code CLI provider, the subprocess is invoked in `~/.omega/workspace/`. Claude Code automatically reads `CLAUDE.md` from its working directory. The `claudemd` module ensures this file exists and stays current, giving the subprocess persistent workspace context.

## How It Works

### Startup (ensure)

On gateway startup, if the provider is `claude-code` and `~/.omega/workspace/CLAUDE.md` doesn't exist, the gateway spawns a background task that runs `claude -p` with an init prompt. The prompt instructs Claude Code to:

1. Explore the workspace and sibling directories (`~/.omega/skills/`, `~/.omega/projects/`)
2. Create a concise `CLAUDE.md` describing the workspace structure, available skills, and conventions

This runs non-blocking — it doesn't delay channel startup. If it fails (e.g., Claude CLI not available), a warning is logged and the gateway continues normally.

### Background loop (refresh)

A background loop runs every 24 hours, asking Claude Code to review and update the existing `CLAUDE.md` if the workspace has changed (new skills, projects, files). If the file was deleted between runs, it falls back to the init flow.

### Subprocess details

Both operations use direct `claude -p` subprocess calls with:
- `--output-format json` and `--dangerously-skip-permissions`
- OS-level sandbox enforcement via `omega_sandbox::sandboxed_command()`
- 120-second timeout
- `CLAUDECODE` env var removed (prevents nested session errors)

This is a system maintenance operation, not a user message — it bypasses the Provider trait intentionally.

## File Location

- **Source:** `src/claudemd.rs`
- **Output:** `~/.omega/workspace/CLAUDE.md`

## Configuration

None required. This feature is always-on for the Claude Code provider and inactive for all other providers. No config.toml section needed.

## Failure Handling

All failures are non-fatal. Errors are logged as warnings via `tracing::warn!`. The gateway continues running normally regardless of CLAUDE.md maintenance status.
