# BUG: Heartbeat Returns Global Content When Project Is Active

**ID:** BUG-HB-PROJECT-SCOPE
**Severity:** Medium
**Reported:** 2026-02-28

## Symptom

When a user activates a project (e.g., "tech-youtuber") and asks about the heartbeat, OMEGA returns the **global heartbeat** (`~/.omega/prompts/HEARTBEAT.md`) instead of the **project-specific heartbeat** (`~/.omega/projects/<name>/HEARTBEAT.md`).

## Root Cause

Three locations always read the global heartbeat, ignoring the active project:

1. **`pipeline.rs:985`** — `build_system_prompt` calls `read_heartbeat_file()` unconditionally. The `active_project` parameter is available but unused for heartbeat.
2. **`commands/settings.rs:191`** — `/heartbeat` command calls `read_heartbeat_file()` with no project awareness.
3. **`pipeline.rs:191`** — `CommandContext` is constructed before `active_project` is fetched (line 235), so commands never have project context.

The heartbeat **loop** (`gateway/heartbeat.rs:253-306`) is NOT affected — it already uses `read_project_heartbeat_file()`.

## Requirements

| ID | Priority | Requirement |
|----|----------|-------------|
| REQ-HBPS-001 | Must | Pipeline heartbeat injection uses project heartbeat when active, falls back to global |
| REQ-HBPS-002 | Must | `/heartbeat` command shows project content when active, falls back to global |
| REQ-HBPS-003 | Must | `active_project` resolved before command dispatch; added to `CommandContext` |
| REQ-HBPS-004 | Must | Tests updated and added for project-scoped heartbeat |

## Design Decision

When a project is active and both heartbeat files exist: show **project only**. Fall back to global only if the project has no heartbeat file.

## Files Changed

| File | Change |
|------|--------|
| `backend/src/gateway/pipeline.rs` | Fetch `active_project` before commands; use it in heartbeat injection |
| `backend/src/commands/mod.rs` | Add `active_project` field to `CommandContext` |
| `backend/src/commands/settings.rs` | Accept and use `active_project` in `handle_heartbeat` |
| `backend/src/commands/tests.rs` | Update existing tests, add project-scoped tests |
