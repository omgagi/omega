# Markers Module (backend/src/markers/)

## Overview

The markers module handles extraction, parsing, and stripping of protocol markers emitted by the AI in response text. These markers are the communication channel between the AI and the gateway -- they instruct Omega to perform actions (schedule tasks, switch language, improve skills, etc.) and are always stripped from the response before it reaches the user.

## Module Structure

| File | Purpose |
|------|---------|
| `mod.rs` | Generic helpers: `extract_inline_marker_value()`, `strip_inline_marker()`, `strip_all_remaining_markers()` |
| `schedule.rs` | `SCHEDULE:` and `SCHEDULE_ACTION:` markers -- parsing, extraction, stripping |
| `protocol.rs` | Simple markers: `LANG_SWITCH:`, `PERSONALITY:`, `FORGET:`, `CANCEL_TASK:`, `UPDATE_TASK:`, `PURGE_FACTS:`, `WHATSAPP_QR:`, `PROJECT_ACTIVATE:`, `PROJECT_DEACTIVATE:` |
| `heartbeat.rs` | Heartbeat markers: `HEARTBEAT_OK`, `HEARTBEAT_INTERVAL:`, `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, plus heartbeat file operations and section parsing/suppression |
| `actions.rs` | Action markers: `BUG_REPORT:`, `SKILL_IMPROVE:`, `ACTION_OUTCOME:`, `REWARD:`, `LESSON:` |
| `helpers.rs` | Status messages, workspace images, inbox classification |
| `tests/` | 6 test submodules with ~145 tests covering all marker types |

## Marker Types

### Schedule Markers
| Marker | Format | Purpose |
|--------|--------|---------|
| `SCHEDULE:` | `desc \| datetime \| repeat` | Create a reminder task |
| `SCHEDULE_ACTION:` | `desc \| datetime \| repeat` | Create an autonomous action task |

### Protocol Markers
| Marker | Format | Purpose |
|--------|--------|---------|
| `LANG_SWITCH:` | `language_name` | Change user's preferred language |
| `PERSONALITY:` | `style_description` | Change response personality |
| `FORGET:` | (no value) | Clear current conversation |
| `CANCEL_TASK:` | `id_prefix` | Cancel a scheduled task by ID prefix |
| `UPDATE_TASK:` | `id_prefix \| field \| value` | Update a scheduled task |
| `PURGE_FACTS:` | (no value) | Clear all facts for user |
| `WHATSAPP_QR:` | (no value) | Trigger WhatsApp QR pairing |
| `PROJECT_ACTIVATE:` | `project_name` | Activate a project context |
| `PROJECT_DEACTIVATE:` | (no value) | Deactivate current project |

### Heartbeat Markers
| Marker | Format | Purpose |
|--------|--------|---------|
| `HEARTBEAT_OK` | (no value) | Signal all-clear (response suppressed) |
| `HEARTBEAT_INTERVAL:` | `minutes` | Dynamically change heartbeat interval |
| `HEARTBEAT_ADD:` | `item` | Add item to heartbeat checklist |
| `HEARTBEAT_REMOVE:` | `item` | Remove item from heartbeat checklist |

### Action Markers
| Marker | Format | Purpose |
|--------|--------|---------|
| `BUG_REPORT:` | `description` | Log a bug to BUG.md |
| `SKILL_IMPROVE:` | `skill_name \| lesson` | Append lesson to skill's SKILL.md |
| `ACTION_OUTCOME:` | `outcome_text` | Report action task execution result |
| `REWARD:` | `outcome_text \| score` | Record reward-based learning outcome |
| `LESSON:` | `domain \| rule` | Record learned behavioral rule |

## How Marker Processing Works

1. **AI generates response** with markers embedded in text
2. **Gateway calls `process_markers()`** which iterates through all known marker types
3. For each found marker:
   - The value is extracted and parsed
   - The corresponding action is executed (DB write, file update, etc.)
   - Results are collected as `MarkerResult` values
4. **All markers are stripped** from the response text
5. **Clean response** is sent to the user
6. **Task confirmation** message is sent separately (if applicable)

## Key Design Decisions

- **Inline detection:** Markers are detected both at line-start and inline (small models sometimes embed markers mid-sentence)
- **Stripping is separate from extraction:** The same text is processed for extraction first, then stripped -- this avoids order-of-operations issues
- **All markers are stripped:** Even unknown/malformed markers are removed via `strip_all_remaining_markers()` to prevent protocol leakage to users
- **Pipe-delimited values:** Multi-field markers use `|` as delimiter (e.g., `SCHEDULE: desc | datetime | repeat`)
