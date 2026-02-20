# Specification: src/markers.rs

## Purpose
Centralized marker extraction, parsing, and stripping for the gateway protocol. All system markers emitted by the AI (SCHEDULE:, LANG_SWITCH:, SELF_HEAL:, LIMITATION:, etc.) are processed here. This module was extracted from gateway.rs to improve maintainability and testability.

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
- **CANCEL_TASK**: `extract_cancel_task`, `strip_cancel_task`
- **PURGE_FACTS**: `has_purge_marker`, `strip_purge_marker`
- **PROJECT_ACTIVATE/DEACTIVATE**: `extract_project_activate`, `has_project_deactivate`, `strip_project_markers`
- **WHATSAPP_QR**: `has_whatsapp_qr_marker`, `strip_whatsapp_qr_marker`
- **HEARTBEAT_ADD/REMOVE/INTERVAL**: `extract_heartbeat_markers`, `strip_heartbeat_markers`, `apply_heartbeat_changes`
- **LIMITATION**: `extract_limitation_marker`, `parse_limitation_line`, `strip_limitation_markers`
- **SELF_HEAL**: `extract_self_heal_marker`, `parse_self_heal_line`, `has_self_heal_resolved_marker`, `strip_self_heal_markers`

### Self-Healing State
- `SelfHealingState` struct (anomaly, verification, iteration, max_iterations, started_at, attempts)
- `self_healing_path()`, `read_self_healing_state()`, `write_self_healing_state()`, `delete_self_healing_state()`
- `detect_repo_path()` -- Auto-detect repo from binary location
- `self_heal_follow_up()` -- Build follow-up task description

### Classification Helpers
- `build_classification_context()` -- Build context string for complexity classifier
- `parse_plan_response()` -- Parse numbered steps from classification response

### Misc Helpers
- `status_messages(lang)` -- Localized status messages (8 languages)
- `friendly_provider_error(raw)` -- Map provider errors to user-friendly messages
- `snapshot_workspace_images(workspace)` -- Snapshot image files for workspace diff
- `is_within_active_hours(start, end)` -- Check if current time is in active window
- `ensure_inbox_dir(data_dir)` -- Create workspace/inbox directory
- `save_attachments_to_inbox()`, `cleanup_inbox_images()` -- Image attachment lifecycle

## Types
- `HeartbeatAction` enum: `Add(String)`, `Remove(String)`, `SetInterval(u64)`
- `SelfHealingState` struct (serializable to JSON)

## Tests
~100 tests covering all marker types, edge cases, inline markers, heartbeat file operations, workspace snapshots, classification parsing, self-healing flow simulation.
