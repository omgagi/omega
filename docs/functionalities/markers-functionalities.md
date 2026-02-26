# Functionalities: Markers

## Overview
The marker protocol allows AI responses to contain structured side-effect instructions. The gateway extracts markers, processes them (creating tasks, storing facts, modifying files), then strips them before sending the response to the user. This is the primary mechanism for AI autonomy.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | extract_all_schedule_markers() | Function | backend/src/markers/schedule.rs:12 | Extracts all SCHEDULE: lines from response text | -- |
| 2 | parse_schedule_line() | Function | backend/src/markers/schedule.rs:20 | Parses `SCHEDULE: desc \| ISO datetime \| repeat` into (desc, due_at, repeat) | -- |
| 3 | strip_schedule_marker() | Function | backend/src/markers/schedule.rs:36 | Strips all SCHEDULE: lines from response text | -- |
| 4 | extract_all_schedule_action_markers() | Function | backend/src/markers/schedule.rs:58 | Extracts all SCHEDULE_ACTION: lines | -- |
| 5 | parse_schedule_action_line() | Function | backend/src/markers/schedule.rs:66 | Parses `SCHEDULE_ACTION: desc \| ISO datetime \| repeat` | -- |
| 6 | strip_schedule_action_marker() | Function | backend/src/markers/schedule.rs:~80 | Strips all SCHEDULE_ACTION: lines | -- |
| 7 | extract_inline_marker_value() | Function | backend/src/markers/mod.rs:~10 | Extracts value for a named inline marker (e.g., LANG_SWITCH: value) | -- |
| 8 | strip_inline_marker() | Function | backend/src/markers/mod.rs:~20 | Strips a single named inline marker from text | -- |
| 9 | strip_all_remaining_markers() | Function | backend/src/markers/mod.rs:~30 | Safety net: strips any remaining known markers (catches inline markers from small models) | -- |
| 10 | LANG_SWITCH marker | Protocol | backend/src/markers/protocol.rs | Switches user's preferred_language fact | Memory |
| 11 | PERSONALITY marker | Protocol | backend/src/markers/protocol.rs | Sets user's personality fact | Memory |
| 12 | FORGET_CONVERSATION marker | Protocol | backend/src/markers/protocol.rs | Closes current conversation (triggers summarization) | Memory |
| 13 | CANCEL_TASK marker | Protocol | backend/src/markers/protocol.rs | Cancels a task by ID prefix | Memory |
| 14 | UPDATE_TASK marker | Protocol | backend/src/markers/protocol.rs | Updates a task's description | Memory |
| 15 | PURGE_FACTS marker | Protocol | backend/src/markers/protocol.rs | Deletes all non-system facts | Memory |
| 16 | PROJECT_ACTIVATE marker | Protocol | backend/src/markers/protocol.rs | Activates a project by setting active_project fact | Memory |
| 17 | PROJECT_DEACTIVATE marker | Protocol | backend/src/markers/protocol.rs | Deactivates current project | Memory |
| 18 | BUILD_PROPOSAL marker | Protocol | backend/src/markers/protocol.rs | Proposes a build request (enters confirmation gate) | Gateway |
| 19 | WHATSAPP_QR marker | Protocol | backend/src/markers/protocol.rs | Triggers WhatsApp QR pairing flow | WhatsApp channel |
| 20 | HEARTBEAT_ADD/REMOVE/INTERVAL markers | Protocol | backend/src/markers/heartbeat.rs | Add/remove items from heartbeat watchlist; change interval | Filesystem, config |
| 21 | SKILL_IMPROVE marker | Protocol | backend/src/markers/actions.rs | Appends lesson to skill's ## Lessons Learned section | Skills, filesystem |
| 22 | BUG_REPORT marker | Protocol | backend/src/markers/actions.rs | Appends bug description to {data_dir}/BUG.md | Filesystem |
| 23 | ACTION_OUTCOME marker | Protocol | backend/src/markers/actions.rs | Enum: Success or Failed(String); used by action tasks for verification | Scheduler |
| 24 | REWARD marker | Protocol | backend/src/markers/actions.rs | `REWARD: score\|domain\|lesson` (score -1 to +1); stores outcome | Memory |
| 25 | LESSON marker | Protocol | backend/src/markers/actions.rs | `LESSON: domain\|rule`; stores distilled behavioral rule | Memory |
| 26 | HeartbeatAction enum | Enum | backend/src/markers/heartbeat.rs:~5 | Add(String), Remove(String), SetInterval(u64) | -- |

## Internal Dependencies
- process_markers() [gateway] -> all extract/parse/strip functions
- SCHEDULE markers -> create_task() [memory]
- HEARTBEAT markers -> apply_heartbeat_changes() -> filesystem + config
- REWARD/LESSON -> store_outcome() / store_lesson() [memory]
- SKILL_IMPROVE -> filesystem (appends to SKILL.md)
- BUG_REPORT -> filesystem (appends to BUG.md)

## Dead Code / Unused
- **extract_schedule_marker()**: `backend/src/markers/schedule.rs:4` -- marked `#[allow(dead_code)]`, superseded by `extract_all_schedule_markers()`
- **extract_schedule_action_marker()**: `backend/src/markers/schedule.rs:50` -- marked `#[allow(dead_code)]`, superseded by `extract_all_schedule_action_markers()`
