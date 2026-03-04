# Functionalities: Process Markers + Shared Markers

## Overview

The marker system is a protocol between AI responses and the gateway. The AI embeds structured markers in its response text (e.g., `SCHEDULE: ...`, `PROJECT_ACTIVATE: ...`), which the gateway extracts, processes (creating tasks, updating state), and strips before sending the cleaned response to the user.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | process_markers() | Service | `backend/src/gateway/process_markers.rs:17` | Extracts and processes all markers from provider response text. Handles 18+ marker types in sequence | All marker extractors/processors |
| 2 | SCHEDULE marker | Marker | `backend/src/gateway/process_markers.rs:27` | Creates reminder tasks from SCHEDULE markers (description, due_at, repeat) | Store::create_task |
| 3 | SCHEDULE_ACTION marker | Marker | `backend/src/gateway/process_markers.rs:75` | Creates action tasks from SCHEDULE_ACTION markers (provider-executed, not user-facing) | Store::create_task |
| 4 | PROJECT_DEACTIVATE marker | Marker | `backend/src/gateway/process_markers.rs:125` | Deactivates current project: writes .disabled marker, deletes active_project fact | Store::delete_fact |
| 5 | PROJECT_ACTIVATE marker | Marker | `backend/src/gateway/process_markers.rs:147` | Activates a project: removes .disabled marker, stores active_project fact, emits MarkerResult::ProjectActivated | Store::store_fact |
| 6 | BUILD_PROPOSAL marker | Marker | `backend/src/gateway/process_markers.rs:172` | Stores a build proposal as pending_build_request fact for user confirmation | Store::store_fact |
| 7 | WHATSAPP_QR marker | Marker | `backend/src/gateway/process_markers.rs:190` | Triggers WhatsApp QR pairing flow | handle_whatsapp_qr |
| 7b | GOOGLE_SETUP marker | Marker | `backend/src/gateway/process_markers.rs:196` | Triggers Google OAuth setup wizard | start_google_session |
| 8 | LANG_SWITCH marker | Marker | `backend/src/gateway/process_markers.rs:196` | Updates user's preferred_language fact | Store::store_fact |
| 9 | PERSONALITY marker | Marker | `backend/src/gateway/process_markers.rs:210` | Sets or resets user personality preference | Store::store_fact / delete_fact |
| 10 | FORGET_CONVERSATION marker | Marker | `backend/src/gateway/process_markers.rs:234` | Closes current conversation and clears CLI session | Store::close_current_conversation, clear_session |
| 11 | PURGE_FACTS marker | Marker | `backend/src/gateway/process_markers.rs:255` | Purges all non-system facts for sender, preserving SYSTEM_FACT_KEYS | Store::delete_facts, store_fact |
| 12 | HEARTBEAT_ADD/REMOVE/INTERVAL markers | Marker | `backend/src/gateway/process_markers.rs:261` | Modifies heartbeat checklist and interval, persists to config.toml | apply_heartbeat_changes, patch_heartbeat_interval |
| 13 | HEARTBEAT_SUPPRESS/UNSUPPRESS markers | Marker | `backend/src/gateway/process_markers.rs:287` | Suppresses/unsuppresses heartbeat checklist sections | apply_suppress_actions |
| 14 | SKILL_IMPROVE marker | Marker | `backend/src/gateway/process_markers.rs:373` | Updates skill files with learned lessons | apply_skill_improve |
| 15 | BUG_REPORT marker | Marker | `backend/src/gateway/process_markers.rs:395` | Appends bug report to BUG.md file | append_bug_report |
| 16 | send_task_confirmation() | Service | `backend/src/gateway/process_markers.rs:323` | Anti-hallucination: formats confirmation with actual DB results, detects similar existing tasks | format_task_confirmation, descriptions_are_similar |
| 17 | process_purge_facts() | Service | `backend/src/gateway/process_markers.rs:415` | Purges user facts while preserving system-managed keys | Store::get_facts, delete_facts |
| 18 | process_improvement_markers() | Service | `backend/src/gateway/process_markers.rs:373` | Processes SKILL_IMPROVE and BUG_REPORT markers | apply_skill_improve, append_bug_report |
| 19 | process_task_and_learning_markers() | Service | `backend/src/gateway/shared_markers.rs:15` | Shared processing for CANCEL_TASK, UPDATE_TASK, REWARD, LESSON markers across pipeline/action/heartbeat | Store::cancel_task, update_task, store_outcome, store_lesson |
| 20 | Marker module (extract/strip) | Library | `backend/src/markers/mod.rs` | Marker extraction, parsing, and stripping organized into submodules: schedule, protocol, heartbeat, actions, helpers | -- |
| 21 | extract_inline_marker_value() | Utility | `backend/src/markers/mod.rs:28` | Generic inline marker value extraction (line-start + inline fallback) | -- |
| 22 | strip_inline_marker() | Utility | `backend/src/markers/mod.rs:60` | Generic inline marker stripping (line-start removes whole line, inline keeps prefix) | -- |
| 23 | strip_all_remaining_markers() | Utility | `backend/src/markers/mod.rs:88` | Safety net: strips any of 21 known marker prefixes still remaining in text | -- |

## Internal Dependencies

- process_markers() calls all individual marker extractors/processors in sequence
- process_task_and_learning_markers() is shared across process_markers(), scheduler_action, heartbeat_helpers
- send_task_confirmation() uses task_confirmation::format_task_confirmation()
- All marker extraction/stripping functions are in markers/* submodules

## Dead Code / Unused

- `#[allow(dead_code)]` on schedule marker parsing helpers (`backend/src/markers/schedule.rs:4,50`) -- struct fields constructed but some not read directly
