# Task Confirmation (backend/src/task_confirmation.rs)

## Overview

The task confirmation module is an **anti-hallucination layer** for task scheduling. When the AI emits `SCHEDULE:` or `SCHEDULE_ACTION:` markers, the gateway processes them against the database and collects results. This module formats those results into a localized confirmation message sent AFTER the AI's response.

**Why it matters:** The AI might claim "I scheduled your reminder for tomorrow at 9am" but the actual database write could fail, the time could be parsed differently, or a duplicate might already exist. The confirmation message shows users what was **actually** created -- not what the AI claimed.

## How It Works

### Flow

1. AI response contains markers like `SCHEDULE: Call dentist | 2026-03-15T10:00:00 | once`
2. Gateway's `process_markers()` processes each marker and collects `MarkerResult` values
3. Gateway calls `format_task_confirmation()` with the collected results
4. If the formatted message is non-empty, it is sent as a separate message after the AI's response

### MarkerResult Types

| Variant | When It Happens |
|---------|----------------|
| `TaskCreated` | Task saved to database successfully |
| `TaskFailed` | Database write failed |
| `TaskParseError` | Marker line could not be parsed |
| `TaskCancelled` | Task cancelled via `CANCEL_TASK:` marker |
| `TaskCancelFailed` | Cancellation failed (no match or DB error) |
| `TaskUpdated` | Task updated via `UPDATE_TASK:` marker |
| `TaskUpdateFailed` | Update failed (no match or DB error) |
| `SkillImproved` | Skill instruction updated via `SKILL_IMPROVE:` marker |
| `SkillImproveFailed` | Skill update failed |
| `BugReported` | Bug logged to `BUG.md` |
| `BugReportFailed` | Bug logging failed |

### Duplicate Detection

The `descriptions_are_similar()` function detects potential duplicate tasks using word overlap:

1. Extract significant words (3+ characters, excluding stop words like "the", "and", "for")
2. Lowercase all words
3. Check if 50%+ of the smaller word set overlaps with the larger set
4. If similar, a warning is included in the confirmation message

### Output Format

**Single task created:**
```
Scheduled: Call dentist -- 2026-03-15T10:00:00 (once)
```

**Multiple tasks created:**
```
Scheduled 3 tasks:
  * Task A -- 2026-02-22T09:00:00 (daily)
  * Task B -- 2026-02-25T10:00:00 (once)
  * Task C -- 2026-03-01T12:00:00 (weekly)
```

**With similar task warning:**
```
Scheduled: Cancel VPS -- 2026-03-15T09:00:00 (once)
Similar task exists: "Cancel Hostinger VPS" -- Mar 15
```

### Smart Suppression

When creates and cancels happen in the same batch (e.g., user asks to reschedule), the cancellation messages are suppressed because they are implicit replacements, not user-requested standalone cancellations.

## Localization

All confirmation text is localized through the `i18n` module. The `lang` parameter (e.g., "English", "Spanish") determines which translations are used for labels like "Scheduled", "Cancelled", "Failed", "Similar task exists", etc.

## API

### `format_task_confirmation(results, similar_warnings, lang) -> Option<String>`

Returns `None` if there are no results to report. Returns `Some(message)` with the formatted confirmation.

### `descriptions_are_similar(a, b) -> bool`

Checks if two task descriptions are semantically similar using word overlap.
