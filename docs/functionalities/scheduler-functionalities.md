# Functionalities: Scheduler

## Overview
The scheduler polls for due tasks and executes them. Reminder tasks deliver text messages. Action tasks invoke the full AI provider with tools, project context, and retry logic. Tasks support recurring schedules and are project-scoped.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | scheduler_loop() | Background Loop | backend/src/gateway/scheduler.rs:~5 | Polls every poll_interval_secs; quiet hours gate defers tasks to next active_start; dispatches reminder vs action tasks | Config, memory |
| 2 | execute_action_task() | Function | backend/src/gateway/scheduler_action.rs:~10 | Builds full system prompt with project ROLE.md, user profile, lessons, outcomes, language, delivery instructions, no-builds restriction, verification instruction; calls provider.complete() | Provider, memory, projects |
| 3 | process_action_markers() | Function | backend/src/gateway/scheduler_action.rs:~100 | Processes all markers from action task responses: SCHEDULE, SCHEDULE_ACTION, HEARTBEAT, CANCEL_TASK, UPDATE_TASK, PROJECT_ACTIVATE/DEACTIVATE, FORGET_CONVERSATION, REWARD, LESSON -- all project-tagged | Markers, memory |
| 4 | ACTION_OUTCOME parsing | Logic | backend/src/gateway/scheduler_action.rs:~80 | Parses success/failed from AI response; success completes task; failure triggers retry | -- |
| 5 | Retry logic | Logic | backend/src/gateway/scheduler_action.rs:~90 | Max 3 retries (MAX_ACTION_RETRIES), 2-minute reschedule via fail_task(); permanent fail after max | Memory |
| 6 | create_task() | Method | backend/crates/omega-memory/src/store/tasks.rs:~10 | 2-level dedup: exact description match + fuzzy (30-min window, >50% word overlap); stores with project tag | Memory |
| 7 | complete_task() | Method | backend/crates/omega-memory/src/store/tasks.rs:~50 | One-shot: status='delivered'; recurring: advance due_at (daily/weekly/monthly/weekdays with sat/sun skip) | Memory |
| 8 | fail_task() | Method | backend/crates/omega-memory/src/store/tasks.rs:~90 | Increments retry_count; if < max retries: reschedule +2min; if >= max: status='failed' permanent | Memory |
| 9 | get_due_tasks() | Method | backend/crates/omega-memory/src/store/tasks.rs:~120 | Queries for pending tasks where due_at <= now | Memory |
| 10 | Quiet hours deferral | Logic | backend/src/gateway/scheduler.rs:~30 | If outside active hours, skips execution; uses next_active_start_utc() to compute next window | Config, helpers |

## Internal Dependencies
- scheduler_loop() -> get_due_tasks() -> execute_action_task() or send reminder text
- execute_action_task() -> provider.complete() -> process_action_markers()
- process_action_markers() -> create_task() (for SCHEDULE/SCHEDULE_ACTION)
- ACTION_OUTCOME -> complete_task() or fail_task()
- fail_task() -> retry logic -> reschedule or permanent failure

## Dead Code / Unused
None detected.
