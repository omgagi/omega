# Specification: src/markers.rs

## Purpose
Centralized marker extraction, parsing, and stripping for the gateway protocol. All system markers emitted by the AI (SCHEDULE:, LANG_SWITCH:, SKILL_IMPROVE:, etc.) are processed here. This module was extracted from gateway.rs to improve maintainability and testability.

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
- **HEARTBEAT_ADD/REMOVE/INTERVAL**: `extract_heartbeat_markers`, `strip_heartbeat_markers`, `apply_heartbeat_changes`
- **SKILL_IMPROVE**: `extract_skill_improve`, `parse_skill_improve_line`, `strip_skill_improve`
- **BUG_REPORT**: `extract_bug_report`, `strip_bug_report`, `append_bug_report`

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
- `InboxGuard` struct -- RAII guard wrapping `Vec<PathBuf>`, calls `cleanup_inbox_images()` on Drop

## Tests
~90 tests covering all marker types, edge cases, inline markers, heartbeat file operations, workspace snapshots, classification parsing, skill improvement, bug reporting, InboxGuard RAII cleanup, zero-byte attachment rejection.
