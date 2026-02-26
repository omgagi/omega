# Functionalities: Commands & i18n

## Overview
17 slash commands dispatched by the gateway's command handler. All commands support 8 languages via the i18n module. The i18n system uses a simple `t(key, lang)` function with fallback to English.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | try_dispatch_command() | Method | backend/src/commands/mod.rs:~10 | Dispatches slash commands to 4 submodules (status, tasks, settings, learning); returns Some(response) or None | All command handlers |
| 2 | /status | Command | backend/src/commands/status.rs:7 | Shows uptime, provider name, database size | Memory, i18n |
| 3 | /memory | Command | backend/src/commands/status.rs:36 | Shows conversation, message, and fact counts for user | Memory, i18n |
| 4 | /history | Command | backend/src/commands/status.rs:54 | Shows last 5 conversation summaries with timestamps | Memory, i18n |
| 5 | /facts | Command | backend/src/commands/status.rs:73 | Lists all known facts about the user; escapes markdown | Memory, i18n |
| 6 | /help | Command | backend/src/commands/status.rs:87 | Lists all 17 commands with descriptions in user's language | i18n |
| 7 | /tasks | Command | backend/src/commands/tasks.rs:7 | Lists pending tasks with short ID, description, type badge, project badge, due date, repeat | Memory, i18n |
| 8 | /cancel | Command | backend/src/commands/tasks.rs:36 | Cancels task by ID prefix | Memory, i18n |
| 9 | /forget | Command | backend/src/commands/tasks.rs:53 | Closes current conversation; note: intercepted early in pipeline for project-aware handling | Memory, i18n |
| 10 | /purge | Command | backend/src/commands/tasks.rs:80 | Deletes all non-system facts; preserves SYSTEM_FACT_KEYS; restores system facts after bulk delete | Memory, config, i18n |
| 11 | /language | Command | backend/src/commands/settings.rs:8 | Show or set preferred_language fact | Memory, i18n |
| 12 | /personality | Command | backend/src/commands/settings.rs:44 | Show, set, or reset personality fact | Memory, i18n |
| 13 | /skills | Command | backend/src/commands/settings.rs:80 | Lists installed skills with availability status | Skills, i18n |
| 14 | /projects | Command | backend/src/commands/settings.rs:97 | Lists projects, marks active one | Memory, projects, i18n |
| 15 | /project | Command | backend/src/commands/settings.rs:125 | Activate/deactivate/show project | Memory, projects, i18n |
| 16 | /whatsapp | Command | backend/src/commands/settings.rs:168 | Returns WHATSAPP_QR marker for gateway interception | -- |
| 17 | /heartbeat | Command | backend/src/commands/settings.rs:172 | Shows heartbeat status, interval, watchlist items | Config, filesystem, i18n |
| 18 | /learning | Command | backend/src/commands/learning.rs:7 | Shows behavioral rules (lessons) and recent outcomes with time-ago formatting | Memory, i18n |
| 19 | i18n::t() | Function | backend/src/i18n/mod.rs:20 | Lookup localized static string by key and language; fallback chain: labels -> confirmations -> commands -> "???" | -- |
| 20 | i18n format helpers | Functions | backend/src/i18n/format.rs | language_set(), language_show(), personality_updated/show(), purge_result(), project_activated/not_found/active_project(), tasks_confirmed/cancelled/updated() | -- |
| 21 | labels lookup | Function | backend/src/i18n/labels.rs:4 | Static translations for headers, labels, empty states (60+ keys x 8 languages) | -- |
| 22 | confirmations lookup | Function | backend/src/i18n/confirmations.rs:4 | Static translations for task/skill/heartbeat/bug confirmations (40+ keys x 8 languages) | -- |

## Internal Dependencies
- try_dispatch_command() -> handle_status/memory/history/facts/help/tasks/cancel/forget/purge/language/personality/skills/projects/project/whatsapp/heartbeat/learning
- All handlers -> i18n::t() and format helpers
- All data handlers -> Store (memory)

## Dead Code / Unused
None detected.
