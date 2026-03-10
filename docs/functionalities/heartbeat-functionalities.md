# Functionalities: Heartbeat

## Overview

The heartbeat system performs periodic AI check-ins on a clock-aligned schedule. It uses a fast model (Sonnet) to classify checklist items into domain groups, then executes each group in parallel with the complex model (Opus). Project-specific heartbeats are also supported.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | heartbeat_loop() | Background Task | `backend/src/gateway/heartbeat.rs` | Clock-aligned heartbeat loop: quiet hours jump-ahead, wall-clock re-snap after system sleep, global + project heartbeats, interval notification via Notify | Provider, Channels, HeartbeatConfig |
| 2 | next_clock_boundary() | Utility | `backend/src/gateway/heartbeat.rs:31` | Computes next clock-aligned boundary given current minute and interval | -- |
| 3 | classify_heartbeat_groups() | Service | `backend/src/gateway/heartbeat.rs` | Fast Sonnet classification of checklist items into domain groups for parallel execution | Provider |
| 4 | execute_heartbeat_group() | Service | `backend/src/gateway/heartbeat.rs` | Executes a single heartbeat domain group with Opus, processes markers | Provider |
| 5 | HEARTBEAT_OK evaluation | Utility | `backend/src/gateway/heartbeat.rs` | Detects HEARTBEAT_OK marker in responses to determine suppression | -- |
| 6 | Project heartbeats | Service | `backend/src/gateway/heartbeat.rs` | Filesystem-based discovery of project-specific HEARTBEAT.md files, executes each project heartbeat | load_projects, read_project_heartbeat_file |
| 7 | build_enrichment() | Service | `backend/src/gateway/heartbeat_helpers.rs` | Builds enrichment context (lessons, outcomes, profile) for heartbeat execution | Store |
| 8 | build_system_prompt() | Service | `backend/src/gateway/heartbeat_helpers.rs` | Builds system prompt for heartbeat execution | Prompts |
| 9 | process_heartbeat_markers() | Service | `backend/src/gateway/heartbeat_helpers.rs` | Processes markers from heartbeat responses (SCHEDULE, HEARTBEAT_*, CANCEL/UPDATE/REWARD/LESSON, PROJECT) | shared_markers |
| 10 | send_heartbeat_result() | Service | `backend/src/gateway/heartbeat_helpers.rs` | Sends heartbeat results to channel, skipping HEARTBEAT_OK responses | Channel |

## Internal Dependencies

- heartbeat_loop() calls classify_heartbeat_groups() -> execute_heartbeat_group() for global heartbeat
- heartbeat_loop() iterates projects with HEARTBEAT.md for project heartbeats
- process_heartbeat_markers() reuses shared_markers::process_task_and_learning_markers()

## Dead Code / Unused

- None detected.
