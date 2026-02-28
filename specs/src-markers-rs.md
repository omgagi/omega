# Specification: backend/src/markers.rs

## Purpose
Centralized marker extraction, parsing, and stripping for the gateway protocol. All system markers emitted by the AI (SCHEDULE:, LANG_SWITCH:, SKILL_IMPROVE:, etc.) are processed here. This module was originally extracted from the monolithic `gateway.rs`. The gateway itself has since been refactored into `backend/src/gateway/` (directory module with 9 files), but markers remain in `backend/src/markers.rs` as a standalone module used by both the gateway and other components.

## Functions (40+)

### Generic Helpers
- `extract_inline_marker_value(text, prefix)` -- Extract value after marker prefix, handles both line-start and inline positions
- `strip_inline_marker(text, prefix)` -- Remove marker from text, handles both line-start (removes entire line) and inline (removes marker to end of line)
- `strip_all_remaining_markers(text)` -- Safety net: strips all known markers still present in text

### Per-Marker Functions
Each marker type has extract/parse/strip/has functions:
- **SCHEDULE**: `extract_schedule_marker`, `parse_schedule_line`, `strip_schedule_marker`
- **SCHEDULE_ACTION**: `extract_schedule_action_marker`, `parse_schedule_action_line`, `strip_schedule_action_markers`
- **LANG_SWITCH**: `extract_lang_switch`, `strip_lang_switch`
- **PERSONALITY**: `extract_personality`, `strip_personality`
- **FORGET_CONVERSATION**: `has_forget_marker`, `strip_forget_marker`
- **CANCEL_TASK**: `extract_all_cancel_tasks`, `strip_cancel_task`
- **UPDATE_TASK**: `extract_all_update_tasks`, `parse_update_task_line`, `strip_update_task`
- **PURGE_FACTS**: `has_purge_marker`, `strip_purge_marker`
- **PROJECT_ACTIVATE/DEACTIVATE**: `extract_project_activate`, `has_project_deactivate`, `strip_project_markers`
- **WHATSAPP_QR**: `has_whatsapp_qr_marker`, `strip_whatsapp_qr_marker`
- **HEARTBEAT_ADD/REMOVE/INTERVAL**: `extract_heartbeat_markers`, `strip_heartbeat_markers`, `apply_heartbeat_changes(actions, project)`, `read_heartbeat_file()`, `read_project_heartbeat_file(project_name)`
- **HEARTBEAT_SUPPRESS_SECTION/UNSUPPRESS_SECTION**: `extract_suppress_section_markers`, `strip_suppress_section_markers`, `apply_suppress_actions(actions, project)`, `add_suppression(section, project)`, `remove_suppression(section, project)`, `read_suppress_file(project)`, `filter_suppressed_sections(content, project)`, `parse_heartbeat_sections(content)`
- **SKILL_IMPROVE**: `extract_skill_improve`, `parse_skill_improve_line`, `strip_skill_improve`, `apply_skill_improve`
- **BUG_REPORT**: `extract_bug_report`, `strip_bug_report`, `append_bug_report`
- **ACTION_OUTCOME**: `extract_action_outcome`, `strip_action_outcome`
- **REWARD**: `extract_all_rewards`, `parse_reward_line`, `strip_reward_markers`
- **LESSON**: `extract_all_lessons`, `parse_lesson_line`, `strip_lesson_markers`

### Classification Helpers
- `build_classification_context()` -- Build context string for complexity classifier
- `parse_plan_response()` -- Parse numbered steps from classification response

### Misc Helpers
- `status_messages(lang)` -- Localized status messages (8 languages)
- `friendly_provider_error(raw)` -- Map provider errors to user-friendly messages
- `snapshot_workspace_images(workspace)` -- Snapshot image files for workspace diff
- `is_within_active_hours(start, end)` -- Check if current time is in active window
- `ensure_inbox_dir(data_dir)` -- Create workspace/inbox directory
- `save_attachments_to_inbox()`, `cleanup_inbox_images()`, `purge_inbox()` -- Image attachment lifecycle
- `InboxGuard` struct -- RAII guard that cleans up inbox files on Drop

## Types
- `HeartbeatAction` enum: `Add(String)`, `Remove(String)`, `SetInterval(u64)`
- `SuppressAction` enum: `Suppress(String)`, `Unsuppress(String)`
- `ActionOutcome` enum: `Success`, `Failed(String)`
- `InboxGuard` struct -- RAII guard wrapping `Vec<PathBuf>`, calls `cleanup_inbox_images()` on Drop

### REWARD Functions (in `actions.rs`)
- `extract_all_rewards(text)` — Extract all `REWARD:` lines. Format: `REWARD: +1|domain|lesson` or `REWARD: -1|domain|lesson`.
- `parse_reward_line(line)` — Parse into `(score: i32, domain: String, lesson: String)`. Validates score is in `{-1, 0, 1}`, domain and lesson are non-empty.
- `strip_reward_markers(text)` — Remove all `REWARD:` lines from text.

### LESSON Functions (in `actions.rs`)
- `extract_all_lessons(text)` — Extract all `LESSON:` lines. Format: `LESSON: domain|rule`.
- `parse_lesson_line(line)` — Parse into `(domain: String, rule: String)`. Validates domain and rule are non-empty.
- `strip_lesson_markers(text)` — Remove all `LESSON:` lines from text.

### Skill Improve Refactoring (in `actions.rs`)
- `apply_skill_improve(data_dir, skill_name, lesson)` — Extracted from `process_markers.rs`. Reads skill's `SKILL.md`, appends lesson under `## Lessons Learned` section (creates section if missing), writes back to disk.

## Tests
~100 tests covering all marker types, edge cases, inline markers, heartbeat file operations, workspace snapshots, classification parsing, skill improvement, bug reporting, action outcome parsing, reward/lesson extraction and parsing, InboxGuard RAII cleanup, zero-byte attachment rejection.
