# src/task_confirmation.rs — Task Scheduling Confirmation

## Purpose

Anti-hallucination layer for task scheduling. When the AI emits SCHEDULE/SCHEDULE_ACTION markers, the gateway processes them and collects results into `MarkerResult` values. This module formats those results into a localized confirmation message sent AFTER the AI's response, ensuring users see what was actually created in the database — not what the AI claimed to create.

## Types

### `MarkerResult` (enum)

| Variant | Fields | When |
|---------|--------|------|
| `TaskCreated` | `description`, `due_at`, `repeat`, `task_type` | Task saved to DB successfully |
| `TaskFailed` | `description`, `reason` | DB write failed |
| `TaskParseError` | `raw_line` | Marker line could not be parsed |
| `TaskCancelled` | `id_prefix` | Task cancelled successfully via CANCEL_TASK marker |
| `TaskCancelFailed` | `id_prefix`, `reason` | Task cancellation failed (no match or DB error) |
| `TaskUpdated` | `id_prefix` | Task updated successfully via UPDATE_TASK marker |
| `TaskUpdateFailed` | `id_prefix`, `reason` | Task update failed (no match or DB error) |

## Functions

### `descriptions_are_similar(a: &str, b: &str) -> bool`

Word-overlap similarity check for duplicate detection. Extracts significant words (3+ chars, excluding stop words), computes overlap between smaller and larger sets. Returns true if >= 50% of the smaller set overlaps.

**Stop words**: the, and, for, that, this, with, from, are, was, were, been, have, has, had, will, would, could, should, may, might, can, about, into, over, after, before, between, under, again, then, once, daily, weekly, monthly.

### `format_task_confirmation(results, similar_warnings, lang) -> Option<String>`

Formats a human-readable confirmation message from marker results. Returns `None` if no results to report.

**Output format:**
- Single task created: `✓ Scheduled: {desc} — {due_at} ({repeat})`
- Multiple tasks created: `✓ Scheduled {n} tasks:` + bulleted list
- Single task cancelled: `✓ Cancelled: [{id_prefix}]` (standalone only)
- Multiple tasks cancelled: `✓ Cancelled {n} tasks:` + bulleted list of `[{id}]` (standalone only)
- Single task updated: `✓ Updated: [{id_prefix}]` (standalone only)
- Multiple tasks updated: `✓ Updated {n} tasks:` + bulleted list of `[{id}]` (standalone only)
- Similar warning: `⚠ Similar task exists: "{desc}" — {due_at}`
- Creation failure: `✗ Failed to save {n} task(s). Please try again.`
- Cancel failure: `✗ Failed to cancel [{id}]: {reason}` (standalone only)
- Update failure: `✗ Failed to update [{id}]: {reason}` (standalone only)

**Implicit replacement suppression:** When creates and cancels/updates appear in the same batch, cancel/update sections are suppressed. The AI auto-replaces similar tasks (cancel old + create new), which is an implementation detail — only the created tasks are shown to the user.

All strings are localized via `i18n::t()` and format helpers: `i18n::tasks_confirmed()`, `i18n::tasks_cancelled_confirmed()`, `i18n::tasks_updated_confirmed()`, `i18n::task_save_failed()`.

## Integration

- `gateway.rs::process_markers()` returns `Vec<MarkerResult>` (previously returned `()`)
- `gateway.rs::send_task_confirmation()` calls `get_tasks_for_sender()` to check for similar existing tasks, then calls `format_task_confirmation()` to build the message
- Called from both `handle_message()` (direct responses) and `execute_steps()` (multi-step planning)

## Tests

| Test | Verifies |
|------|----------|
| `test_descriptions_are_similar_exact_match` | Identical descriptions match |
| `test_descriptions_are_similar_reworded` | Semantically similar descriptions match |
| `test_descriptions_are_similar_different` | Unrelated descriptions don't match |
| `test_descriptions_are_similar_empty` | Empty strings don't match |
| `test_descriptions_are_similar_short_words_ignored` | Stop words and short words excluded |
| `test_descriptions_are_similar_case_insensitive` | Case doesn't affect similarity |
| `test_format_task_confirmation_single_created` | Single task formats correctly |
| `test_format_task_confirmation_multiple_created` | Multiple tasks show count + list |
| `test_format_task_confirmation_with_failure` | Failure message shown |
| `test_format_task_confirmation_with_similar_warning` | Warning about similar task shown |
| `test_format_task_confirmation_empty` | Empty results return None |
| `test_format_task_confirmation_single_cancelled` | Single cancelled task formats with id |
| `test_format_task_confirmation_multiple_cancelled` | Multiple cancellations show count + list |
| `test_format_task_confirmation_cancel_failed` | Cancel failure shows id and reason |
| `test_format_task_confirmation_single_updated` | Single updated task formats with id |
| `test_format_task_confirmation_mixed_suppresses_cancels` | When creates present, cancel confirmations are suppressed |
| `test_significant_words` | Word extraction filters correctly |
