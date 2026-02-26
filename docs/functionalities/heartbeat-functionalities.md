# Functionalities: Heartbeat

## Overview
The heartbeat system provides autonomous periodic check-ins. It reads checklist files (global + per-project), classifies items into semantic groups, executes each group via the AI provider, processes resulting markers, and delivers findings to the user. It operates entirely without user input.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | heartbeat_loop() | Background Loop | backend/src/gateway/heartbeat.rs:~10 | Clock-aligned periodic loop; checks active hours; runs global then per-project heartbeats | Config, provider, memory |
| 2 | classify_heartbeat_groups() | Method | backend/src/gateway/heartbeat.rs:~80 | Classifies checklist items into semantic groups via Sonnet; returns DIRECT for <=3 items or same domain | Provider |
| 3 | execute_heartbeat_group() | Method | backend/src/gateway/heartbeat.rs:~120 | Builds enrichment + checklist prompt, calls provider via Opus, processes markers; returns None if HEARTBEAT_OK | Provider, memory, markers |
| 4 | run_project_heartbeats() | Method | backend/src/gateway/heartbeat.rs:~160 | Discovers all users with active_project fact; checks for per-project HEARTBEAT.md; runs single-call heartbeat per project | Memory, filesystem |
| 5 | build_enrichment() | Function | backend/src/gateway/heartbeat_helpers.rs:~10 | Loads all facts + recent summaries (3) + lessons (project-scoped) + outcomes (24h, project-scoped) for context | Memory |
| 6 | build_system_prompt() | Function | backend/src/gateway/heartbeat_helpers.rs:~40 | Identity + Soul + System + current time + optional project ROLE.md | Prompts, projects |
| 7 | process_heartbeat_markers() | Function | backend/src/gateway/heartbeat_helpers.rs:~70 | Handles SCHEDULE, SCHEDULE_ACTION, HEARTBEAT_ADD/REMOVE/INTERVAL, CANCEL_TASK, UPDATE_TASK, REWARD, LESSON; all project-tagged | Markers, memory |
| 8 | send_heartbeat_result() | Function | backend/src/gateway/heartbeat_helpers.rs:~120 | Audits the heartbeat interaction and sends result to configured channel | Audit, channel |
| 9 | read_heartbeat_file() | Function | backend/src/markers/heartbeat.rs:~5 | Reads ~/.omega/prompts/HEARTBEAT.md | Filesystem |
| 10 | read_project_heartbeat_file() | Function | backend/src/markers/heartbeat.rs:~15 | Reads ~/.omega/projects/<name>/HEARTBEAT.md | Filesystem |
| 11 | apply_heartbeat_changes() | Function | backend/src/markers/heartbeat.rs:~25 | Applies Add/Remove/SetInterval actions to heartbeat files; project-aware; duplicate prevention; case-insensitive matching | Filesystem, config |
| 12 | /heartbeat command | Handler | backend/src/commands/settings.rs:172 | Shows heartbeat status (enabled/disabled), interval, and current watchlist items | Config, filesystem, i18n |

## Internal Dependencies
- heartbeat_loop() -> classify_heartbeat_groups() -> execute_heartbeat_group()
- heartbeat_loop() -> run_project_heartbeats()
- execute_heartbeat_group() -> build_enrichment() + build_system_prompt() + process_heartbeat_markers()
- process_heartbeat_markers() -> apply_heartbeat_changes() (for HEARTBEAT_ADD/REMOVE/INTERVAL)
- apply_heartbeat_changes() -> patch_heartbeat_interval() (persists to config.toml)

## Dead Code / Unused
None detected.
